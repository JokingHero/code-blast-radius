use std::path::Path;
use std::fs;
use std::collections::{HashMap, HashSet};
use ignore::WalkBuilder;
use blake3;
use rayon::prelude::*;
use pathdiff;

use crate::models::{BoundaryIndex, FileBoundary, FileId};
use crate::analysis::boundary::extract_boundary;
use crate::analysis::language::get_config_for_extension;
use crate::manifest::scan_manifest_content; // Import the updated helper

pub struct FileScanner;

// Helper enum to handle the result of the parallel scan
enum ScanResult {
    Boundary(FileBoundary),
    PackageDef(String, String), // (Package Name, Relative Directory)
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

    pub fn scan(
        &self,
        root: &Path,
        index: &mut BoundaryIndex,
        root_id: &str
    ) {
        let root_abs = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        
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
        // We read all files into memory. For large repos, this uses RAM, 
        // but ensures we have zero-latency access for parsing.
        let scanned_files: Vec<FileData> = file_entries
            .into_par_iter()
            .filter_map(|path| {
                let filename = path.file_name()?.to_str()?;
                let extension = path.extension().map(|s| s.to_str().unwrap_or("")).unwrap_or("").to_string();
                
                // Filter: Is it a supported language OR a manifest?
                let is_manifest = matches!(filename, "package.json" | "Cargo.toml" | "pyproject.toml");
                if !is_manifest && get_config_for_extension(&extension).is_none() {
                    return None;
                }

                let content = fs::read_to_string(&path).ok()?;
                let hash = blake3::hash(content.as_bytes()).into();

                // If pathdiff fails (e.g. cross-drive on Windows), returns None
                // so filter_map skips the file, ensuring we never store absolute paths.
                let diff = pathdiff::diff_paths(&path, &root_abs)?;
                
                let relative_path = diff
                    .to_string_lossy()
                    .replace('\\', "/");

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
        let existing_files: HashMap<String, [u8; 32]> = index.files.values()
            .filter(|f| f.root_id == root_id)
            .map(|f| (f.path.clone(), f.hash))
            .collect();
        
        let existing_ids: HashMap<String, FileId> = index.files.values()
            .filter(|f| f.root_id == root_id)
            .map(|f| (f.path.clone(), f.id))
            .collect();

        // 3. Processing Phase (CPU Bound - Parallelized)
        let results: Vec<ScanResult> = scanned_files
            .into_par_iter()
            .map(|file_data| {
                let filename = Path::new(&file_data.relative_path)
                    .file_name().and_then(|s| s.to_str()).unwrap_or("");

                // A. Handle Manifests (Always re-parse to ensure map integrity)
                // This is extremely fast (small JSON/TOML files)
                if matches!(filename, "package.json" | "Cargo.toml" | "pyproject.toml") {
                    let meta = scan_manifest_content(filename, &file_data.content);
                    
                    if let Some(pkg_name) = meta.package_name {
                        // Calculate directory of the package
                        let dir = Path::new(&file_data.relative_path)
                            .parent()
                            .map(|p| p.to_string_lossy().to_string().replace('\\', "/"))
                            .unwrap_or_else(|| String::from(""));
                        
                        return ScanResult::PackageDef(pkg_name, dir);
                    }
                    
                    // Even if it's a manifest, it might not define a package name.
                    // We don't track manifests as "Files" in the index, only their metadata.
                    return ScanResult::None; 
                }

                // B. Handle Source Code (Change Detection Optimization)
                if let Some(&old_hash) = existing_files.get(&file_data.relative_path) {
                    if old_hash == file_data.hash {
                        return ScanResult::None; // Skip unchanged files
                    }
                }

                // Parse Source Code
                // get_config_for_extension is safe here (lazy static)
                if let Some(config) = get_config_for_extension(&file_data.extension) {
                    match extract_boundary(
                        &file_data.relative_path,
                        &file_data.content,
                        config,
                        file_data.hash
                    ) {
                        Ok(mut boundary) => {
                            boundary.root_id = root_id.to_string();
                            // Preserve ID if updating existing file
                            if let Some(&id) = existing_ids.get(&file_data.relative_path) {
                                boundary.id = id;
                            }
                            return ScanResult::Boundary(boundary);
                        },
                        Err(_) => return ScanResult::None,
                    }
                }

                ScanResult::None
            })
            .collect();

        // 4. Update Index (Sequential - very fast map operations)
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
                },
                ScanResult::PackageDef(name, dir) => {
                    // Update global package map
                    index.package_map.insert(name, dir);
                },
                ScanResult::None => {}
            }
        }

        // 5. Cleanup Deleted Files
        let ids_to_remove: Vec<FileId> = index.files.values()
            .filter(|f| f.root_id == root_id && !seen_paths.contains(&f.path))
            .map(|f| f.id)
            .collect();

        for id in ids_to_remove {
            index.files.remove(&id);
        }

        // 6. Global Map Rebuild (Inverted Indices)
        self.rebuild_maps(index);
    }

    fn rebuild_maps(&self, index: &mut BoundaryIndex) {
        index.symbol_map.clear();
        index.path_map.clear();

        for file in index.files.values() {
            // A. Populate Path Map
            let parts: Vec<&str> = file.path.split('/').collect();
            let len = parts.len();
            for i in 0..len {
                let suffix = parts[i..len].join("/");
                index.path_map.entry(suffix).or_default().push(file.id);
            }

            // B. Populate Symbol Map
            for def in &file.defs {
                index.symbol_map.entry(def.name.clone()).or_default().push(file.id);
            }
        }
    }
}