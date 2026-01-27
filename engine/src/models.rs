use rkyv::{Archive, Deserialize, Serialize};
use std::collections::HashMap;

pub type FileId = u32;

/// Simplified classification for UI icons and skeletal logic.
#[derive(Archive, Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[archive(check_bytes)]
pub enum SymbolKind {
    File,
    Module,
    Function,
    Class,
    Interface,
    Variable,
    Method,
    Export,
    Unknown,
}

/// A simplified Definition found within a file.
/// We store ranges immediately to avoid re-parsing during the "Run/Recipe" phase.
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct Definition {
    pub name: String,
    pub kind: SymbolKind,

    // Start/End byte offsets for the entire definition (e.g., "fn foo() { ... }")
    pub range: (usize, usize),

    // Start/End byte offsets for the body to be skeletonized (e.g., "{ ... }")
    // If None, the whole definition is atomic/one-line or cannot be skeletonized.
    pub body_range: Option<(usize, usize)>,
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone, Default)]
#[archive(check_bytes)]
pub struct FrameworkHint {
    pub key: String,           // e.g., "@Controller", "render", "route"
    pub value: String,         // e.g., "/api/users", "user_view", "{ selector: 'app-root' }"
    pub range: (usize, usize), // Where it was found (for debugging/highlighting)
}

/// The atomic unit of our index.
/// Represents one file and its public boundary (definitions and outgoing references).
#[derive(Archive, Deserialize, Serialize, Debug, Clone, Default)]
#[archive(check_bytes)]
pub struct FileBoundary {
    pub id: FileId,

    // Relative path from the workspace root (e.g., "src/utils.rs")
    pub path: String,

    // Tracks which root config this file belongs to (for multi-root workspaces)
    pub root_id: String,

    // Blake3 hash of the file content for change detection
    pub hash: [u8; 32],
    
    pub token_count: u32,

    // 1. What does this file define? (The API)
    pub defs: Vec<Definition>,

    // 2. What files does it explicitly import? (Structural Dependencies)
    // Stored as raw strings from source (e.g., "./utils", "react", "crate::models")
    pub imports: Vec<String>,

    // 3. What symbols does it mention? (Logical Dependencies)
    // Extracted identifiers from the code flow (e.g., "User", "AuthService", "login")
    pub symbol_refs: Vec<String>,

    // Raw string literals found in the file (e.g. "/api/users", "GET")
    // These are candidates for "Synthetic Reference" promotion.
    pub literals: Vec<String>,

    // Logical definitions inferred from the file path/structure
    // e.g. "route:/api/users"
    pub synthetic_defs: Vec<String>,

    // Carries framework-specific raw data extracted during parsing.
    // The parser doesn't know what it means, it just knows it looks interesting.
    pub framework_hints: Vec<FrameworkHint>,
}

/// The Main Index.
/// Replaces the old Graph. It is essentially a flat list of files plus
/// two inverted indices (HashMaps) for O(1) lookups.
#[derive(Archive, Deserialize, Serialize, Debug, Default)]
#[archive(check_bytes)]
pub struct BoundaryIndex {
    // Stability: Ensures consistent IDs across incremental scans
    pub next_file_id: u32,

    // The Source of Truth
    pub files: HashMap<FileId, FileBoundary>,

    // The "Yellow Pages" (Inverted Index for Definitions)
    // Key: Symbol Name (e.g., "AuthService")
    // Value: List of FileIds that define this symbol
    pub symbol_map: HashMap<String, Vec<FileId>>,

    // Path Suffix Map (Inverted Index for Imports)
    // Allows fuzzy resolution of imports like "./utils" -> "src/utils.ts"
    // Key: Path segments (e.g., "utils.ts", "src/utils.ts")
    // Value: List of matching FileIds
    pub path_map: HashMap<String, Vec<FileId>>,
    pub package_map: HashMap<String, String>,

    // Key: "Significant Token" (lowercase stem of import or symbol).
    // Value: List of files that contain this token in their imports or refs.
    pub usage_map: HashMap<String, Vec<FileId>>,

    // Maps prefixes ("@components/") to relative paths ("src/components/")
    pub alias_map: HashMap<String, String>,
}

impl BoundaryIndex {
    pub fn new() -> Self {
        Self {
            next_file_id: 1, // Start at 1, 0 is reserved
            files: HashMap::new(),
            symbol_map: HashMap::new(),
            path_map: HashMap::new(),
            package_map: HashMap::new(),
            usage_map: HashMap::new(),
            alias_map: HashMap::new(),
        }
    }
}
