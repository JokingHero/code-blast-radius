use crate::models::{BoundaryIndex, FileId};
use std::collections::HashSet;
use std::path::Path;

pub struct JitWalker<'a> {
    index: &'a BoundaryIndex,
}

impl<'a> JitWalker<'a> {
    pub fn new(index: &'a BoundaryIndex) -> Self {
        Self { index }
    }

    /// Finds files that depend ON the start_ids.
    /// This is the "Blast Radius" calculation.
    pub fn walk_impact(&self, start_ids: &[FileId], max_depth: usize) -> Vec<FileId> {
        let mut visited = HashSet::new();
        let mut current_frontier: Vec<FileId> = start_ids.to_vec();
        let mut results = Vec::new();

        for &id in start_ids {
            visited.insert(id);
            results.push(id);
        }

        // Prefixes used to match bare references to synthetic definitions
        // e.g. ref "app-user" matches def "html:tag:app-user"
        let synthetic_prefixes = [
            "route:GET:",
            "route:POST:",
            "route:PUT:",
            "route:DELETE:",
            "html:tag:",
            "di:",
            "view:",
        ];

        for _depth in 0..max_depth {
            if current_frontier.is_empty() {
                break;
            }

            // 1. Build Query & Identify "Target Anchors"
            // We gather all symbols defined by the current frontier files to see who uses them.
            let mut search_defs: HashSet<&str> = HashSet::new();
            let mut search_paths: Vec<&str> = Vec::new();

            // Anchors are simple tokens used to look up potential candidates in the inverted usage_map.
            let mut target_anchors: HashSet<String> = HashSet::new();

            // Special list for checking if a literal contains a route path (substring match)
            let mut search_routes: Vec<&str> = Vec::new();

            for &id in &current_frontier {
                if let Some(f) = self.index.files.get(&id) {
                    search_paths.push(f.path.as_str());

                    // Add filename parts as anchors (for import resolution)
                    if let Some(stem) = extract_file_stem(&f.path) {
                        target_anchors.insert(stem.clone());
                        if is_generic_filename(&stem) {
                            if let Some(parent) = extract_parent_dir_name(&f.path) {
                                target_anchors.insert(parent);
                            }
                        }
                    }

                    // Add physical definitions
                    for def in &f.defs {
                        search_defs.insert(def.name.as_str());
                        target_anchors.insert(def.name.to_lowercase());
                    }

                    // Add synthetic definitions (Framework concepts)
                    for syn_def in &f.synthetic_defs {
                        search_defs.insert(syn_def.as_str());
                        target_anchors.insert(syn_def.to_lowercase());

                        // If "route:GET:/api/users", extract "/api/users"
                        if let Some(val) = extract_value_from_synthetic(syn_def) {
                            let val_lower = val.to_lowercase();
                            target_anchors.insert(val_lower.clone());

                            // Also add the last segment as an anchor to find files
                            // that might have long strings containing this value.
                            // e.g. "http://localhost/api/users" might be indexed under "users"
                            if let Some(last_segment) = extract_significant_token(val) {
                                target_anchors.insert(last_segment);
                            }

                            if syn_def.starts_with("route:") {
                                search_routes.push(val);
                            }
                        }
                    }
                }
            }

            // 2. Candidate Selection
            // Use the Inverted Index (usage_map) to find files that *might* contain a match.
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
            // Check the candidates to see if they actually match the search criteria.
            let mut next_frontier = Vec::new();

            for id in candidates_to_check {
                let candidate = match self.index.files.get(&id) {
                    Some(c) => c,
                    None => continue,
                };

                let mut is_match = false;

                // A. Logic Refs (Code Symbols)
                // e.g. A TypeScript file uses "UserService" or "app-root"
                if !is_match {
                    for reference in &candidate.symbol_refs {
                        // 1. Direct Match
                        if search_defs.contains(reference.as_str()) {
                            is_match = true;
                            break;
                        }
                        // 2. Synthetic Prefix Match (e.g. ref "app-root" -> matches def "html:tag:app-root")
                        for prefix in synthetic_prefixes {
                            let probe = format!("{}{}", prefix, reference);
                            if search_defs.contains(probe.as_str()) {
                                is_match = true;
                                break;
                            }
                        }
                        if is_match {
                            break;
                        }
                    }
                }

                // B. Literals (Strings found in code)
                // e.g. A shell script uses "http://localhost/api/users"
                if !is_match {
                    for literal in &candidate.literals {
                        // 1. Direct Match
                        if search_defs.contains(literal.as_str()) {
                            is_match = true;
                            break;
                        }

                        // 2. Synthetic Prefix Match (e.g. literal "/api/v1" -> matches def "route:GET:/api/v1")
                        for prefix in synthetic_prefixes {
                            let probe = format!("{}{}", prefix, literal);
                            if search_defs.contains(probe.as_str()) {
                                is_match = true;
                                break;
                            }
                        }
                        if is_match {
                            break;
                        }

                        // 3. Route Containment Match (Substring)
                        // e.g. literal "http://localhost/api/v1" contains "/api/v1"
                        for route_path in &search_routes {
                            if literal.contains(route_path) {
                                is_match = true;
                                break;
                            }
                        }
                        if is_match {
                            break;
                        }
                    }
                }

                // C. Structural Imports (Path matching)
                if !is_match {
                    for import_str in &candidate.imports {
                        // Handle Monorepo Aliases (e.g. @app/ui -> packages/ui)
                        let effective_search_str = if let Some((alias, target_dir)) = self
                            .index
                            .package_map
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
                                // Sub-Check A: File Stem Match (import "./utils" matches "utils.ts")
                                if let Some(stem) = extract_file_stem(search_path) {
                                    if stem == import_lower {
                                        is_match = true;
                                        break;
                                    }
                                }

                                // Sub-Check B: Suffix Match (e.g. import "style.css")
                                if path_lower.ends_with(&import_lower) {
                                    is_match = true;
                                    break;
                                }

                                // Sub-Check C: Directory Import (Index file)
                                if is_generic_filename_path(search_path) {
                                    if let Some(parent) = extract_parent_dir_name(search_path) {
                                        if parent == import_lower {
                                            is_match = true;
                                            break;
                                        }
                                    }
                                }

                                // Sub-Check D: Path Without Extension Match
                                // e.g. import "src/Button" matches "src/Button.tsx"
                                if let Some(dot_index) = path_lower.rfind('.') {
                                    let path_no_ext = &path_lower[..dot_index];
                                    if path_no_ext.ends_with(&import_lower) {
                                        // Boundary check: ensure we matched a full segment
                                        let remainder = path_no_ext.len() - import_lower.len();
                                        if remainder == 0
                                            || path_no_ext.as_bytes()[remainder - 1] == b'/'
                                        {
                                            is_match = true;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        if is_match {
                            break;
                        }
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

    /// Finds files that the start_ids depend ON.
    /// Used for "What does this file need?" queries.
    pub fn walk_dependencies(&self, start_ids: &[FileId], max_depth: usize) -> Vec<FileId> {
        let mut visited = HashSet::new();
        let mut current_frontier: Vec<FileId> = start_ids.to_vec();
        let mut results = Vec::new();

        for &id in start_ids {
            visited.insert(id);
            results.push(id);
        }

        // Prefixes for mapping literals back to potential synthetic definitions
        let synthetic_prefixes = [
            "route:GET:",
            "route:POST:",
            "route:PUT:",
            "route:DELETE:",
            "html:tag:",
            "di:",
            "view:",
        ];

        for _depth in 0..max_depth {
            if current_frontier.is_empty() {
                break;
            }
            let mut next_frontier = Vec::new();

            for &id in &current_frontier {
                if let Some(file) = self.index.files.get(&id) {
                    let file_dir = std::path::Path::new(&file.path)
                        .parent()
                        .unwrap_or(std::path::Path::new(""));

                    // 1. Check Imports (Path Map lookup)
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
                            // Handle Aliases/Packages
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

                        let extensions = [
                            "",
                            ".ts",
                            ".tsx",
                            ".js",
                            ".jsx",
                            ".rs",
                            ".py",
                            ".java",
                            ".go",
                            "/index.ts",
                            "/index.js",
                        ];
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

                    // 2. Check Code References (Symbol Map lookup)
                    for symbol_ref in &file.symbol_refs {
                        // Direct Match
                        if let Some(target_ids) = self.index.symbol_map.get(symbol_ref) {
                            for &target_id in target_ids {
                                if target_id == id {
                                    continue;
                                }
                                if visited.insert(target_id) {
                                    results.push(target_id);
                                    next_frontier.push(target_id);
                                }
                            }
                        }

                        // Synthetic Prefix Match
                        for prefix in synthetic_prefixes {
                            let probe = format!("{}{}", prefix, symbol_ref);
                            if let Some(target_ids) = self.index.symbol_map.get(&probe) {
                                for &target_id in target_ids {
                                    if target_id == id {
                                        continue;
                                    }
                                    if visited.insert(target_id) {
                                        results.push(target_id);
                                        next_frontier.push(target_id);
                                    }
                                }
                            }
                        }
                    }

                    // 3. Check Literals (Symbol Map lookup)
                    for literal in &file.literals {
                        // Direct Match
                        if let Some(target_ids) = self.index.symbol_map.get(literal) {
                            for &target_id in target_ids {
                                if target_id == id {
                                    continue;
                                }
                                if visited.insert(target_id) {
                                    results.push(target_id);
                                    next_frontier.push(target_id);
                                }
                            }
                        }
                        // Synthetic Prefix Match
                        for prefix in synthetic_prefixes {
                            let probe = format!("{}{}", prefix, literal);
                            if let Some(target_ids) = self.index.symbol_map.get(&probe) {
                                for &target_id in target_ids {
                                    if target_id == id {
                                        continue;
                                    }
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

// --- Helper Functions ---

fn extract_value_from_synthetic(key: &str) -> Option<&str> {
    // "route:GET:/api/users" -> "/api/users"
    // "di:UserService" -> "UserService"
    let first_colon = key.find(':')?;
    let rest = &key[first_colon + 1..];

    // If there is a second colon, the value follows it.
    // If not, the remainder is the value.
    if let Some(second_colon) = rest.find(':') {
        Some(&rest[second_colon + 1..])
    } else {
        Some(rest)
    }
}

fn extract_significant_token(path: &str) -> Option<String> {
    // extracts "health" from "/api/health" or "http://localhost/api/health"
    let last_segment = path.split('/').last()?;
    if last_segment.is_empty() || last_segment == "." || last_segment == ".." {
        return None;
    }
    // Remove query params or file extensions
    let clean = last_segment.split('?').next().unwrap_or(last_segment);
    let stem = Path::new(clean)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(clean);

    Some(stem.to_lowercase())
}

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
            Component::ParentDir => {
                components.pop();
            }
            Component::CurDir => {}
            _ => {}
        }
    }
    Some(components.join("/"))
}