// File: engine/src/schema.rs
use rkyv::{Archive, Deserialize, Serialize};
use std::collections::HashMap;

pub type FileId = u32;
pub type SymbolId = u32;

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct ImportNode {
    // e.g., "add" in `import { add } from ...`
    pub name: String,
    // e.g., "./utils" or "express"
    pub source: String, 
    // e.g., "add_alias" in `import { add as add_alias } ...`
    // For now, we'll keep it simple and just use `name`
    pub alias: Option<String>,
}

#[derive(Archive, Deserialize, Serialize, Debug)]
#[archive(check_bytes)]
pub struct FileNode {
    pub id: FileId,
    pub path: String,
    pub hash: [u8; 32],
}

#[derive(Archive, Deserialize, Serialize, Debug)]
#[archive(check_bytes)]
pub struct SymbolNode {
    pub id: SymbolId,
    pub file_id: FileId,
    pub name: String,
    pub kind: String, 
    pub range_start: usize,
    pub range_end: usize,
    pub doc_comment: Option<String>,
}

#[derive(Archive, Deserialize, Serialize, Debug, Default)]
#[archive(check_bytes)]
pub struct WorkspaceIndex {
    pub files: HashMap<String, FileNode>,
    pub symbols: HashMap<SymbolId, SymbolNode>,
    pub symbol_map: HashMap<String, Vec<SymbolId>>,
    
    // What does this file import?
    pub file_imports: HashMap<FileId, Vec<ImportNode>>,

    // We keep this for the "Scan" phase (Raw strings)
    pub raw_calls: HashMap<SymbolId, Vec<String>>,

    // The "Linker" populates this (Resolved IDs)
    pub resolved_calls: HashMap<SymbolId, Vec<SymbolId>>, 
}