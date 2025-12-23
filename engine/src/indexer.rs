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
        // ... same as before ...
        let mut config_map = HashMap::new();
        for config in get_language_configs() {
            for ext in config.file_extensions {
                config_map.insert(ext.to_string(), config);
            }
        }
        Self { index: WorkspaceIndex::default(), configs: config_map }
    }

    // ... save() and load_from_file() remain the same ...
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

    // --- FIX STARTS HERE ---

    /// Removes all symbols, calls, and map entries associated with a specific file ID.
    fn clear_file_symbols(&mut self, file_id: u32) {
        // 1. Find all Symbol IDs belonging to this file
        // Note: In a production DB, we would have a reverse index for this.
        // For now, iterating the symbol table is acceptable.
        let ids_to_remove: Vec<u32> = self.index.symbols.values()
            .filter(|s| s.file_id == file_id)
            .map(|s| s.id)
            .collect();

        for sym_id in ids_to_remove {
            // Remove from main storage
            if let Some(sym) = self.index.symbols.remove(&sym_id) {
                // Remove from name map (Reverse Index)
                if let Some(id_list) = self.index.symbol_map.get_mut(&sym.name) {
                    id_list.retain(|&id| id != sym_id);
                    if id_list.is_empty() {
                        self.index.symbol_map.remove(&sym.name);
                    }
                }
            }
            // Remove from calls
            self.index.calls.remove(&sym_id);
        }
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

                    // Check if file is new or changed
                    // We map to bool to drop the borrow immediately
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

        let all_known_paths: Vec<String> = self.index.files.keys().cloned().collect();
        
        for path in all_known_paths {
            if !seen_paths.contains(&path) {
                println!("Detected deletion: {}", path);
                // This returns Option<u32> and drops the borrow of self.index.files immediately.
                let id_to_remove = self.index.files.get(&path).map(|node| node.id);
                
                if let Some(id) = id_to_remove {
                    self.clear_file_symbols(id); // no active borrow
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
        // 1. Determine File ID
        // FIX: Check existence and get ID without holding the borrow into the 'if' block
        let existing_id = self.index.files.get(path_key).map(|node| node.id);

        let file_id = if let Some(id) = existing_id {
            // ISSUE B Fix: File exists, clear old symbols
            self.clear_file_symbols(id); // Safe now
            id
        } else {
            // New file, generate new ID
            self.index.files.len() as u32
        };

        // 2. Update/Insert File Node
        self.index.files.insert(path_key.to_string(), FileNode {
            id: file_id,
            path: path_key.to_string(),
            hash,
        });

        // 3. Parse and Store New Symbols
        if let Ok(functions) = analyze_source(path_obj, content, config) {
            for func in functions {
                // Safe ID generation
                let symbol_id = self.index.symbols.keys().max().map(|k| k + 1).unwrap_or(0);

                let node = SymbolNode {
                    id: symbol_id,
                    file_id,
                    name: func.name.clone(),
                    kind: "function".to_string(),
                    range_start: 0, 
                    range_end: 0,
                    doc_comment: func.documentation,
                };

                self.index.symbols.insert(symbol_id, node);

                self.index.symbol_map.entry(func.name.clone())
                    .or_insert_with(Vec::new)
                    .push(symbol_id);

                self.index.calls.insert(symbol_id, func.calls);
            }
        }
    }
    
    pub fn export_graph(&self) -> crate::analyzer::CodebaseGraph {
        let mut graph = crate::analyzer::CodebaseGraph::new();
        for (_id, sym) in &self.index.symbols {
            let calls = self.index.calls.get(&sym.id).cloned().unwrap_or_default();
            if let Some(file_node) = self.index.files.values().find(|f| f.id == sym.file_id) {
                 graph.insert(sym.name.clone(), crate::analyzer::FunctionInfo {
                    name: sym.name.clone(),
                    file_path: PathBuf::from(&file_node.path),
                    source_code: "TODO".to_string(), 
                    documentation: sym.doc_comment.clone(),
                    calls,
                 });
            }
        }
        graph
    }
}