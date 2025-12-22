use rkyv::{Archive, Deserialize, Serialize};
use std::collections::HashMap;

pub type FileId = u32;
pub type SymbolId = u32;

#[derive(Archive, Deserialize, Serialize, Debug)]
#[archive(check_bytes)]
pub struct FileNode {
    pub id: FileId,
    pub path: String,
    pub hash: [u8; 32], // Blake3 hash to detect changes
}

#[derive(Archive, Deserialize, Serialize, Debug)]
#[archive(check_bytes)]
pub struct SymbolNode {
    pub id: SymbolId,
    pub file_id: FileId,
    pub name: String,
    pub kind: String, // "function", "class", "const"
    pub range_start: usize, // Byte offset in file
    pub range_end: usize,
    pub doc_comment: Option<String>,
}

#[derive(Archive, Deserialize, Serialize, Debug, Default)]
#[archive(check_bytes)]
pub struct WorkspaceIndex {
    // We map a file path to an ID
    pub files: HashMap<String, FileNode>,
    // We map a function name to a list of IDs (handling duplicates)
    pub symbol_map: HashMap<String, Vec<SymbolId>>,
    // The actual storage of symbols
    pub symbols: HashMap<SymbolId, SymbolNode>,
    // The dependency graph: Who calls who? (CallerID -> CalleeName)
    // Note: In Phase 1, we store names because resolving IDs is hard
    pub calls: HashMap<SymbolId, Vec<String>>, 
}