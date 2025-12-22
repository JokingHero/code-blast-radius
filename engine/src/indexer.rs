use crate::schema::{WorkspaceIndex, FileNode, SymbolNode};
use crate::analyzer::analyze_source;
use crate::language::{get_language_configs, LanguageConfig};

use std::path::{Path, PathBuf};
use std::fs;
use std::collections::{HashMap, HashSet};
use walkdir::WalkDir;
use blake3;

pub struct Indexer {
    pub index: WorkspaceIndex,
    // We keep configs here so we don't fetch them every loop
    configs: HashMap<String, &'static LanguageConfig>, 
}

impl Indexer {
    pub fn new() -> Self {
        // Build a lookup map for extensions (rs -> RustConfig)
        let mut config_map = HashMap::new();
        for config in get_language_configs() {
            for ext in config.file_extensions {
                config_map.insert(ext.to_string(), config);
            }
        }

        Self { 
            index: WorkspaceIndex::default(),
            configs: config_map 
        }
    }

    // TODO: We will implement proper rkyv loading later.
    // For now, simple logic.
    pub fn load_from_file(_path: &Path) -> anyhow::Result<Self> {
        // Placeholder: Always return empty for now until we do serialization
        Ok(Self::new())
    }

    pub fn scan(&mut self, root: &Path) {
        println!("Indexing...");
        let mut seen_files = HashSet::new();

        for entry in WalkDir::new(root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file()) 
        {
            let path = entry.path();
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

            if let Some(config) = self.configs.get(ext) {
                // 1. Read file
                if let Ok(content) = fs::read_to_string(path) {
                    // 2. Hash it
                    let hash = blake3::hash(content.as_bytes());
                    let hash_bytes: [u8; 32] = hash.into();
                    let path_str = path.to_string_lossy().to_string();

                    seen_files.insert(path_str.clone());

                    // 3. Check Change
                    let needs_update = match self.index.files.get(&path_str) {
                        Some(node) => node.hash != hash_bytes,
                        None => true, // New file
                    };

                    if needs_update {
                        // println!("Parsing: {}", path_str);
                        self.update_file(&path_str, path, &content, hash_bytes, config);
                    } else {
                        // println!("Skipping (Cached): {}", path_str);
                    }
                }
            }
        }

        // 4. Cleanup deleted files
        // (If a file is in the index but not in `seen_files`, remove it)
        // This is tricky with our current Schema, let's leave for next iteration
    }

    fn update_file(
        &mut self, 
        path_key: &str, 
        path_obj: &Path, 
        content: &str, 
        hash: [u8; 32], 
        config: &LanguageConfig
    ) {
        // 1. Create/Update File Node
        // Simple ID generation strategy: hash the path or just increment? 
        // Let's use simple increment for now.
        let file_id = self.index.files.len() as u32; 
        
        self.index.files.insert(path_key.to_string(), FileNode {
            id: file_id,
            path: path_key.to_string(),
            hash,
        });

        // 2. Run Analyzer
        if let Ok(functions) = analyze_source(path_obj, content, config) {
            for func in functions {
                // Store in Symbol Table
                let symbol_id = self.index.symbols.len() as u32;
                
                let node = SymbolNode {
                    id: symbol_id,
                    file_id,
                    name: func.name.clone(),
                    kind: "function".to_string(),
                    range_start: 0, // Analyzer doesn't return byte ranges yet, todo
                    range_end: 0,
                    doc_comment: func.documentation,
                };

                self.index.symbols.insert(symbol_id, node);

                // Update Symbol Map (Name -> IDs)
                self.index.symbol_map.entry(func.name.clone())
                    .or_insert_with(Vec::new)
                    .push(symbol_id);

                // Update Call Graph (SymbolID -> Call Names)
                // Note: We are storing Call Strings, not IDs yet.
                self.index.calls.insert(symbol_id, func.calls);
            }
        }
    }

    // Helper to convert our robust Schema back to the simple `CodebaseGraph` 
    // needed by the current CLI logic (temporary bridge)
    pub fn export_graph(&self) -> crate::analyzer::CodebaseGraph {
        let mut graph = crate::analyzer::CodebaseGraph::new();

        for (_id, sym) in &self.index.symbols {
            // Find calls
            let calls = self.index.calls.get(&sym.id).cloned().unwrap_or_default();
            
            // Find file path
            // This is slow (linear scan), but fine for export
            // In real app we would map ID -> File directly
            if let Some(file_node) = self.index.files.values().find(|f| f.id == sym.file_id) {
                 graph.insert(sym.name.clone(), crate::analyzer::FunctionInfo {
                    name: sym.name.clone(),
                    file_path: PathBuf::from(&file_node.path),
                    source_code: "TODO: Store source or load on demand".to_string(), 
                    documentation: sym.doc_comment.clone(),
                    calls,
                 });
            }
        }
        graph
    }
}