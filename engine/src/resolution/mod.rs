pub mod persistence;
pub mod utils;
pub mod resolvers;
pub mod scanner;
pub mod pipeline;

use crate::models::{ Edge, EdgeKind, SymbolId, WorkspaceIndex, SymbolIndex, FileId };
use crate::resolution::persistence::PersistenceManager;
use std::path::{ Path, PathBuf };
use std::collections::HashMap;

pub struct Indexer {
    // The Knowledge Graph (Persisted)
    pub index: WorkspaceIndex,
    // The Lookups (Rebuildable)
    pub lookup: SymbolIndex,
    // Runtime-only Reverse Graph (Target -> [Sources])
    pub reverse_graph: HashMap<SymbolId, Vec<Edge>>,

    // Transient map for O(1) lookup of Absolute Paths to File IDs.
    pub path_map: HashMap<PathBuf, FileId>,
    
    // Reverse map for O(1) lookup of File ID to Absolute Path.
    pub id_map: HashMap<FileId, PathBuf>,
}

impl Indexer {
    pub fn new() -> Self {
        let mut index = WorkspaceIndex::default();
        //Start IDs at 1 to reserve 0 for EXTERNAL_FILE_ID
        index.next_file_id = 1;
        index.next_symbol_id = 1;

        Self {
            index,
            lookup: SymbolIndex::default(),
            reverse_graph: HashMap::new(),
            path_map: HashMap::new(),
            id_map: HashMap::new(),
        }
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let manager = PersistenceManager::new();
        manager.save_index(&self.index, path)
    }

    pub fn load_from_file(path: &Path) -> anyhow::Result<Self> {
        let manager = PersistenceManager::new();
        let index = manager.load_index(path)?;

        let mut indexer = Self::new();
        indexer.index = index;
        indexer.rebuild_derived_indices();
        
        Ok(indexer)
    }

    fn rebuild_derived_indices(&mut self) {
        self.lookup.symbol_map.clear();
        self.lookup.file_to_module.clear();

        for sym in self.index.symbols.values() {
            self.lookup.symbol_map.entry(sym.name.clone()).or_default().push(sym.id);

            if sym.kind == crate::models::SymbolKind::Module {
                self.lookup.file_to_module.insert(sym.file_id, sym.id);
            }
        }

        self.lookup.file_imports = self.index.file_imports.clone();
        self.lookup.file_exports = self.index.file_exports.clone();
        self.build_reverse_graph();
    }

    pub fn add_edge(&mut self, source: SymbolId, target: SymbolId, kind: EdgeKind) {
        crate::resolution::resolvers::add_edge(&mut self.index, source, target, kind);
    }

    pub fn build_reverse_graph(&mut self) {
        self.reverse_graph.clear();
        for (&source, edges) in &self.index.graph {
            for edge in edges {
                self.reverse_graph.entry(edge.target_id).or_default().push(Edge {
                    target_id: source,
                    kind: edge.kind,
                });
            }
        }
    }

    pub fn get_impacted_files(&self, target_path: &Path) -> Vec<String> {
        // Note: target_path from test might be relative or absolute.
        // We canonicalize it to match path_map keys.
        let abs_target = std::fs::canonicalize(target_path).unwrap_or(target_path.to_path_buf());
        
        let target_id = match self.path_map.get(&abs_target) {
            Some(&id) => id,
            None => return vec![],
        };

        let mut impacted_paths = Vec::new();
        for (&source_file_id, dependencies) in &self.index.file_dependencies {
            if dependencies.contains(&target_id) {
                if let Some(source_node) = self.index.files.values().find(|f| f.id == source_file_id) {
                    impacted_paths.push(source_node.relative_path.clone());
                }
            }
        }
        impacted_paths
    }

    pub fn remove_root(&mut self, root_id: &str) {
        // 1. Remove from roots list
        self.index.roots.retain(|r| *r != root_id);

        // 2. Identify files to remove based on root_id field
        let keys_to_remove: Vec<String> = self.index.files
            .iter()
            .filter(|(_, node)| node.root_id == root_id)
            .map(|(k, _)| k.clone())
            .collect();

        // 3. Remove them
        let scanner = crate::resolution::scanner::FileScanner::new();
        for key in keys_to_remove {
            scanner.remove_file(&key, &mut self.index, &mut self.lookup);
        }
    }
}