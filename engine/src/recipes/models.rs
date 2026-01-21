use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipe {
    pub name: String,
    pub description: Option<String>,
    pub operations: Vec<RecipeOperation>,
    // Key: Relative file path (e.g., "src/utils.ts") -> Transform
    pub transforms: HashMap<String, FileTransform>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "params")]
pub enum RecipeOperation {
    /// Add files where the path matches the glob pattern
    AddFiles { pattern: String },
    /// Remove files where the path matches the glob pattern
    RemoveFiles { pattern: String },
    /// Select files part of the blast radius of a symbol
    BlastRadius {
        symbol: String,
        max_depth: u32,
        exclude_tests: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", content = "symbols")]
pub enum FileTransform {
    /// Hide bodies of ONLY these symbols
    Skeletonize(Vec<String>),
    /// Hide bodies of ALL symbols EXCEPT these (Focus Mode)
    FocusOn(Vec<String>),
}
