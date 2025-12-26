use rkyv::{Archive, Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub type FileId = u32;
pub type SymbolId = u32;

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct ImportNode {
    pub name: String,
    pub source: String, 
    pub alias: Option<String>,
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct ExportNode {
    pub name: Option<String>, // Some("myFunc") for named, None for `*`
    pub source: String,       // "./math"
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct ConfigUsage {
    pub key: String,       // "OPENAI_KEY"
    pub range_start: usize,
    pub range_end: usize,
}

#[derive(Archive, Deserialize, Serialize, Debug)]
#[archive(check_bytes)]
pub struct FileNode {
    pub id: FileId,
    pub path: String,
    pub hash: [u8; 32],
    pub is_test: bool,
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct SymbolNode {
    pub id: SymbolId,
    pub file_id: FileId,
    pub parent_id: Option<SymbolId>,
    pub name: String,
    pub kind: String, 
    pub range_start: usize,
    pub range_end: usize,
    pub doc_comment: Option<String>,
    pub return_type: Option<String>,
    pub is_test: bool,
    pub is_external: bool,      // Is this from node_modules or a std lib?
    pub external_source: Option<String>, // e.g., "axios" or "fs"
}

#[derive(Archive, Deserialize, Serialize, Debug, Default)]
#[archive(check_bytes)]
pub struct WorkspaceIndex {
    pub next_file_id: u32,
    pub next_symbol_id: u32,
    pub roots: Vec<String>, // Track which folders are indexed
    pub files: HashMap<String, FileNode>,
    pub symbols: HashMap<SymbolId, SymbolNode>,
    pub symbol_map: HashMap<String, Vec<SymbolId>>,
    pub file_imports: HashMap<FileId, Vec<ImportNode>>,
    pub file_exports: HashMap<FileId, Vec<ExportNode>>, 
    // Mapping of Container (Class/Interface) -> Set of method names it owns
    pub container_methods: HashMap<SymbolId, HashSet<String>>,
    // Mapping of Function -> (Variable Name -> Set of Methods called on it)
    // Example: "reset_func_id" -> { "device": ["stop", "start"] }
    pub fingerprints: HashMap<SymbolId, HashMap<String, Vec<String>>>,
     // Mapping of FunctionSymbolID -> (VariableName -> TypeName)
    // e.g., "main_id" -> { "user": "User" }
    pub local_variable_types: HashMap<SymbolId, HashMap<String, String>>,
    
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

    // Mapping of SymbolId -> List of Config Keys it uses
    pub symbol_config_refs: HashMap<SymbolId, Vec<String>>,
    
    // Mapping of Config Key -> List of Locations where it's defined (e.g., .env, config.json)
    pub config_definitions: HashMap<String, Vec<SymbolId>>, 

    pub external_symbols: HashSet<String>, // Track known external modules used
    pub external_packages: HashSet<String>, // List of detected external libraries (e.g., "react", "serde", "numpy")
}