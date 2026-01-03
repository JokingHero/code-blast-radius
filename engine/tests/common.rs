use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;
use blast_radius_engine::models::{SymbolId, EdgeKind, WorkspaceIndex};

pub struct TestWorkspace {
    _temp: TempDir, 
    pub path: PathBuf,
}

impl TestWorkspace {
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