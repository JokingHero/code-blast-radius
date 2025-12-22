use crate::schema::{WorkspaceIndex, FileNode, SymbolNode};
use std::path::Path;
use std::fs;

pub struct Indexer {
    pub index: WorkspaceIndex,
}

impl Indexer {
    pub fn new() -> Self {
        Self { index: WorkspaceIndex::default() }
    }

    // 1. Load from disk (Memory Map)
    pub fn load_from_file(path: &Path) -> anyhow::Result<Self> {
        // Implement rkyv loading here
        // If file doesn't exist, return new()
        Ok(Self::new())
    }

    // 2. Scan folder
    pub fn scan(&mut self, root: &Path) {
        // Use WalkDir
        // For each file:
        //   Calculate Blake3 Hash
        //   If hash == self.index.files[path].hash -> SKIP PARSING (Fast!)
        //   Else -> Parse with Tree-sitter -> Update Index
    }
    
    // 3. Save to disk
    pub fn save(&self, path: &Path) {
        // Implement rkyv saving
    }
}