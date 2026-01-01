pub mod persistence;
pub mod utils;
pub mod resolvers;
pub mod scanner;
pub mod pipeline;

use crate::models::{
    Edge, EdgeKind, SymbolId, WorkspaceIndex, SymbolIndex
};
use crate::resolution::persistence::PersistenceManager;

use std::path::Path;
use std::collections::HashMap;

pub struct Indexer {
    // The Knowledge Graph (Persisted)
    pub index: WorkspaceIndex,
    // The Lookups (Rebuildable)
    pub lookup: SymbolIndex,
    
    // Runtime-only Reverse Graph (Target -> [Sources])
    pub reverse_graph: HashMap<SymbolId, Vec<Edge>>,
}

impl Indexer {
    pub fn new() -> Self {
        Self {
            index: WorkspaceIndex::default(),
            lookup: SymbolIndex::default(),
            reverse_graph: HashMap::new(),
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
        for sym in self.index.symbols.values() {
             self.lookup.symbol_map.entry(sym.name.clone()).or_default().push(sym.id);
        }
        self.build_reverse_graph();
    }

    // Helper for manual edge addition (mostly used by tests/scanners)
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
        let target_key = utils::to_index_path(target_path);
        let target_id = match self.index.files.get(&target_key) {
            Some(node) => node.id, None => return vec![],
        };
        let mut impacted_paths = Vec::new();
        for (&source_file_id, dependencies) in &self.index.file_dependencies {
            if dependencies.contains(&target_id) {
                if let Some(source_node) = self.index.files.values().find(|f| f.id == source_file_id) {
                    impacted_paths.push(source_node.path.clone());
                }
            }
        }
        impacted_paths
    }
}