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

        Self { 
            index: WorkspaceIndex::default(),
            configs: config_map 
        }
    }

    /// Serializes the in-memory index to disk using rkyv
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        // Serialize to a bytes vector (rkyv handles alignment)
        let bytes = to_bytes::<_, 4096>(&self.index)
            .map_err(|e| anyhow::anyhow!("Serialization failed: {}", e))?;

        let mut file = File::create(path)?;
        file.write_all(&bytes)?;
        Ok(())
    }

    /// Memory-maps the file and deserializes it back into a WorkspaceIndex
    pub fn load_from_file(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }

        let file = File::open(path)?;
        // Safety: We assume the file is not modified externally while mapped.
        // For a CLI tool running in short bursts, this is generally acceptable.
        let mmap = unsafe { MmapOptions::new().map(&file)? };

        // 1. Check integrity (Zero-copy check)
        if let Err(e) = check_archived_root::<WorkspaceIndex>(&mmap[..]) {
            eprintln!("Index corrupted or incompatible, starting fresh: {}", e);
            return Ok(Self::new());
        }

        // 2. Deserialize to Heap (Deep Copy)
        // We do this because we need to Mutate the index (add new files/symbols).
        // If we only needed read-access, we could just use the mmap directly.
        let index: WorkspaceIndex = unsafe {
            rkyv::from_bytes_unchecked(&mmap[..])
                .map_err(|e| anyhow::anyhow!("Deserialization failed: {}", e))?
        };

        let mut config_map = HashMap::new();
        for config in get_language_configs() {
            for ext in config.file_extensions {
                config_map.insert(ext.to_string(), config);
            }
        }

        Ok(Self {
            index,
            configs: config_map,
        })
    }

    pub fn scan(&mut self, root: &Path) {
        let mut seen_files = HashSet::new();

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

                    seen_files.insert(path_str.clone());

                    // Check if file is new or changed
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
    }

    fn update_file(
        &mut self, 
        path_key: &str, 
        path_obj: &Path, 
        content: &str, 
        hash: [u8; 32], 
        config: &LanguageConfig
    ) {
        // 1. Get or Create File ID
        let file_id = if let Some(node) = self.index.files.get(path_key) {
            node.id
        } else {
            self.index.files.len() as u32
        };

        self.index.files.insert(path_key.to_string(), FileNode {
            id: file_id,
            path: path_key.to_string(),
            hash,
        });

        // 2. Parse Symbols
        // Note: Currently we append. In a real system, we'd need to clear 
        // old symbols associated with `file_id` before adding new ones.
        if let Ok(functions) = analyze_source(path_obj, content, config) {
            for func in functions {
                let symbol_id = self.index.symbols.len() as u32;
                
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
                    source_code: String::new(), // optimized out for graph
                    documentation: sym.doc_comment.clone(),
                    calls,
                 });
            }
        }
        graph
    }
}