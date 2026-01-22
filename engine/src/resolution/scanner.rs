use blake3;
use ignore::WalkBuilder;
use pathdiff;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use crate::analysis::boundary::extract_boundary;
use crate::analysis::language::get_config_for_extension;
use crate::inference::conventions::ConventionEngine;
use crate::inference::frameworks::FrameworkManager;
use crate::inference::{configs, InferenceEngine};
use crate::manifest::scan_manifest_content;
use crate::models::{BoundaryIndex, FileBoundary, FileId};

pub struct FileScanner;

// Helper enum to handle the result of the parallel scan
enum ScanResult {
    Boundary(FileBoundary),
    PackageDef(String, String), // (Package Name, Relative Directory)
    Aliases(HashMap<String, String>),
    None,
}

struct FileData {
    relative_path: String,
    extension: String,
    hash: [u8; 32],
    content: String,
}

impl FileScanner {
    pub fn new() -> Self {
        Self
    }

    /// The main entry point for scanning a directory.
    pub fn scan(&self, root: &Path, index: &mut BoundaryIndex, root_id: &str) {
        let root_abs = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());

        // 0. Setup Inference Engine
        // We create it here and pass it into the parallel iterator.
        // It must be Send + Sync (which it is, as it contains Box<dyn InferenceRule>).
        let mut inference_engine = InferenceEngine::new();
        // Register the Path-Based Convention Engine
        inference_engine.register(ConventionEngine::new());

        // Register the Content-Based Framework Engine
        let mut fw_manager = FrameworkManager::new();
        configs::register_all(&mut fw_manager);
        inference_engine.register(fw_manager);

        // 1. Discovery Phase (IO Bound - optimized by ignore crate)
        let walker = WalkBuilder::new(&root_abs)
            .hidden(false)
            .git_ignore(true)
            .build();

        let mut file_entries = Vec::new();
        for result in walker {
            if let Ok(entry) = result {
                if entry.path().is_file() {
                    file_entries.push(entry.into_path());
                }
            }
        }

        // 2. Read Phase (IO/CPU Mixed - Parallelized)
        let scanned_files: Vec<FileData> = file_entries
            .into_par_iter()
            .filter_map(|path| {
                let filename = path.file_name()?.to_str()?;
                let extension = path
                    .extension()
                    .map(|s| s.to_str().unwrap_or(""))
                    .unwrap_or("")
                    .to_string();

                let is_manifest =
                    matches!(filename, "package.json" | "Cargo.toml" | "pyproject.toml");
                
                // Skip files we don't know how to parse, unless they are manifests
                if !is_manifest && get_config_for_extension(&extension).is_none() {
                    return None;
                }

                let content = fs::read_to_string(&path).ok()?;
                let hash = blake3::hash(content.as_bytes()).into();

                let diff = pathdiff::diff_paths(&path, &root_abs)?;
                let relative_path = diff.to_string_lossy().replace('\\', "/");

                Some(FileData {
                    relative_path,
                    extension,
                    hash,
                    content,
                })
            })
            .collect();

        // Capture seen paths for deletion logic later
        let seen_paths: HashSet<String> = scanned_files
            .iter()
            .map(|f| f.relative_path.clone())
            .collect();

        // Prepare existing state for Change Detection (Incremental Scan)
        let existing_files: HashMap<String, [u8; 32]> = index
            .files
            .values()
            .filter(|f| f.root_id == root_id)
            .map(|f| (f.path.clone(), f.hash))
            .collect();

        let existing_ids: HashMap<String, FileId> = index
            .files
            .values()
            .filter(|f| f.root_id == root_id)
            .map(|f| (f.path.clone(), f.id))
            .collect();

        // 3. Processing Phase (CPU Bound - Parallelized)
        let results: Vec<ScanResult> = scanned_files
            .into_par_iter()
            .map(|file_data| {
                let path_obj = Path::new(&file_data.relative_path);
                let filename = path_obj.file_name().and_then(|s| s.to_str()).unwrap_or("");

                // 1. Handle TSConfig / JSConfig Aliases
                if filename == "tsconfig.json" || filename == "jsconfig.json" {
                    let mut aliases = HashMap::new();
                    let config_dir = path_obj.parent().unwrap_or(Path::new(""));

                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&file_data.content)
                    {
                        if let Some(paths) = json
                            .get("compilerOptions")
                            .and_then(|o| o.get("paths"))
                            .and_then(|p| p.as_object())
                        {
                            for (key, val) in paths {
                                let clean_key = key.replace("/*", "/");
                                if let Some(first_path) = val
                                    .as_array()
                                    .and_then(|a| a.get(0))
                                    .and_then(|v| v.as_str())
                                {
                                    let clean_val = first_path.replace("/*", "/");
                                    let resolved_path = config_dir.join(&clean_val);
                                    let final_path =
                                        resolved_path.to_string_lossy().replace('\\', "/");
                                    aliases.insert(clean_key, final_path);
                                }
                            }
                        }
                    }
                    if !aliases.is_empty() {
                        return ScanResult::Aliases(aliases);
                    }
                    return ScanResult::None;
                }

                // A. Handle Manifests
                if matches!(filename, "package.json" | "Cargo.toml" | "pyproject.toml") {
                    let meta = scan_manifest_content(filename, &file_data.content);
                    if let Some(pkg_name) = meta.package_name {
                        let dir = Path::new(&file_data.relative_path)
                            .parent()
                            .map(|p| p.to_string_lossy().to_string().replace('\\', "/"))
                            .unwrap_or_else(|| String::from(""));

                        return ScanResult::PackageDef(pkg_name, dir);
                    }
                    return ScanResult::None;
                }

                // B. Handle Source Code
                if let Some(&old_hash) = existing_files.get(&file_data.relative_path) {
                    if old_hash == file_data.hash {
                        return ScanResult::None; // Skip unchanged
                    }
                }

                if let Some(config) = get_config_for_extension(&file_data.extension) {
                    match extract_boundary(
                        &file_data.relative_path,
                        &file_data.content,
                        config,
                        file_data.hash,
                    ) {
                        Ok(mut boundary) => {
                            boundary.root_id = root_id.to_string();

                            // --- INFERENCE STEP ---
                            // Look at physical facts and infer logical concepts
                            inference_engine.run(&mut boundary);
                            // ----------------------

                            if let Some(&id) = existing_ids.get(&file_data.relative_path) {
                                boundary.id = id;
                            }
                            return ScanResult::Boundary(boundary);
                        }
                        Err(_) => {
                            return ScanResult::None;
                        }
                    }
                }

                ScanResult::None
            })
            .collect();

        // 4. Update Index (Sequential)
        for res in results {
            match res {
                ScanResult::Boundary(boundary) => {
                    let id = if boundary.id != 0 {
                        boundary.id
                    } else {
                        let new_id = index.next_file_id;
                        index.next_file_id += 1;
                        new_id
                    };
                    let mut final_boundary = boundary;
                    final_boundary.id = id;
                    index.files.insert(id, final_boundary);
                }
                ScanResult::PackageDef(name, dir) => {
                    index.package_map.insert(name, dir);
                }
                ScanResult::Aliases(map) => {
                    index.alias_map.extend(map);
                }
                ScanResult::None => {}
            }
        }

        // 5. Cleanup Deleted Files
        let ids_to_remove: Vec<FileId> = index
            .files
            .values()
            .filter(|f| f.root_id == root_id && !seen_paths.contains(&f.path))
            .map(|f| f.id)
            .collect();

        for id in ids_to_remove {
            index.files.remove(&id);
        }

        // 6. Global Map Rebuild (Crucial for Topic Linking)
        self.rebuild_maps(index);
    }

    /// Rebuilds inverted indices (Symbol Map, Path Map, Usage Map).
    /// Updated to split literals and topics into segments for fuzzy matching.
    fn rebuild_maps(&self, index: &mut BoundaryIndex) {
        index.symbol_map.clear();
        index.path_map.clear();
        index.usage_map.clear();

        // --- PASS 1: Build Knowledge Base (Definitions) ---
        // We identify "Concepts" and also index them by their constituent parts.
        let mut known_concepts: HashSet<String> = HashSet::new();

        for file in index.files.values() {
            // A. Path Map (Fuzzy file resolution)
            let parts: Vec<&str> = file.path.split('/').collect();
            let len = parts.len();
            for i in 0..len {
                let suffix = parts[i..len].join("/");
                index.path_map.entry(suffix).or_default().push(file.id);
            }

            // B. Symbol Map (Physical Definitions)
            for def in &file.defs {
                index
                    .symbol_map
                    .entry(def.name.clone())
                    .or_default()
                    .push(file.id);
            }

            // C. Symbol Map (Synthetic/Logical Concepts)
            for syn_def in &file.synthetic_defs {
                index
                    .symbol_map
                    .entry(syn_def.clone())
                    .or_default()
                    .push(file.id);
                known_concepts.insert(syn_def.clone());

                // NEW: Index synthetic definition segments into usage_map.
                // If this file defines "topic:user/created", we want to be found
                // by walkers looking for "user" anchors.
                if let Some(tokens) = extract_topic_tokens(syn_def) {
                    for token in tokens {
                        index.usage_map.entry(token).or_default().push(file.id);
                    }
                }
            }
        }

        // --- PASS 2: Link Usages (References & Promoted Literals) ---
        for file in index.files.values() {
            // 1. Imports
            for import_str in &file.imports {
                if let Some(token) = extract_significant_token(import_str) {
                    index.usage_map.entry(token).or_default().push(file.id);
                }
            }

            // 2. Symbol References (Code identifiers)
            for ref_str in &file.symbol_refs {
                let token = ref_str.to_lowercase();
                index.usage_map.entry(token).or_default().push(file.id);
            }

            // 3. Literals (The Magic Glue)
            for literal in &file.literals {
                // Heuristic A: Exact Match (Promote literal to usage if Concept exists)
                if known_concepts.contains(literal) {
                    index
                        .usage_map
                        .entry(literal.clone())
                        .or_default()
                        .push(file.id);
                }

                // Heuristic B: Route/Topic Promotion
                // If the literal looks like a path ("/api/foo") or topic ("user.created"),
                // check if it matches a known concept prefixed with route/topic/etc.
                if literal.contains('/') || literal.contains('.') {
                    // Try to match specific prefixes
                    let route_key = format!("route:{}", literal);
                    if known_concepts.contains(&route_key) {
                        index.usage_map.entry(route_key).or_default().push(file.id);
                    }
                    
                    let topic_key = format!("topic:{}", literal);
                    if known_concepts.contains(&topic_key) {
                        index.usage_map.entry(topic_key).or_default().push(file.id);
                    }

                    // NEW: Tokenize the literal.
                    // If this file has literal "user/#", we index it under "user".
                    // This allows the Walker (starting at "user/created") to find this file.
                    if let Some(tokens) = extract_topic_tokens(literal) {
                        for token in tokens {
                            index.usage_map.entry(token).or_default().push(file.id);
                        }
                    }
                }
            }
        }

        // Deduplicate usage entries to keep index small
        for list in index.usage_map.values_mut() {
            list.sort_unstable();
            list.dedup();
        }
    }
}

// --- Helpers ---

/// Extracts the "significant" part of a file path or import (last segment).
fn extract_significant_token(path: &str) -> Option<String> {
    let last_segment = path.split('/').last()?;
    if last_segment.is_empty() || last_segment == "." || last_segment == ".." {
        return None;
    }
    let stem = std::path::Path::new(last_segment)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(last_segment);
    Some(stem.to_lowercase())
}

/// Extracts semantic segments from a topic string.
/// e.g. "topic:user/created" -> ["user", "created"]
/// e.g. "user/#" -> ["user"]
fn extract_topic_tokens(text: &str) -> Option<Vec<String>> {
    // 1. Strip synthetic prefixes if present
    let clean = if let Some(idx) = text.find(':') {
        // Heuristic: only strip if prefix is short (likely a category)
        if idx < 15 {
            &text[idx + 1..]
        } else {
            text
        }
    } else {
        text
    };

    let mut tokens = Vec::new();
    let delimiters = ['/', '.', ':'];
    
    for part in clean.split(|c| delimiters.contains(&c)) {
        // Filter out noise:
        // - Wildcards (*, #, +)
        // - Parameters ({id}, :id)
        // - Very short segments (v1, a, b) - optional, but helps precision
        if part.len() > 2 
            && !part.contains('*') 
            && !part.contains('#') 
            && !part.contains('+') 
            && !part.starts_with('{') 
            && !part.starts_with(':') 
        {
            tokens.push(part.to_lowercase());
        }
    }

    if tokens.is_empty() {
        None
    } else {
        Some(tokens)
    }
}