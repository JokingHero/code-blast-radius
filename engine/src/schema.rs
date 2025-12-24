use rkyv::{Archive, Deserialize, Serialize};
use std::collections::HashMap;

pub type FileId = u32;
pub type SymbolId = u32;

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct ImportNode {
    pub name: String,
    pub source: String, 
    pub alias: Option<String>,
}

#[derive(Archive, Deserialize, Serialize, Debug)]
#[archive(check_bytes)]
pub struct FileNode {
    pub id: FileId,
    pub path: String,
    pub hash: [u8; 32],
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
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
    pub file_imports: HashMap<FileId, Vec<ImportNode>>,
    
    // Explicit Calls
    pub raw_calls: HashMap<SymbolId, Vec<String>>,
    pub resolved_calls: HashMap<SymbolId, Vec<SymbolId>>, 

    // Inheritance Graph (Parent Symbol ID -> List of Child Symbol IDs)
    pub inheritance: HashMap<SymbolId, Vec<SymbolId>>,

    // Implicit File Dependencies (File ID -> List of File IDs)
    pub file_dependencies: HashMap<FileId, Vec<FileId>>,

    // Temporary storage for linking phase (Map<FileId, ...>)
    pub raw_literals: HashMap<FileId, Vec<String>>,
    // Temporary storage for implementations (SymbolId -> Vec<ParentName>)
    pub raw_implementations: HashMap<SymbolId, Vec<String>>, 
}