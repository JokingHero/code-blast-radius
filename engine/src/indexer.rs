use crate::schema::{WorkspaceIndex, FileNode, SymbolNode};
use crate::analyzer::analyze_source;
use crate::language::{get_language_configs, LanguageConfig};

use std::path::{Path, PathBuf};

use std::fs::{self, File};
use std::io::Write;
use std::collections::{HashMap, HashSet};
use walkdir::WalkDir;
use blake3;
use memmap2::MmapOptions;
use rkyv::{to_bytes, check_archived_root};

pub struct Indexer {
    pub index: WorkspaceIndex,
    configs: HashMap<String, &'static LanguageConfig>, 
}

impl Indexer {
    pub fn new() -> Self {
        let mut config_map = HashMap::new();
        for config in get_language_configs() {
            for ext in config.file_extensions {
                config_map.insert(ext.to_string(), config);
            }
        }
        Self { index: WorkspaceIndex::default(), configs: config_map }
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let bytes = to_bytes::<_, 4096>(&self.index)
            .map_err(|e| anyhow::anyhow!("Serialization failed: {}", e))?;
        let mut file = File::create(path)?;
        file.write_all(&bytes)?;
        Ok(())
    }

    pub fn load_from_file(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() { return Ok(Self::new()); }
        let file = File::open(path)?;
        let mmap = unsafe { MmapOptions::new().map(&file)? };
        if let Err(e) = check_archived_root::<WorkspaceIndex>(&mmap[..]) {
            eprintln!("Index corrupted: {}", e);
            return Ok(Self::new());
        }
        let index: WorkspaceIndex = unsafe { 
            rkyv::from_bytes_unchecked(&mmap[..]).map_err(|e| anyhow::anyhow!(e))? 
        };
        let mut config_map = HashMap::new();
        for config in get_language_configs() {
            for ext in config.file_extensions {
                config_map.insert(ext.to_string(), config);
            }
        }
        Ok(Self { index, configs: config_map })
    }

    fn clear_file_symbols(&mut self, file_id: u32) {
        let ids_to_remove: Vec<u32> = self.index.symbols.values()
            .filter(|s| s.file_id == file_id)
            .map(|s| s.id)
            .collect();

        for sym_id in ids_to_remove {
            if let Some(sym) = self.index.symbols.remove(&sym_id) {
                if let Some(id_list) = self.index.symbol_map.get_mut(&sym.name) {
                    id_list.retain(|&id| id != sym_id);
                    if id_list.is_empty() {
                        self.index.symbol_map.remove(&sym.name);
                    }
                }
            }
            // Clear raw calls
            self.index.raw_calls.remove(&sym_id);
            // Clear resolved calls (though cleaning strictly involves iterating everything, 
            // orphan IDs are acceptable for now)
            self.index.resolved_calls.remove(&sym_id);
        }
        // Clear imports
        self.index.file_imports.remove(&file_id);
    }

    pub fn scan(&mut self, root: &Path) {
        let mut seen_paths = HashSet::new();

        for entry in WalkDir::new(root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file()) 
        {
            let path = entry.path();
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

            if let Some(config) = self.configs.get(ext) {
                if let Ok(content) = fs::read_to_string(path) {
                    let hash = blake3::hash(content.as_bytes());
                    let hash_bytes: [u8; 32] = hash.into();
                    let path_str = path.to_string_lossy().to_string();

                    seen_paths.insert(path_str.clone());

                    let needs_update = match self.index.files.get(&path_str) {
                        Some(node) => node.hash != hash_bytes,
                        None => true, 
                    };

                    if needs_update {
                        self.update_file(&path_str, path, &content, hash_bytes, config);
                    }
                }
            }
        }

        // Handle Deletions
        let all_known_paths: Vec<String> = self.index.files.keys().cloned().collect();
        for path in all_known_paths {
            if !seen_paths.contains(&path) {
                let id_to_remove = self.index.files.get(&path).map(|node| node.id);
                if let Some(id) = id_to_remove {
                    self.clear_file_symbols(id); 
                    self.index.files.remove(&path);
                }
            }
        }
    }

    fn update_file(
        &mut self, 
        path_key: &str, 
        path_obj: &Path, 
        content: &str, 
        hash: [u8; 32], 
        config: &LanguageConfig
    ) {
        let existing_id = self.index.files.get(path_key).map(|node| node.id);

        let file_id = if let Some(id) = existing_id {
            self.clear_file_symbols(id); 
            id
        } else {
            self.index.files.len() as u32
        };

        self.index.files.insert(path_key.to_string(), FileNode {
            id: file_id,
            path: path_key.to_string(),
            hash,
        });

        if let Ok(analysis) = analyze_source(path_obj, content, config) {
            
            // 1. Store Imports
            if !analysis.imports.is_empty() {
                self.index.file_imports.insert(file_id, analysis.imports);
            }

            // 2. Store Functions
            for func in analysis.functions {
                let symbol_id = self.index.symbols.keys().max().map(|k| k + 1).unwrap_or(0);

                let node = SymbolNode {
                    id: symbol_id,
                    file_id,
                    name: func.name.clone(),
                    kind: "function".to_string(),
                    range_start: func.range_start,
                    range_end: func.range_end,
                    doc_comment: func.documentation,
                };

                self.index.symbols.insert(symbol_id, node);

                self.index.symbol_map.entry(func.name.clone())
                    .or_insert_with(Vec::new)
                    .push(symbol_id);

                // Store raw calls for the Resolution phase
                if !func.calls.is_empty() {
                    self.index.raw_calls.insert(symbol_id, func.calls);
                }
            }
        }
    }

    // --- RESOLUTION LOGIC ---

    /// Converts raw string calls into concrete Symbol IDs based on imports.
    pub fn resolve_references(&mut self) {
        self.index.resolved_calls.clear();

        // Clone entries to iterate while mutating the index
        let entries: Vec<(u32, Vec<String>)> = self.index.raw_calls.iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();

        for (caller_id, called_names) in entries {
            let caller_sym = self.index.symbols.get(&caller_id).unwrap();
            let caller_file_id = caller_sym.file_id;
            
            let mut resolved_targets = Vec::new();

            for func_name in called_names {
                // 1. Local File Check
                // 2. Import Check
                if let Some(target_id) = self.resolve_single_call(caller_file_id, &func_name) {
                    resolved_targets.push(target_id);
                } else {
                    // 3. Global Fallback (Loose Mode)
                    // If semantic resolution failed, find anything with that name.
                    if let Some(candidates) = self.index.symbol_map.get(&func_name) {
                        resolved_targets.extend(candidates.iter().cloned());
                    }
                }
            }
            
            // Deduplicate
            resolved_targets.sort();
            resolved_targets.dedup();
            
            if !resolved_targets.is_empty() {
                self.index.resolved_calls.insert(caller_id, resolved_targets);
            }
        }
    }

    fn resolve_single_call(&self, file_id: u32, func_name: &str) -> Option<u32> {
        // 1. Check Local File
        if let Some(candidates) = self.index.symbol_map.get(func_name) {
            for &id in candidates {
                if let Some(sym) = self.index.symbols.get(&id) {
                    if sym.file_id == file_id {
                        return Some(id);
                    }
                }
            }
        }

        // 2. Check Imports
        if let Some(imports) = self.index.file_imports.get(&file_id) {
            for import in imports {
                if import.name == func_name {
                    // Try to find the file `import.source` points to
                    if let Some(target_file_id) = self.resolve_import_path(file_id, &import.source) {
                        // Look for func_name inside target_file_id
                        if let Some(candidates) = self.index.symbol_map.get(func_name) {
                            for &id in candidates {
                                if let Some(sym) = self.index.symbols.get(&id) {
                                    if sym.file_id == target_file_id {
                                        return Some(id);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn resolve_import_path(&self, from_file_id: u32, import_source: &str) -> Option<u32> {
        let from_file_node = self.index.files.values().find(|f| f.id == from_file_id)?;
        let from_path = Path::new(&from_file_node.path);
        let parent_dir = from_path.parent()?;

        let extensions = ["ts", "js", "tsx", "jsx", "rs", "py"];
        
        let candidate_base = parent_dir.join(import_source);
        
        // Helper to check if a path exists in our index
        // We use fs::canonicalize to resolve "." and ".." if the file actually exists on disk.
        // Since WalkDir returns absolute paths in tests, this ensures we match keys.
        let check_path = |p: PathBuf| -> Option<u32> {
            let target = if p.exists() {
                fs::canonicalize(&p).ok().map(|c| c.to_string_lossy().to_string())?
            } else {
                p.to_string_lossy().to_string()
            };
            self.index.files.get(&target).map(|n| n.id)
        };

        // Try file.ts
        for ext in extensions {
             let candidate = candidate_base.with_extension(ext);
             if let Some(id) = check_path(candidate) {
                 return Some(id);
             }
        }
        
        // Try file/index.ts
        let index_base = candidate_base.join("index");
        for ext in extensions {
             let candidate = index_base.with_extension(ext);
             if let Some(id) = check_path(candidate) {
                 return Some(id);
             }
        }

        None
    }
}