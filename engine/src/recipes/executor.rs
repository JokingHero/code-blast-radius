use anyhow::{Context, Result};
use globset::Glob;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use crate::analysis::language::get_config_for_extension;
use crate::models::{BoundaryIndex, Definition, FileId};
use crate::query::output::{ContextOutput, FileContent, FileContextMetadata, LineRange};
use crate::query::walker::JitWalker;
use crate::recipes::models::{FileTransform, Recipe, RecipeOperation};

pub struct RecipeExecutor<'a> {
    index: &'a BoundaryIndex,
    // We need roots to resolve absolute paths for file reading
    // Key: root_id, Value: Absolute Path
    roots: HashMap<String, PathBuf>,
}

impl<'a> RecipeExecutor<'a> {
    pub fn new(index: &'a BoundaryIndex, roots: HashMap<String, String>) -> Self {
        let parsed_roots = roots
            .into_iter()
            .map(|(k, v)| (k, PathBuf::from(v)))
            .collect();
        Self {
            index,
            roots: parsed_roots,
        }
    }

    /// Phase 1: File Discovery (Resolving the list of files)
    fn resolve_files(&self, operations: &[RecipeOperation]) -> Result<HashSet<FileId>> {
        let mut working_set = HashSet::new();

        for op in operations {
            match op {
                RecipeOperation::AddFiles { pattern } => {
                    let normalized = pattern.replace('\\', "/");
                    let glob = Glob::new(&normalized)?.compile_matcher();

                    for file in self.index.files.values() {
                        if glob.is_match(&file.path) {
                            working_set.insert(file.id);
                        }
                    }
                }
                RecipeOperation::RemoveFiles { pattern } => {
                    let normalized = pattern.replace('\\', "/");
                    let glob = Glob::new(&normalized)?.compile_matcher();
                    working_set.retain(|id| {
                        if let Some(f) = self.index.files.get(id) {
                            !glob.is_match(&f.path)
                        } else {
                            true
                        }
                    });
                }
                RecipeOperation::BlastRadius {
                    symbol,
                    max_depth,
                    exclude_tests,
                } => {
                    // 1. Find seeds
                    let mut seeds = Vec::new();

                    // Is it a symbol name?
                    if let Some(ids) = self.index.symbol_map.get(symbol) {
                        seeds.extend(ids);
                    }

                    // Is it a file path?
                    for file in self.index.files.values() {
                        if file.path.ends_with(symbol) {
                            seeds.push(file.id);
                        }
                    }

                    if seeds.is_empty() {
                        continue;
                    }

                    // 2. Walk
                    let walker = JitWalker::new(self.index);
                    let impacted = walker.walk_impact(&seeds, *max_depth as usize);
                    working_set.extend(impacted);
                    let dependencies = walker.walk_dependencies(&seeds, *max_depth as usize);
                    working_set.extend(dependencies);

                    if *exclude_tests {
                        working_set.retain(|id| {
                            if let Some(f) = self.index.files.get(id) {
                                !f.is_test 
                            } else {
                                true
                            }
                        });
                    }
                }
            }
        }

        Ok(working_set)
    }

    /// Phase 2: Processing (Reading & masking content)
    fn process_file(
        &self,
        file_id: FileId,
        transform: Option<&FileTransform>,
    ) -> Result<FileContent> {
        let file_node = self
            .index
            .files
            .get(&file_id)
            .context("File ID not found")?;

        let root_path = self
            .roots
            .get(&file_node.root_id)
            .context("Root ID not found for file")?;

        let absolute_path = root_path.join(&file_node.path);
        let raw_content = fs::read_to_string(&absolute_path)
            .with_context(|| format!("Failed to read {:?}", absolute_path))?;

        let ext = absolute_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("txt")
            .to_string();
        let skeleton_template = get_config_for_extension(&ext)
            .map(|c| c.skeleton_template)
            .unwrap_or(" ... ");

        let current_hash: [u8; 32] = blake3::hash(raw_content.as_bytes()).into();
        let is_stale = current_hash != file_node.hash;

        // Apply Transform ONLY if hashes match
        let final_content = if is_stale {
            // If stale, we cannot trust the byte offsets in file_node.defs.
            // We return the raw content to avoid panicking.
            eprintln!("Warning: File '{}' is changed on disk but not in index. Skipping transforms.", file_node.path);
            raw_content.clone()
        } else if let Some(tr) = transform {
            self.apply_transform(&raw_content, &file_node.defs, tr, skeleton_template)
        } else {
            raw_content.clone()
        };

        let line_count = final_content.lines().count();
        // Since we might have skeletonized the file, calculate exact size of result
        let token_count = (final_content.len() / 4) as u32;
        let ext = absolute_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("txt")
            .to_string();

        Ok(FileContent {
            metadata: FileContextMetadata {
                file_id: file_node.id,
                path: file_node.path.clone(),
                root_name: None, // Filled by UI if needed
                language: ext,
                is_test: file_node.is_test, 
                relevant_lines: vec![LineRange {
                    start: 1,
                    end: line_count,
                }],
                token_count,
            },
            content: final_content,
        })
    }

    fn apply_transform(
        &self,
        content: &str,
        defs: &[Definition],
        transform: &FileTransform,
        skeleton_template: &str,
    ) -> String {
        let mut masks: Vec<(usize, usize)> = Vec::new();

        for def in defs {
            // Only mask if we have a body range
            if let Some((start, end)) = def.body_range {
                let should_mask = match transform {
                    FileTransform::Skeletonize(targets) => targets.contains(&def.name),
                    FileTransform::FocusOn(targets) => !targets.contains(&def.name),
                };

                if should_mask {
                    masks.push((start, end));
                }
            }
        }

        if masks.is_empty() {
            return content.to_string();
        }

        // Sort by start position to process linearly
        masks.sort_by(|a, b| a.0.cmp(&b.0));

        let mut result = String::with_capacity(content.len());
        let mut last_pos = 0;

        for (start, end) in masks {
            // Safety check bounds
            if start < last_pos || start >= content.len() || end > content.len() {
                continue;
            }
            result.push_str(&content[last_pos..start]);
            result.push_str(skeleton_template);
            last_pos = end;
        }

        // Append remaining text
        if last_pos < content.len() {
            result.push_str(&content[last_pos..]);
        }

        result
    }

    // --- Public API ---

    pub fn execute_full(&self, recipe: &Recipe) -> Result<ContextOutput<FileContent>> {
        let ids = self.resolve_files(&recipe.operations)?;
        let mut files = Vec::new();

        for id in ids {
            let file_node = self.index.files.get(&id).unwrap();

            // Match transforms to file paths
            let transform = recipe
                .transforms
                .iter()
                .find(|(k, _)| file_node.path.ends_with(*k))
                .map(|(_, v)| v);

            if let Ok(content) = self.process_file(id, transform) {
                files.push(content);
            }
        }

        files.sort_by(|a, b| a.metadata.path.cmp(&b.metadata.path));

        Ok(ContextOutput {
            target: recipe.name.clone(),
            files,
        })
    }

    // Metadata-only version (faster, no file reading)
    pub fn execute_metadata(&self, recipe: &Recipe) -> Result<ContextOutput<FileContextMetadata>> {
        let ids = self.resolve_files(&recipe.operations)?;
        let mut meta = Vec::new();

        for id in ids {
            if let Some(file_node) = self.index.files.get(&id) {
                let ext = std::path::Path::new(&file_node.path)
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("txt")
                    .to_string();

                meta.push(FileContextMetadata {
                    file_id: file_node.id,
                    path: file_node.path.clone(),
                    root_name: None,
                    language: ext,
                    is_test: file_node.is_test,
                    relevant_lines: vec![], // No lines calculated for metadata-only
                    token_count: file_node.token_count,
                });
            }
        }

        meta.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(ContextOutput {
            target: recipe.name.clone(),
            files: meta,
        })
    }

    pub fn get_file_content(&self, file_id: FileId, recipe: &Recipe) -> Result<Option<String>> {
        if let Some(file_node) = self.index.files.get(&file_id) {
            let transform = recipe
                .transforms
                .iter()
                .find(|(k, _)| file_node.path.ends_with(*k))
                .map(|(_, v)| v);

            match self.process_file(file_id, transform) {
                Ok(fc) => Ok(Some(fc.content)),
                Err(e) => Err(e),
            }
        } else {
            Ok(None)
        }
    }
}
