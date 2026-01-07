use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;
use blast_radius_engine::models::{SymbolId, EdgeKind, WorkspaceIndex};
use blast_radius_engine::resolution::{Indexer, pipeline::Pipeline};

pub struct TestWorkspace {
    _temp: TempDir, 
    pub path: PathBuf,
}

impl TestWorkspace {
    #[allow(dead_code)] 
    pub fn new() -> Self {
        let temp = TempDir::new().expect("Failed to create temp dir");
        let path = temp.path().to_path_buf();
        Self { _temp: temp, path }
    }

    #[allow(dead_code)] 
    pub fn create_file(&self, relative_path: &str, content: &str) {
        let file_path = self.path.join(relative_path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).expect("Failed to create parent dirs");
        }
        let mut file = File::create(&file_path).expect("Failed to create file");
        file.write_all(content.as_bytes()).expect("Failed to write content");
    }
}

// --- HELPERS ---

/// Helper function to run a full scan + hydrate + resolve cycle for tests.
/// This replaces the old `common::run_pipeline(&mut indexer, &workspace.path)` pattern.
#[allow(dead_code)]
pub fn run_pipeline(indexer: &mut Indexer, workspace_path: &std::path::Path) {
    let mut pipeline = Pipeline::new();
    
    // Scan with a default root ID
    pipeline.scan(indexer, workspace_path, Some("root_1"));

    // Manual Hydration for Test Environment
    let mut active_roots_map = std::collections::HashMap::new();
    active_roots_map.insert("root_1".to_string(), workspace_path.to_path_buf());
    
    let (pm, im) = pipeline.hydrate_maps(&indexer.index, &active_roots_map);
    indexer.path_map = pm;
    indexer.id_map = im;

    // Run resolution
    let mut staging = pipeline.hydrate_staging(&indexer.index);
    
    // Prepare root paths vector for the resolver
    let active_roots_vec = vec![workspace_path.to_path_buf()];
    
    pipeline.resolve(indexer, &mut staging, &active_roots_vec);
}

#[allow(dead_code)]
pub fn get_calls(index: &WorkspaceIndex, source_id: SymbolId) -> Vec<SymbolId> {
    if let Some(edges) = index.graph.get(&source_id) {
        edges.iter()
            .filter(|e| e.kind == EdgeKind::Calls)
            .map(|e| e.target_id)
            .collect()
    } else {
        Vec::new()
    }
}

#[allow(dead_code)]
pub fn get_type_refs(index: &WorkspaceIndex, source_id: SymbolId) -> Vec<SymbolId> {
    if let Some(edges) = index.graph.get(&source_id) {
        edges.iter()
            .filter(|e| e.kind == EdgeKind::TypeReference)
            .map(|e| e.target_id)
            .collect()
    } else {
        Vec::new()
    }
}