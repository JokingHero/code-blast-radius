use std::collections::HashSet;
use crate::models::{BoundaryIndex, FileId};
use std::path::Path;

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

            // 1. Build Query & Identify "Target Anchors"
            // We need to know which tokens point TO the files in our frontier.
            let mut search_defs: HashSet<&str> = HashSet::new();
            let mut search_paths: Vec<&str> = Vec::new();
            
            // New: Tokens that would be used to import/reference these files
            let mut target_anchors: HashSet<String> = HashSet::new();

            for &id in &current_frontier {
                if let Some(f) = self.index.files.get(&id) {
                    // A. Path Anchors
                    search_paths.push(f.path.as_str());
                    
                    // Logic: If I am "src/utils.ts", people import me via "utils".
                    if let Some(stem) = extract_file_stem(&f.path) {
                        target_anchors.insert(stem.clone());
                        
                        // FIX for Index/Mod files:
                        // If I am "src/auth/index.ts", people import me via "auth".
                        if is_generic_filename(&stem) {
                            if let Some(parent) = extract_parent_dir_name(&f.path) {
                                target_anchors.insert(parent);
                            }
                        }
                    }

                    // B. Symbol Anchors
                    // If I define "AuthService", people reference me via "authservice".
                    for def in &f.defs {
                        search_defs.insert(def.name.as_str());
                        target_anchors.insert(def.name.to_lowercase());
                    }
                }
            }

            // 2. Candidate Selection (The Speedup)
            // Instead of scanning all files, we grab files that match our anchors from the usage_map.
            let mut candidate_ids: HashSet<FileId> = HashSet::new();
            
            for anchor in target_anchors {
                if let Some(ids) = self.index.usage_map.get(&anchor) {
                    candidate_ids.extend(ids);
                }
            }
            
            // Edge case: If usage_map is empty (e.g. fresh scan issue?), 
            // or we have zero anchors (unlikely), strictly we define no candidates.
            // But we filter out files we've already visited to save time.
            let candidates_to_check: Vec<FileId> = candidate_ids
                .into_iter()
                .filter(|id| !visited.contains(id))
                .collect();

            // 3. Verification (The Accuracy)
            // We run the expensive heuristic logic ONLY on the candidates.
            // Note: We removed 'par_iter' because candidates_to_check is likely small (~10-50 files).
            // Overhead of thread spawning might exceed checking cost. Standard iter is fine.
            let mut next_frontier = Vec::new();

            for id in candidates_to_check {
                 let candidate = match self.index.files.get(&id) {
                     Some(c) => c,
                     None => continue,
                 };

                 let mut is_match = false;

                 // A. Logic Refs (Keep strict: code symbols are usually case-sensitive)
                 for reference in &candidate.symbol_refs {
                     if search_defs.contains(reference.as_str()) {
                         is_match = true; 
                         break;
                     }
                 }

                 if !is_match {
                     // B. Structural Imports (Make lenient)
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
                        let clean_search_str = effective_search_str
                            .strip_prefix("./")
                            .unwrap_or(&effective_search_str);

                        let import_lower = clean_search_str.to_lowercase();

                        for search_path in &search_paths {
                            let path_lower = search_path.to_lowercase();

                            // Heuristic Checks
                            if path_lower.contains(&import_lower) {
                                // Sub-Check A: File Stem Match 
                                if let Some(stem) = extract_file_stem(search_path) {
                                    if stem == import_lower {
                                        is_match = true; break;
                                    }
                                }
                                
                                // Sub-Check B: Suffix Match
                                if path_lower.ends_with(&import_lower) {
                                    is_match = true; break;
                                }

                                // Sub-Check C: Path Segment Match
                                if import_lower.contains('/') {
                                    is_match = true; break;
                                }
                                
                                // Sub-Check D: Directory Import (Index file support)
                                // If search_path is "auth/index.ts" and import is "auth"
                                if is_generic_filename_path(search_path) {
                                    if let Some(parent) = extract_parent_dir_name(search_path) {
                                        if parent == import_lower {
                                            is_match = true; break;
                                        }
                                    }
                                }
                            }
                        }
                        if is_match { break; }
                    }
                 }

                 if is_match {
                     if visited.insert(id) {
                         results.push(id);
                         next_frontier.push(id);
                     }
                 }
            }
            
            current_frontier = next_frontier;
        }

        results
    }

    pub fn walk_dependencies(&self, start_ids: &[FileId], max_depth: usize) -> Vec<FileId> {
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

            let mut next_frontier = Vec::new();

            for &id in &current_frontier {
                if let Some(file) = self.index.files.get(&id) {
                    let file_dir = std::path::Path::new(&file.path).parent().unwrap_or(std::path::Path::new(""));

                    // 1. Resolve Structural Imports
                    for import_str in &file.imports {
                        let clean = import_str.trim_matches(|c| c == '"' || c == '\'');
                        
                        let mut candidates = Vec::new();

                        // A. Rust Crate Aliases
                        if clean.starts_with("crate::") {
                            let path = clean.replace("crate::", "src/").replace("::", "/");
                            candidates.push(path);
                        } 
                        // B. Relative Imports
                        else if clean.starts_with('.') {
                            let joined = file_dir.join(clean);
                            if let Some(normalized) = normalize_path_simple(&joined) {
                                candidates.push(normalized);
                            }
                        } 
                        // C. Absolute / Aliased / Package Imports
                        else {
                            // 1. Standard (e.g. "react")
                            candidates.push(clean.to_string());
                            
                            // 2. Monorepo Packages (package.json "name")
                            // e.g. "@my-org/ui" -> "packages/ui"
                            for (pkg_name, pkg_path) in &self.index.package_map {
                                if clean.starts_with(pkg_name) {
                                    let resolved = clean.replace(pkg_name, pkg_path);
                                    candidates.push(resolved);
                                }
                            }

                            // 3. TSConfig Aliases (tsconfig.json "paths") --- THIS IS THE FINAL ADDITION
                            // e.g. "@/" -> "src/"
                            for (alias_key, target_path) in &self.index.alias_map {
                                if clean.starts_with(alias_key) {
                                    let resolved = clean.replace(alias_key, target_path);
                                    candidates.push(resolved);
                                }
                            }
                        }

                        // D. Check Index
                        let extensions = ["", ".ts", ".tsx", ".js", ".jsx", ".rs", ".py", ".java", ".go", "/index.ts", "/index.js"];
                        
                        for base_path in candidates {
                            for ext in extensions.iter() {
                                let probe = format!("{}{}", base_path, ext);
                                
                                if let Some(target_ids) = self.index.path_map.get(&probe) {
                                    for &target_id in target_ids {
                                        if visited.insert(target_id) {
                                            results.push(target_id);
                                            next_frontier.push(target_id);
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // 2. Resolve Logical Symbols (Existing logic)
                    for symbol_ref in &file.symbol_refs {
                        if let Some(target_ids) = self.index.symbol_map.get(symbol_ref) {
                            for &target_id in target_ids {
                                if target_id == id { continue; }
                                if visited.insert(target_id) {
                                    results.push(target_id);
                                    next_frontier.push(target_id);
                                }
                            }
                        }
                    }
                }
            }
            current_frontier = next_frontier;
        }

        results
    }
}

// --- Helpers consistent with Scanner ---

fn extract_file_stem(path: &str) -> Option<String> {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
}

fn extract_parent_dir_name(path: &str) -> Option<String> {
    Path::new(path)
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
}

fn is_generic_filename(stem: &str) -> bool {
    matches!(stem, "index" | "mod" | "__init__" | "main" | "lib")
}

fn is_generic_filename_path(path: &str) -> bool {
    if let Some(stem) = extract_file_stem(path) {
        is_generic_filename(&stem)
    } else {
        false
    }
}

fn normalize_path_simple(path: &Path) -> Option<String> {
    use std::path::Component;
    let mut components = Vec::new();
    
    for component in path.components() {
        match component {
            Component::Normal(c) => components.push(c.to_str()?),
            Component::ParentDir => { components.pop(); }, // Go back one level
            Component::CurDir => {}, // Ignore "."
            _ => {}
        }
    }
    
    Some(components.join("/"))
}