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

    /// Finds files that depend ON the start_ids.
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
            let mut search_defs: HashSet<&str> = HashSet::new();
            let mut search_paths: Vec<&str> = Vec::new();
            let mut target_anchors: HashSet<String> = HashSet::new();

            for &id in &current_frontier {
                if let Some(f) = self.index.files.get(&id) {
                    search_paths.push(f.path.as_str());
                    
                    if let Some(stem) = extract_file_stem(&f.path) {
                        target_anchors.insert(stem.clone());
                        if is_generic_filename(&stem) {
                            if let Some(parent) = extract_parent_dir_name(&f.path) {
                                target_anchors.insert(parent);
                            }
                        }
                    }

                    for def in &f.defs {
                        search_defs.insert(def.name.as_str());
                        target_anchors.insert(def.name.to_lowercase());
                    }

                    for syn_def in &f.synthetic_defs {
                        search_defs.insert(syn_def.as_str());
                        target_anchors.insert(syn_def.to_lowercase());
                        if let Some(val) = extract_value_from_synthetic(syn_def) {
                            target_anchors.insert(val.to_lowercase());
                        }
                    }
                }
            }

            // 2. Candidate Selection
            let mut candidate_ids: HashSet<FileId> = HashSet::new();
            for anchor in target_anchors {
                if let Some(ids) = self.index.usage_map.get(&anchor) {
                    candidate_ids.extend(ids);
                }
            }
            
            let candidates_to_check: Vec<FileId> = candidate_ids
                .into_iter()
                .filter(|id| !visited.contains(id))
                .collect();

            // 3. Verification
            let mut next_frontier = Vec::new();

            for id in candidates_to_check {
                 let candidate = match self.index.files.get(&id) {
                     Some(c) => c,
                     None => continue,
                 };

                 let mut is_match = false;

                 // A. Logic Refs
                 for reference in &candidate.symbol_refs {
                     if search_defs.contains(reference.as_str()) {
                         is_match = true; break;
                     }
                 }

                 // B. Literals (Synthetic Defs)
                 if !is_match {
                     for literal in &candidate.literals {
                         if search_defs.contains(literal.as_str()) {
                             is_match = true; break;
                         }
                         let prefixes = ["route:GET:", "route:POST:", "html:tag:", "di:", "view:"];
                         for prefix in prefixes {
                             let probe = format!("{}{}", prefix, literal);
                             if search_defs.contains(probe.as_str()) {
                                 is_match = true; break;
                             }
                         }
                         if is_match { break; }
                     }
                 }

                 // C. Structural Imports
                 if !is_match {
                     for import_str in &candidate.imports {
                        // Monorepo/Package Map
                        let effective_search_str = if let Some((alias, target_dir)) = self.index.package_map
                            .iter()
                            .find(|(k, _)| import_str.starts_with(*k))
                        {
                            import_str.replace(alias, target_dir)
                        } else {
                            import_str.clone()
                        };

                        let clean_search_str = effective_search_str
                            .strip_prefix("./")
                            .unwrap_or(&effective_search_str);

                        let import_lower = clean_search_str.to_lowercase();

                        for search_path in &search_paths {
                            let path_lower = search_path.to_lowercase();

                            if path_lower.contains(&import_lower) {
                                // Sub-Check A: File Stem Match 
                                if let Some(stem) = extract_file_stem(search_path) {
                                    if stem == import_lower {
                                        is_match = true; break;
                                    }
                                }
                                
                                // Sub-Check B: Suffix Match (e.g. import "style.css")
                                if path_lower.ends_with(&import_lower) {
                                    is_match = true; break;
                                }

                                // Sub-Check C: Directory Import (Index file)
                                if is_generic_filename_path(search_path) {
                                    if let Some(parent) = extract_parent_dir_name(search_path) {
                                        if parent == import_lower {
                                            is_match = true; break;
                                        }
                                    }
                                }

                                // Sub-Check D: Path Without Extension Match (Fix for Monorepo Test)
                                // e.g. import "src/Button" matches "src/Button.tsx"
                                if let Some(dot_index) = path_lower.rfind('.') {
                                    let path_no_ext = &path_lower[..dot_index];
                                    if path_no_ext.ends_with(&import_lower) {
                                         // Boundary check: ensure we matched a full segment
                                         // "packages/ui/src/button" ends with "button" -> OK
                                         // "packages/ui/src/button" ends with "packages/ui/src/button" -> OK
                                         let remainder = path_no_ext.len() - import_lower.len();
                                         if remainder == 0 || path_no_ext.as_bytes()[remainder - 1] == b'/' {
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

    // Keep walk_dependencies and helpers as they were (restored below for completeness)
    pub fn walk_dependencies(&self, start_ids: &[FileId], max_depth: usize) -> Vec<FileId> {
        let mut visited = HashSet::new();
        let mut current_frontier: Vec<FileId> = start_ids.to_vec();
        let mut results = Vec::new();

        for &id in start_ids {
            visited.insert(id);
            results.push(id);
        }

        for _depth in 0..max_depth {
            if current_frontier.is_empty() { break; }
            let mut next_frontier = Vec::new();

            for &id in &current_frontier {
                if let Some(file) = self.index.files.get(&id) {
                    let file_dir = std::path::Path::new(&file.path).parent().unwrap_or(std::path::Path::new(""));

                    for import_str in &file.imports {
                        let clean = import_str.trim_matches(|c| c == '"' || c == '\'');
                        let mut candidates = Vec::new();

                        if clean.starts_with("crate::") {
                            let path = clean.replace("crate::", "src/").replace("::", "/");
                            candidates.push(path);
                        } else if clean.starts_with('.') {
                            let joined = file_dir.join(clean);
                            if let Some(normalized) = normalize_path_simple(&joined) {
                                candidates.push(normalized);
                            }
                        } else {
                            candidates.push(clean.to_string());
                            for (pkg_name, pkg_path) in &self.index.package_map {
                                if clean.starts_with(pkg_name) {
                                    candidates.push(clean.replace(pkg_name, pkg_path));
                                }
                            }
                            for (alias_key, target_path) in &self.index.alias_map {
                                if clean.starts_with(alias_key) {
                                    candidates.push(clean.replace(alias_key, target_path));
                                }
                            }
                        }

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

                    for literal in &file.literals {
                        if let Some(target_ids) = self.index.symbol_map.get(literal) {
                             for &target_id in target_ids {
                                if target_id == id { continue; }
                                if visited.insert(target_id) {
                                    results.push(target_id);
                                    next_frontier.push(target_id);
                                }
                            }
                        }
                        let prefixes = ["route:GET:", "route:POST:", "html:tag:", "di:", "view:"];
                        for prefix in prefixes {
                            let probe = format!("{}{}", prefix, literal);
                            if let Some(target_ids) = self.index.symbol_map.get(&probe) {
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
            }
            current_frontier = next_frontier;
        }
        results
    }
}

fn extract_value_from_synthetic(key: &str) -> Option<&str> {
    if let Some(idx) = key.rfind(':') {
        if idx + 1 < key.len() { return Some(&key[idx+1..]); }
    }
    None
}

fn extract_file_stem(path: &str) -> Option<String> {
    Path::new(path).file_stem().and_then(|s| s.to_str()).map(|s| s.to_lowercase())
}

fn extract_parent_dir_name(path: &str) -> Option<String> {
    Path::new(path).parent().and_then(|p| p.file_name()).and_then(|s| s.to_str()).map(|s| s.to_lowercase())
}

fn is_generic_filename(stem: &str) -> bool {
    matches!(stem, "index" | "mod" | "__init__" | "main" | "lib")
}

fn is_generic_filename_path(path: &str) -> bool {
    if let Some(stem) = extract_file_stem(path) { is_generic_filename(&stem) } else { false }
}

fn normalize_path_simple(path: &Path) -> Option<String> {
    use std::path::Component;
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(c) => components.push(c.to_str()?),
            Component::ParentDir => { components.pop(); },
            Component::CurDir => {},
            _ => {}
        }
    }
    Some(components.join("/"))
}