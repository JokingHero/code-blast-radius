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

        // Prepare existing state for Change Detection
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

        // 6. Global Map Rebuild
        self.rebuild_maps(index);
    }

    fn rebuild_maps(&self, index: &mut BoundaryIndex) {
        index.symbol_map.clear();
        index.path_map.clear();
        index.usage_map.clear();

        // --- PASS 1: Build Knowledge Base (Definitions) ---
        // We need to know what "Concepts" exist in the workspace before we can
        // link literals to them.
        let mut known_concepts: HashSet<String> = HashSet::new();

        for file in index.files.values() {
            // A. Path Map
            let parts: Vec<&str> = file.path.split('/').collect();
            let len = parts.len();
            for i in 0..len {
                let suffix = parts[i..len].join("/");
                index.path_map.entry(suffix).or_default().push(file.id);
            }

            // B. Symbol Map (Physical)
            for def in &file.defs {
                index
                    .symbol_map
                    .entry(def.name.clone())
                    .or_default()
                    .push(file.id);
            }

            // C. Symbol Map (Synthetic/Logical)
            for syn_def in &file.synthetic_defs {
                index
                    .symbol_map
                    .entry(syn_def.clone())
                    .or_default()
                    .push(file.id);
                known_concepts.insert(syn_def.clone());
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

            // 2. Symbol References (Code)
            for ref_str in &file.symbol_refs {
                let token = ref_str.to_lowercase();
                index.usage_map.entry(token).or_default().push(file.id);
            }

            // 3. Literal Promotion (The Magic)
            // If a literal matches a Known Concept, promote it to a usage.
            for literal in &file.literals {
                // Heuristic A: Exact Match (Rare, but possible for internal IDs)
                if known_concepts.contains(literal) {
                    index
                        .usage_map
                        .entry(literal.clone())
                        .or_default()
                        .push(file.id);
                    continue;
                }

                // Heuristic B: Route Promotion
                // If the literal looks like a path ("/api/foo") and we have a concept "route:/api/foo"
                if literal.starts_with('/') {
                    let route_key = format!("route:{}", literal);
                    if known_concepts.contains(&route_key) {
                        // Crucial: We map the *Concept Key* to this file.
                        // The Walker will search for "route:/api/foo", find it in usage_map (this file),
                        // and find it in symbol_map (the definition file).
                        index.usage_map.entry(route_key).or_default().push(file.id);
                    }
                }

                // Heuristic C: Database Tables / Topics (Future extensions)
                // if literal.contains('_') && known_concepts.contains(&format!("db:{}", literal)) ...
            }
        }

        // Deduplicate usage entries
        for list in index.usage_map.values_mut() {
            list.sort_unstable();
            list.dedup();
        }
    }
}

// Helper to normalize tokens consistently
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
