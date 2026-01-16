use std::path::Path;
use std::fs;
use std::collections::{HashMap, HashSet};
use ignore::WalkBuilder;
use blake3;
use rayon::prelude::*;
use pathdiff;

use crate::models::{BoundaryIndex, FileBoundary, FileId};
use crate::analysis::boundary::extract_boundary;
use crate::analysis::language::{get_language_configs, LanguageConfig};

pub struct FileScanner {
    pub configs: HashMap<String, LanguageConfig>,
}

impl FileScanner {
    pub fn new() -> Self {
        let mut config_map = HashMap::new();
        for config in get_language_configs() {
            for &ext in config.file_extensions {
                config_map.insert(ext.to_string(), config.clone());
            }
        }
        Self {
            configs: config_map,
        }
    }

    pub fn scan(
        &self,
        root: &Path,
        index: &mut BoundaryIndex,
        root_id: &str
    ) {
        let root_abs = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        
        // 1. Walk & Collect Candidates
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

        // 2. Parallel Read & Hash
        struct FileData {
            relative_path: String,
            extension: String,
            hash: [u8; 32],
            content: String,
        }

        let scanned_files: Vec<FileData> = file_entries
            .into_par_iter()
            .filter_map(|path| {
                let extension = path.extension()?.to_str()?.to_string();
                
                if !self.configs.contains_key(&extension) {
                    return None;
                }

                let content = fs::read_to_string(&path).ok()?;
                let hash = blake3::hash(content.as_bytes()).into();

                let relative_path = pathdiff::diff_paths(&path, &root_abs)
                    .unwrap_or_else(|| path.clone())
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

        // 3. Identify & Process Changes (Parallel Parse)
        let existing_files: HashMap<String, [u8; 32]> = index.files.values()
            .filter(|f| f.root_id == root_id)
            .map(|f| (f.path.clone(), f.hash))
            .collect();
        
        // Map path -> existing ID to preserve IDs across updates
        let existing_ids: HashMap<String, FileId> = index.files.values()
            .filter(|f| f.root_id == root_id)
            .map(|f| (f.path.clone(), f.id))
            .collect();

        let new_boundaries: Vec<FileBoundary> = scanned_files
            .into_par_iter()
            .filter_map(|file_data| {
                // Check if unchanged
                if let Some(&old_hash) = existing_files.get(&file_data.relative_path) {
                    if old_hash == file_data.hash {
                        return None; // SKIP PARSING
                    }
                }

                // PARSE
                let config = &self.configs[&file_data.extension];
                match extract_boundary(
                    &file_data.relative_path,
                    &file_data.content,
                    config,
                    file_data.hash
                ) {
                    Ok(mut boundary) => {
                        boundary.root_id = root_id.to_string();
                        // Preserve ID if it existed
                        if let Some(&id) = existing_ids.get(&file_data.relative_path) {
                            boundary.id = id;
                        }
                        Some(boundary)
                    },
                    Err(_) => None,
                }
            })
            .collect();

        // 4. Sequential Update
        for boundary in new_boundaries {
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

        // 5. Cleanup Deleted Files
        let ids_to_remove: Vec<FileId> = index.files.values()
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