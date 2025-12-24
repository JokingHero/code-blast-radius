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
            self.index.raw_calls.remove(&sym_id);
            self.index.raw_implementations.remove(&sym_id);
            self.index.resolved_calls.remove(&sym_id);
        }
        self.index.file_imports.remove(&file_id);
        self.index.raw_literals.remove(&file_id);
    }

    /// Scans multiple roots into one workspace index.
    pub fn scan_workspace(&mut self, roots: &[PathBuf]) {
        let mut seen_paths = HashSet::new();

        for root in roots {
            let root_abs = fs::canonicalize(root).unwrap_or_else(|_| root.clone());
            
            for entry in WalkDir::new(&root_abs)
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
                        
                        // Use absolute path as key to prevent cross-repo collisions
                        let path_key = path.to_string_lossy().to_string();
                        seen_paths.insert(path_key.clone());

                        let needs_update = match self.index.files.get(&path_key) {
                            Some(node) => node.hash != hash_bytes,
                            None => true, 
                        };

                        if needs_update {
                            self.update_file(&path_key, path, &content, hash_bytes, config);
                        }
                    }
                }
            }
        }

        // Handle Deletions: Remove files that were in the index but not seen in any root
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

    /// Original scan logic preserved for single-folder backwards compatibility
    pub fn scan(&mut self, root: &Path) {
        self.scan_workspace(&[root.to_path_buf()]);
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
            // Find a unique ID
            self.index.files.values().map(|f| f.id).max().map(|id| id + 1).unwrap_or(0)
        };

        self.index.files.insert(path_key.to_string(), FileNode {
            id: file_id,
            path: path_key.to_string(),
            hash,
        });

        if let Ok(analysis) = analyze_source(path_obj, content, config) {
            
            if !analysis.imports.is_empty() {
                self.index.file_imports.insert(file_id, analysis.imports);
            }

            if !analysis.literals.is_empty() {
                self.index.raw_literals.insert(file_id, analysis.literals);
            }

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

                if !func.calls.is_empty() {
                    self.index.raw_calls.insert(symbol_id, func.calls);
                }
            }

            for (child_name, parent_name) in analysis.implementations {
                if let Some(ids) = self.index.symbol_map.get(&child_name) {
                    if let Some(&child_id) = ids.iter().find(|&&id| {
                        self.index.symbols.get(&id).map(|s| s.file_id) == Some(file_id)
                    }) {
                        self.index.raw_implementations.entry(child_id)
                            .or_insert_with(Vec::new)
                            .push(parent_name);
                    }
                }
            }
        }
    }

    pub fn resolve_references(&mut self) {
        // 1. Explicit Function Calls (Symbol to Symbol)
        self.resolve_function_calls();
        
        // 2. Bare Imports (File to File dependencies for side-effects)
        self.resolve_bare_imports();
        
        // 3. Heuristic / Literal Connections (Polyglot literals and Inheritance)
        self.resolve_implicit_connections();
    }

    fn resolve_function_calls(&mut self) {
        self.index.resolved_calls.clear();

        let entries: Vec<(u32, Vec<String>)> = self.index.raw_calls.iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();

        for (caller_id, called_names) in entries {
            let caller_sym = self.index.symbols.get(&caller_id).unwrap();
            let caller_file_id = caller_sym.file_id;
            
            let mut resolved_targets = Vec::new();

            for func_name in called_names {
                if let Some(target_id) = self.resolve_single_call(caller_file_id, &func_name) {
                    resolved_targets.push(target_id);
                } else {
                    // Global Fallback
                    if let Some(candidates) = self.index.symbol_map.get(&func_name) {
                        resolved_targets.extend(candidates.iter().cloned());
                    }
                }
            }
            
            resolved_targets.sort();
            resolved_targets.dedup();
            
            if !resolved_targets.is_empty() {
                self.index.resolved_calls.insert(caller_id, resolved_targets);
            }
        }
    }

    fn resolve_bare_imports(&mut self) {
        // We do not clear file_dependencies here because it is also used by resolve_implicit_connections
        let file_ids: Vec<u32> = self.index.file_imports.keys().cloned().collect();
        for file_id in file_ids {
            let imports = self.index.file_imports.get(&file_id).unwrap().clone();
            for import in imports {
                // If name is empty, it's a side-effect import (e.g., import "./setup")
                if import.name.is_empty() {
                    if let Some(target_file_id) = self.resolve_import_path(file_id, &import.source) {
                        self.index.file_dependencies.entry(file_id)
                            .or_default()
                            .push(target_file_id);
                    }
                }
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

        // 2. Check Named Imports
        if let Some(imports) = self.index.file_imports.get(&file_id) {
            for import in imports {
                if import.name == func_name {
                    if let Some(target_file_id) = self.resolve_import_path(file_id, &import.source) {
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

        let extensions = ["ts", "js", "tsx", "jsx", "rs", "py", "java", "sh"];
        let candidate_base = parent_dir.join(import_source);
        
        let check_path = |p: PathBuf| -> Option<u32> {
             let s = p.to_string_lossy().to_string();
             // Since we use absolute/canonical paths in index, we try to canonicalize candidate
             if let Ok(abs) = fs::canonicalize(&p) {
                 return self.index.files.get(&abs.to_string_lossy().to_string()).map(|n| n.id);
             }
             self.index.files.get(&s).map(|n| n.id)
        };

        if let Some(id) = check_path(candidate_base.clone()) { return Some(id); }

        for ext in extensions {
             if let Some(id) = check_path(candidate_base.with_extension(ext)) { return Some(id); }
        }
        
        let index_base = candidate_base.join("index");
        for ext in extensions {
             if let Some(id) = check_path(index_base.with_extension(ext)) { return Some(id); }
        }

        None
    }

    fn resolve_implicit_connections(&mut self) {
        self.index.file_dependencies.clear();
        self.index.inheritance.clear();

        // --- A. Build Inheritance Graph ---
        let raw_impls: Vec<(u32, Vec<String>)> = self.index.raw_implementations.iter()
            .map(|(k, v)| (*k, v.clone())).collect();

        for (child_id, parent_names) in raw_impls {
            for parent_name in parent_names {
                if let Some(parent_ids) = self.index.symbol_map.get(&parent_name) {
                    for &parent_id in parent_ids {
                        self.index.inheritance.entry(parent_id)
                            .or_default()
                            .push(child_id);
                    }
                }
            }
        }

        // --- B. Build Literal Bridge ---
        let mut global_string_map: HashMap<String, Vec<u32>> = HashMap::new();
        let entries: Vec<(u32, Vec<String>)> = self.index.raw_literals.iter()
            .map(|(k, v)| (*k, v.clone())).collect();

        for (src_id, literals) in entries {
            for lit in literals {
                if let Some(target_id) = self.resolve_import_path(src_id, &lit) {
                    if src_id != target_id {
                        self.index.file_dependencies.entry(src_id)
                            .or_default()
                            .push(target_id);
                    }
                }

                let is_path = lit.contains('/') || lit.contains('.');
                let is_constant = lit.len() > 5 && lit.chars().all(|c| c.is_uppercase() || c == '_');
                
                if is_path || is_constant {
                    global_string_map.entry(lit.clone()).or_default().push(src_id);
                }
            }
        }

        for (_, file_ids) in global_string_map {
            if file_ids.len() < 2 || file_ids.len() > 50 { continue; }
            for &f1 in &file_ids {
                for &f2 in &file_ids {
                    if f1 != f2 {
                        self.index.file_dependencies.entry(f1).or_default().push(f2);
                    }
                }
            }
        }
        
        for deps in self.index.file_dependencies.values_mut() {
            deps.sort(); deps.dedup();
        }
        for children in self.index.inheritance.values_mut() {
            children.sort(); children.dedup();
        }
    }
}