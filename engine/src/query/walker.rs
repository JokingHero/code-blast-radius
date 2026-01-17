use std::collections::HashSet;
use crate::models::{BoundaryIndex, FileId};
use rayon::prelude::*;
use std::path::Path;

/// Performs a search over the BoundaryIndex to find related files.
pub struct JitWalker<'a> {
    index: &'a BoundaryIndex,
}

impl<'a> JitWalker<'a> {
    pub fn new(index: &'a BoundaryIndex) -> Self {
        Self { index }
    }

    pub fn walk_impact(&self, start_ids: &[FileId], max_depth: usize) -> Vec<FileId> {
        let mut visited = HashSet::new();
        let mut current_frontier: Vec<FileId> = start_ids.to_vec();
        let mut results = Vec::new();

        for &id in start_ids {
            visited.insert(id);
            results.push(id);
        }

        for _depth in 0..max_depth {
            if current_frontier.is_empty() {
                break;
            }

            // 1. Build Query (Targets we are looking for)
            let mut search_defs: HashSet<&str> = HashSet::new();
            let mut search_paths: Vec<&str> = Vec::new();

            for &id in &current_frontier {
                if let Some(f) = self.index.files.get(&id) {
                    for def in &f.defs {
                        search_defs.insert(def.name.as_str());
                    }
                    search_paths.push(f.path.as_str());
                }
            }

            // 2. Parallel Scan
            // We iterate over every file in the index (candidates) to see if they
            // import/reference anything in our 'search_paths' or 'search_defs'.
            let next_candidates: Vec<FileId> = self.index.files
                .par_iter()
                .map(|(_, f)| f)
                .filter(|candidate| {
                    // A. Logic Refs (Keep strict: code symbols are usually case-sensitive)
                    for reference in &candidate.symbol_refs {
                        if search_defs.contains(reference.as_str()) {
                            return true;
                        }
                    }

                    // B. Structural Imports (Make lenient: file paths vary by OS/Dev habit)
                    for import_str in &candidate.imports {
                        // 1. Monorepo Alias Resolution
                        let effective_search_str = if let Some((alias, target_dir)) = self.index.package_map
                            .iter()
                            .find(|(k, _)| import_str.starts_with(*k))
                        {
                            import_str.replace(alias, target_dir)
                        } else {
                            import_str.clone()
                        };

                        // 2. Normalize Relative Imports
                        // remove "./" prefix if present, so "./utils" becomes "utils"
                        let clean_search_str = effective_search_str
                            .strip_prefix("./")
                            .unwrap_or(&effective_search_str);

                        // 3. Prepare for Case-Insensitive Comparison
                        let import_lower = clean_search_str.to_lowercase();

                        for search_path in &search_paths {
                            let path_lower = search_path.to_lowercase();

                            // HEURISTIC MATCHING (Case-Insensitive)
                            
                            // Check 1: Does the path contain the import string?
                            if path_lower.contains(&import_lower) {
                                
                                // Sub-Check A: File Stem Match 
                                // Matches import "utils" to file ".../Utils.ts"
                                if let Some(stem) = Path::new(search_path).file_stem().and_then(|s| s.to_str()) {
                                    if stem.to_lowercase() == import_lower {
                                        return true;
                                    }
                                }

                                // Sub-Check B: Suffix Match
                                // Matches import "src/Utils" to file "src/Utils.ts"
                                if path_lower.ends_with(&import_lower) {
                                    return true;
                                }

                                // Sub-Check C: Path Segment Match
                                // Matches import "components/Button" to file "src/components/Button/index.tsx"
                                // Prevents simple substring matching (e.g. "auth" matching "author.ts") unless slashes involve hierarchy.
                                if import_lower.contains('/') {
                                    return true;
                                }
                            }
                        }
                    }
                    false
                })
                .map(|f| f.id)
                .collect();

            let mut next_frontier = Vec::new();
            for id in next_candidates {
                if visited.insert(id) {
                    results.push(id);
                    next_frontier.push(id);
                }
            }
            current_frontier = next_frontier;
        }

        results
    }
}