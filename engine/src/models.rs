use rkyv::{Archive, Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// --- Analysis Structs (Used during extraction phase) ---

#[derive(Archive, Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[archive(check_bytes)]
pub enum SymbolKind {
    Module,
    Function,
    Method,
    Container,      // Classes, Interfaces, Structs, Impls
    Macro,
    MacroGenerated,
    Variable,       // const x = ...
    External,       // Imported from libraries
    Resource,       // Infrastructure (Terraform/Cloud)
    Test,           // Explicit test functions
    Unknown,
}

#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub name: String,
    pub kind: SymbolKind,
    pub is_anonymous: bool,
    pub range_start: usize,
    pub range_end: usize,
    pub source_code: String,
    pub documentation: Option<String>,
    pub calls: Vec<String>,
    pub type_refs: Vec<String>,
    pub decorators: Vec<String>,
    pub dispatched_actions: Vec<String>,
    pub handled_actions: Vec<String>,
    pub fingerprints: HashMap<String, Vec<String>>,
    pub return_type: Option<String>,
    pub local_types: HashMap<String, String>,
    pub local_assigns: HashMap<String, String>,
    pub config_keys: Vec<String>,
    pub routes: Vec<String>,
}

pub struct FileAnalysis {
    pub functions: Vec<FunctionInfo>,
    pub imports: Vec<ImportNode>,
    pub exports: Vec<ExportNode>,
    pub literals: Vec<String>,
    pub implementations: Vec<(String, String)>,
    pub global_vars: HashMap<String, String>,
    pub middleware_usage: Vec<String>,
    pub defined_routes: Vec<String>,
}

// --- Persistence Structs (Used for Indexing/Linking) ---

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
    pub kind: SymbolKind,
    pub range_start: usize,
    pub range_end: usize,
    pub doc_comment: Option<String>,
    pub return_type: Option<String>,
    pub is_test: bool,
    pub is_external: bool,      // Is this from node_modules or a std lib?
    pub external_source: Option<String>, // e.g., "axios" or "fs"
    pub decorators: Vec<String>, 
    pub routes: Vec<String>,
}

// 1. Define the specific nature of the relationship
#[derive(Archive, Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[archive(check_bytes)]
pub enum EdgeKind {
    // Structural
    Contains,      // Parent -> Child (e.g. Class -> Method)
    Defines,       // File -> Module
    
    // Logic
    Calls,         // Function -> Function
    
    // Type System
    Inherits,      // Child Class -> Parent Class
    Implements,    // Class -> Interface (Renamed from Implement for consistency)
    TypeReference, // Function -> Type (arg/return), Variable -> Type
    
    // Meta / Frameworks
    Imports,       // Module -> Module (File dependency) (Renamed from Import)
    Constructs,    // Function -> Class (new Foo())
    Injects,       // DI Container -> Service
    Configures,    // Config Key -> Symbol
    
    // Event/State
    Dispatches,    // Function -> Redux Action / Event
    Handles,       // Redux Reducer / Listener -> Action
    
    // Generic Fallback
    Related, 
}

// 2. The Edge definition
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct Edge {
    pub target_id: SymbolId,
    pub kind: EdgeKind,
}

/// Holds raw data extracted during the scan phase. 
/// These fields are transient in nature: they are used to infer edges 
/// but are not strictly required for traversing the final graph.
#[derive(Archive, Deserialize, Serialize, Debug, Default, Clone)]
#[archive(check_bytes)]
pub struct StagingArea {
    pub container_methods: HashMap<SymbolId, HashSet<String>>,
    pub fingerprints: HashMap<SymbolId, HashMap<String, Vec<String>>>,
    pub local_variable_types: HashMap<SymbolId, HashMap<String, String>>,
    pub raw_calls: HashMap<SymbolId, Vec<String>>,
    pub raw_literals: HashMap<FileId, Vec<String>>,
    pub raw_implementations: HashMap<SymbolId, Vec<String>>, 
    pub symbol_config_refs: HashMap<SymbolId, Vec<String>>,
    pub raw_type_refs: HashMap<SymbolId, Vec<String>>,
    pub raw_decorators: HashMap<SymbolId, Vec<String>>,
    pub raw_action_dispatches: HashMap<SymbolId, Vec<String>>,
    pub raw_action_handlers: HashMap<SymbolId, Vec<String>>,
    pub raw_middleware_usage: HashMap<FileId, Vec<String>>,
}

/// Holds lookup maps for fast resolution of names, imports, and paths.
#[derive(Archive, Deserialize, Serialize, Debug, Default, Clone)]
#[archive(check_bytes)]
pub struct LookupTable {
    pub symbol_map: HashMap<String, Vec<SymbolId>>, // Name -> [IDs]
    pub file_imports: HashMap<FileId, Vec<ImportNode>>,
    pub file_exports: HashMap<FileId, Vec<ExportNode>>, 
    pub implicit_routes: HashMap<String, SymbolId>, // Route -> Symbol
    pub import_mappings: HashMap<String, String>,   // Alias -> Path
    pub package_path_map: HashMap<String, String>,  // Package -> Path
    pub external_symbols: HashSet<String>,
    pub external_packages: HashSet<String>,
    pub config_definitions: HashMap<String, Vec<SymbolId>>, 
}

#[derive(Archive, Deserialize, Serialize, Debug, Default)]
#[archive(check_bytes)]
pub struct WorkspaceIndex {
    // Metadata & Identifiers
    pub next_file_id: u32,
    pub next_symbol_id: u32,
    pub roots: Vec<String>,
    
    // Primary Data Stores (The "Knowledge Graph")
    pub files: HashMap<String, FileNode>,
    pub symbols: HashMap<SymbolId, SymbolNode>,

    // The Unified Graph (Adjacency List)
    // Maps Source Symbol -> List of Outgoing Edges
    pub graph: HashMap<SymbolId, Vec<Edge>>,

    // -- Sub-Structs for Separation of Concerns --
    
    // Acceleration structures for O(1) lookups
    pub lookup: LookupTable,

    // Raw/Intermediate Data used during resolution logic
    pub staging: StagingArea,

    // Legacy/Cache fields (maintained for backward compat or specific caches)
    pub file_dependencies: HashMap<FileId, Vec<FileId>>,
}