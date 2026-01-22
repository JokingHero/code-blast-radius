use crate::models::{BoundaryIndex, FileId};
use crate::topic::matches_topic;
use std::collections::{HashSet, HashMap};
use std::path::Path;

pub struct JitWalker<'a> {
    index: &'a BoundaryIndex,
}

impl<'a> JitWalker<'a> {
    pub fn new(index: &'a BoundaryIndex) -> Self {
        Self { index }
    }

    /// Finds files that depend ON the start_ids.
    /// (Forward Search: "Who breaks if I change this?")
    pub fn walk_impact(&self, start_ids: &[FileId], max_depth: usize) -> Vec<FileId> {
        let mut visited = HashSet::new();
        let mut current_frontier: Vec<FileId> = start_ids.to_vec();
        let mut results = Vec::new();

        for &id in start_ids {
            visited.insert(id);
            results.push(id);
        }

        // Prefixes to treat as topics for wildcard matching
        let topic_prefixes = [
            "topic:", "event:", "queue:", "route:", "di:", "view:", "html:tag:",
        ];

        for _depth in 0..max_depth {
            if current_frontier.is_empty() {
                break;
            }

            // 1. Build Query & Identify "Target Anchors"
            // We gather all symbols defined by the current frontier files.
            let mut search_defs: HashSet<&str> = HashSet::new();
            let mut search_paths: Vec<&str> = Vec::new();
            let mut search_topics: Vec<&str> = Vec::new();

            // Anchors are simple tokens used to look up potential candidates in the inverted usage_map.
            let mut target_anchors: HashSet<String> = HashSet::new();

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

                    // Add synthetic definitions (Framework concepts & Topics)
                    for syn_def in &f.synthetic_defs {
                        search_defs.insert(syn_def.as_str());
                        target_anchors.insert(syn_def.to_lowercase());

                        // Generate sub-anchors (e.g. "topic:user/created" -> "user", "created")
                        add_topic_anchors(syn_def, &mut target_anchors);

                        if topic_prefixes.iter().any(|p| syn_def.starts_with(p)) {
                            search_topics.push(syn_def);
                        }
                    }
                    
                    // Also consider literals in the starting file as potential topics
                    for literal in &f.literals {
                        add_topic_anchors(literal, &mut target_anchors);
                        search_topics.push(literal);
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

                // A. Topic / Wildcard Matching
                if !is_match {
                    let candidate_strings: Vec<&str> = candidate.synthetic_defs.iter()
                        .map(|s| s.as_str())
                        .chain(candidate.literals.iter().map(|s| s.as_str()))
                        .collect();

                    for &my_topic in &search_topics {
                        for &their_topic in &candidate_strings {
                            if matches_topic(their_topic, my_topic) || matches_topic(my_topic, their_topic) {
                                is_match = true;
                                break;
                            }
                        }
                        if is_match { break; }
                    }
                }

                // B. Logic Refs (Code Symbols)
                if !is_match {
                    for reference in &candidate.symbol_refs {
                        // Direct Match
                        if search_defs.contains(reference.as_str()) {
                            is_match = true;
                            break;
                        }
                        // Synthetic Prefix Match (legacy support)
                        for prefix in topic_prefixes {
                            let probe = format!("{}{}", prefix, reference);
                            if search_defs.contains(probe.as_str()) {
                                is_match = true;
                                break;
                            }
                        }
                        if is_match { break; }
                    }
                }

                // C. Structural Imports (Path matching)
                if !is_match {
                    for import_str in &candidate.imports {
                        // Handle Monorepo Aliases
                        let aliased = resolve_alias(import_str, &self.index.package_map);
                        
                        // FIX: Clean relative prefixes ("./utils" -> "utils") so substring/stem match works
                        let clean_import = aliased.strip_prefix("./").unwrap_or(&aliased);
                        let import_lower = clean_import.to_lowercase();

                        for search_path in &search_paths {
                            let path_lower = search_path.to_lowercase();
                            if is_path_match(&path_lower, &import_lower, search_path) {
                                is_match = true;
                                break;
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

    /// Finds files that the start_ids depend ON.
    /// (Backward Search: "What does this file need?")
    pub fn walk_dependencies(&self, start_ids: &[FileId], max_depth: usize) -> Vec<FileId> {
        let mut visited = HashSet::new();
        let mut current_frontier: Vec<FileId> = start_ids.to_vec();
        let mut results = Vec::new();

        for &id in start_ids {
            visited.insert(id);
            results.push(id);
        }

        let topic_prefixes = [
            "topic:", "event:", "queue:", "route:", "di:", "view:", "html:tag:",
        ];

        for _depth in 0..max_depth {
            if current_frontier.is_empty() {
                break;
            }
            let mut next_frontier = Vec::new();

            // 1. Build Query from Start Files
            let mut search_tokens: HashSet<String> = HashSet::new();
            let mut search_literals: Vec<&str> = Vec::new();

            for &id in &current_frontier {
                if let Some(file) = self.index.files.get(&id) {
                    // Collect symbols we use
                    for sym in &file.symbol_refs {
                        search_tokens.insert(sym.to_lowercase());
                    }
                    
                    // Collect literals (potential topics/imports)
                    for lit in &file.literals {
                        search_literals.push(lit);
                        add_topic_anchors(lit, &mut search_tokens);
                    }
                    
                    // Collect imports as tokens
                    for imp in &file.imports {
                        if let Some(stem) = extract_significant_token(imp) {
                            search_tokens.insert(stem);
                        }
                    }
                }
            }

            // 2. Candidate Selection via Usage Map
            let mut candidate_ids: HashSet<FileId> = HashSet::new();
            for token in search_tokens {
                if let Some(ids) = self.index.usage_map.get(&token) {
                    candidate_ids.extend(ids);
                }
            }

            let candidates_to_check: Vec<FileId> = candidate_ids
                .into_iter()
                .filter(|id| !visited.contains(id))
                .collect();

            // 3. Verification
            for id in candidates_to_check {
                let candidate = match self.index.files.get(&id) { Some(c) => c, None => continue };
                let mut is_match = false;

                // A. Reverse Topic Match
                if !is_match {
                    let candidate_defs: Vec<&str> = candidate.synthetic_defs.iter()
                        .map(|s| s.as_str())
                        .chain(candidate.defs.iter().map(|d| d.name.as_str()))
                        .collect();

                    for &my_lit in &search_literals {
                        for &their_def in &candidate_defs {
                             if matches_topic(my_lit, their_def) || matches_topic(their_def, my_lit) {
                                is_match = true;
                                break;
                             }
                        }
                        if is_match { break; }

                        // Check specific prefixes
                        for prefix in topic_prefixes {
                            let probe = format!("{}{}", prefix, my_lit);
                            if candidate_defs.contains(&probe.as_str()) {
                                is_match = true;
                                break;
                            }
                        }
                        if is_match { break; }
                    }
                }

                // B. Import Match (Does my import point to this file?)
                if !is_match {
                    // Iterate my files again to check imports specifically against this candidate path
                    for &my_id in &current_frontier {
                        let my_file = self.index.files.get(&my_id).unwrap();
                        let my_dir = Path::new(&my_file.path).parent().unwrap_or(Path::new(""));

                        for import_str in &my_file.imports {
                            let resolved = resolve_import_path(import_str, my_dir, &self.index.package_map, &self.index.alias_map);
                            // Simple suffix check for speed
                            if candidate.path.ends_with(&resolved) || candidate.path.contains(&resolved) {
                                is_match = true; 
                                break;
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
}

// --- Helper Functions ---

fn add_topic_anchors(text: &str, anchors: &mut HashSet<String>) {
    let clean = if let Some(idx) = text.find(':') { 
        // Heuristic: strip prefix only if short
        if idx < 15 { &text[idx+1..] } else { text }
    } else { text };
    
    let delimiters = ['/', '.', ':'];
    for part in clean.split(|c| delimiters.contains(&c)) {
        // Skip wildcards and short words to avoid noise
        if part.len() > 2 && !part.contains('*') && !part.contains('#') && !part.contains('+') {
            anchors.insert(part.to_lowercase());
        }
    }
}

fn extract_significant_token(path: &str) -> Option<String> {
    let last_segment = path.split('/').last()?;
    if last_segment.is_empty() || last_segment == "." || last_segment == ".." {
        return None;
    }
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

fn resolve_alias(import_str: &str, package_map: &HashMap<String, String>) -> String {
    if let Some((alias, target_dir)) = package_map
        .iter()
        .find(|(k, _)| import_str.starts_with(*k))
    {
        import_str.replace(alias, target_dir)
    } else {
        import_str.to_string()
    }
}

fn is_path_match(path_lower: &str, import_lower: &str, original_path: &str) -> bool {
    // 1. Substring match for directories
    if path_lower.contains(import_lower) {
        
        // 2. Suffix match (standard)
        if path_lower.ends_with(import_lower) {
            return true;
        }

        // 3. Stem match ("./utils" -> "utils.ts")
        if let Some(stem) = extract_file_stem(original_path) {
            if stem == import_lower {
                return true;
            }
        }

        // 4. Index file match
        if is_generic_filename_path(original_path) {
             if let Some(parent) = extract_parent_dir_name(original_path) {
                if parent == import_lower {
                    return true;
                }
            }
        }

        // 5. Extensionless match ("src/Button" -> "src/Button.tsx")
        if let Some(dot_index) = path_lower.rfind('.') {
            let path_no_ext = &path_lower[..dot_index];
            if path_no_ext.ends_with(import_lower) {
                // Ensure boundary
                let remainder = path_no_ext.len().saturating_sub(import_lower.len());
                if remainder == 0 || path_no_ext.as_bytes()[remainder - 1] == b'/' {
                    return true;
                }
            }
        }
    }
    false
}

fn is_generic_filename_path(path: &str) -> bool {
    if let Some(stem) = extract_file_stem(path) {
        is_generic_filename(&stem)
    } else {
        false
    }
}

fn resolve_import_path(
    import_str: &str, 
    base_dir: &Path, 
    pkg_map: &HashMap<String, String>, 
    alias_map: &HashMap<String, String>
) -> String {
    let clean = import_str.trim_matches(|c| c == '"' || c == '\'');
    
    // Handle Aliases
    for (alias, target) in alias_map {
        if clean.starts_with(alias) {
            return clean.replace(alias, target);
        }
    }
    for (pkg, target) in pkg_map {
        if clean.starts_with(pkg) {
            return clean.replace(pkg, target);
        }
    }

    if clean.starts_with('.') {
         base_dir.join(clean).to_string_lossy().replace('\\', "/")
    } else {
        clean.to_string()
    }
}