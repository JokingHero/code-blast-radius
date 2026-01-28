use crate::workspace::WorkspaceManager;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResolvedPathDTO {
    pub original: String,
    pub relative_path: Option<String>,
    pub root_id: Option<String>,
    pub is_indexed: bool,
}

pub struct PathService;

impl PathService {
    /// Resolves a list of file paths (absolute or relative) against the loaded workspace roots.
    /// Determines if they exist inside a root and if they are currently indexed.
    pub fn resolve(manager: &WorkspaceManager, paths: Vec<String>) -> Vec<ResolvedPathDTO> {
        let mut results = Vec::new();
        let index = &manager.index;

        for path_str in paths {
            let raw_path = PathBuf::from(&path_str);
            
            // Normalize: Get absolute path if exists, otherwise keep raw
            let abs_path = if raw_path.exists() {
                std::fs::canonicalize(&raw_path).unwrap_or(raw_path)
            } else {
                raw_path
            };

            // Helper: Strip Windows UNC prefix (\\?\) for consistent matching
            let to_clean_path = |p: &PathBuf| -> PathBuf {
                let s = p.to_string_lossy().to_string();
                if s.starts_with(r"\\?\") { 
                    PathBuf::from(&s[4..]) 
                } else { 
                    p.clone() 
                }
            };

            let clean_lookup = to_clean_path(&abs_path);
            let mut found_match = false;

            // Check against all configured workspace roots
            for root in &manager.config.roots {
                let clean_root = to_clean_path(&root.path);

                if clean_lookup.starts_with(&clean_root) {
                    if let Ok(rel) = clean_lookup.strip_prefix(&clean_root) {
                        
                        // Normalize to Forward Slashes for Index Lookup (Engine standard)
                        let rel_str = rel.to_string_lossy().replace('\\', "/");
                        let mut indexed = false;

                        // 1. Direct Exact Match in Index
                        if index.files.values().any(|f| f.path == rel_str && f.root_id == root.id) {
                            indexed = true;
                        } 
                        // 2. Fallback: Fuzzy check via path_map (handles variations in separators or case)
                        else if let Some(ids) = index.path_map.get(&rel_str) {
                            // Ensure match belongs to THIS root
                            if ids.iter().any(|id| index.files.get(id).map_or(false, |f| f.root_id == root.id)) {
                                indexed = true;
                            }
                        }

                        results.push(ResolvedPathDTO {
                            original: path_str.clone(),
                            relative_path: Some(rel_str),
                            root_id: Some(root.id.clone()),
                            is_indexed: indexed,
                        });
                        found_match = true;
                        break; // Stop checking roots once found
                    }
                }
            }

            if !found_match {
                // Completely outside any known workspace root
                results.push(ResolvedPathDTO {
                    original: path_str,
                    relative_path: None,
                    root_id: None,
                    is_indexed: false,
                });
            }
        }
        results
    }
}