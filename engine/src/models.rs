use rkyv::{Archive, Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// --- Analysis Structs ---

#[derive(Archive, Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[archive(check_bytes)]
pub enum SymbolKind {
    Module,
    Function,
    Method,
    Container,
    Macro,
    MacroGenerated,
    Variable,
    External,
    Resource,
    Test,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub name: String,
    pub kind: SymbolKind,
    pub is_anonymous: bool,
    pub range_start: usize,
    pub range_end: usize,
    pub body_start: Option<usize>,
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

// --- Persistence Structs ---

pub type FileId = u32;
pub type SymbolId = u32;
/// Reserved ID for symbols that do not belong to a physical file in the workspace
/// (e.g., node_modules, cargo crates, built-ins).
pub const EXTERNAL_FILE_ID: FileId = 0;

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
    pub name: Option<String>,
    pub source: String,
}

#[derive(Archive, Deserialize, Serialize, Debug)]
#[archive(check_bytes)]
pub struct FileNode {
    pub id: FileId,
    pub path: String,
    pub hash: [u8; 32],
    pub is_test: bool,
    pub literals: Vec<String>,
    pub middleware_usage: Vec<String>,
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
    pub body_start: Option<usize>,
    pub doc_comment: Option<String>,
    pub return_type: Option<String>,
    pub is_test: bool,
    pub is_external: bool,
    pub external_source: Option<String>,
    pub decorators: Vec<String>, 
    pub routes: Vec<String>,
    pub calls: Vec<String>,
    pub type_refs: Vec<String>,
    pub fingerprints: HashMap<String, Vec<String>>,
    pub local_types: HashMap<String, String>,
    pub config_keys: Vec<String>,
    pub dispatched_actions: Vec<String>,
    pub handled_actions: Vec<String>,
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[archive(check_bytes)]
pub enum EdgeKind {
    Contains, Defines, Calls, Inherits, Implements, TypeReference,
    Imports, Constructs, Injects, Configures, Dispatches, Handles, Related,
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct Edge {
    pub target_id: SymbolId,
    pub kind: EdgeKind,
}

/// Transient raw data used during resolution, never saved to disk.
#[derive(Debug, Default)]
pub struct StagingArea {
    pub raw_calls: HashMap<SymbolId, Vec<String>>,
    pub raw_literals: HashMap<FileId, Vec<String>>,
    pub raw_implementations: HashMap<SymbolId, Vec<String>>, 
    pub fingerprints: HashMap<SymbolId, HashMap<String, Vec<String>>>,
    pub container_methods: HashMap<SymbolId, HashSet<String>>,
    pub local_variable_types: HashMap<SymbolId, HashMap<String, String>>,
    pub symbol_config_refs: HashMap<SymbolId, Vec<String>>,
    pub raw_type_refs: HashMap<SymbolId, Vec<String>>,
    pub raw_decorators: HashMap<SymbolId, Vec<String>>,
    pub raw_action_dispatches: HashMap<SymbolId, Vec<String>>,
    pub raw_action_handlers: HashMap<SymbolId, Vec<String>>,
    pub raw_middleware_usage: HashMap<FileId, Vec<String>>,
}

/// Lookup indices for O(1) access. Rebuilt on load or populated during scan.
#[derive(Debug, Default)]
pub struct SymbolIndex {
    pub symbol_map: HashMap<String, Vec<SymbolId>>, // Name -> [IDs]
    pub file_imports: HashMap<FileId, Vec<ImportNode>>,
    pub file_exports: HashMap<FileId, Vec<ExportNode>>, 
    pub implicit_routes: HashMap<String, SymbolId>,
    pub import_mappings: HashMap<String, String>,
    pub package_path_map: HashMap<String, String>,
    pub external_symbols: HashSet<String>,
    pub external_packages: HashSet<String>,
    pub config_definitions: HashMap<String, Vec<SymbolId>>, 
}

#[derive(Archive, Deserialize, Serialize, Debug, Default)]
#[archive(check_bytes)]
pub struct WorkspaceIndex {
    // Metadata
    pub next_file_id: u32,
    pub next_symbol_id: u32,
    pub roots: Vec<String>,
    
    // Core Data
    pub files: HashMap<String, FileNode>,
    pub symbols: HashMap<SymbolId, SymbolNode>,
    pub file_imports: HashMap<FileId, Vec<ImportNode>>,
    pub file_exports: HashMap<FileId, Vec<ExportNode>>,
    pub graph: HashMap<SymbolId, Vec<Edge>>,

    // This is technically a "cached result" but valuable enough to persist 
    // for impact analysis without re-resolving.
    pub file_dependencies: HashMap<FileId, Vec<FileId>>,
}