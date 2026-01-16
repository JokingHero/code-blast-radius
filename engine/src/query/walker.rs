use std::collections::HashSet;
use crate::models::{BoundaryIndex, FileId};
use rayon::prelude::*;

/// Performs a search over the BoundaryIndex to find related files.
/// 
/// Instead of following pre-computed graph edges, this calculates relationships
/// on-the-fly based on Definitions vs References.
pub struct JitWalker<'a> {
    index: &'a BoundaryIndex,
}

impl<'a> JitWalker<'a> {
    pub fn new(index: &'a BoundaryIndex) -> Self {
        Self { index }
    }

    /// Finds files that depend on the `start_ids` (Downstream analysis).
    /// 
    /// Logic:
    /// 1. Identify what `start_ids` provide (Definitions & Path).
    /// 2. Scan all files to see if they consume those Definitions or Import that Path.
    /// 3. Repeat for `depth` iterations.
    pub fn walk_impact(&self, start_ids: &[FileId], max_depth: usize) -> Vec<FileId> {
        let mut visited = HashSet::new();
        let mut current_frontier: Vec<FileId> = start_ids.to_vec();
        let mut results = Vec::new();

        // Mark initial seeds as visited
        for &id in start_ids {
            visited.insert(id);
            results.push(id);
        }

        for _depth in 0..max_depth {
            if current_frontier.is_empty() {
                break;
            }

            // 1. Build the Search Query for this step
            // Collect all definitions and paths from the current frontier
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

            // 2. PARALLEL SCAN of the entire world
            // This turns the O(N) loop into O(N / Cores)
            // Fix: Use index.files.par_iter() which gives (key, value) pairs
            let next_candidates: Vec<FileId> = self.index.files
                .par_iter() 
                .map(|(_, f)| f) // Drop the key
                .filter(|candidate| {
                    // Don't check files we've already added to results
                    // Note: We can't easily check 'visited' inside par_iter without a lock,
                    // so we do a cheap filter later.
                    
                    // A. Check Logic Refs
                    for reference in &candidate.symbol_refs {
                        if search_defs.contains(reference.as_str()) {
                            return true;
                        }
                    }

                    // B. Check Structural Imports
                    for import_str in &candidate.imports {
                         // Type inference fix: Explicitly iterate search_paths
                        for search_path in &search_paths {
                             if search_path.contains(import_str) {
                                // Simple heuristic: suffix match or path match
                                if search_path.ends_with(import_str) || 
                                   (search_path.contains(import_str) && import_str.contains('/')) {
                                    return true;
                                }
                            }
                        }
                    }
                    
                    false
                })
                .map(|f| f.id)
                .collect();

            // 3. Update State (Sequential part)
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