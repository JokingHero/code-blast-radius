use std::collections::{ HashMap, HashSet };
use std::fs;

use anyhow::{ Result, Context };
use globset::{ Glob };

use crate::analysis::language::{ LanguageConfig, get_language_configs };
use crate::resolution::Indexer;
use crate::query::traversal::{ GraphWalker, TraversalMode };
use crate::query::output::{ ContextOutput, FileContent, FileContextMetadata, LineRange };
use crate::recipes::models::{ Recipe, RecipeOperation, FileTransform };
use crate::models::{ SymbolKind, FileId };

#[derive(Debug, Clone)]
struct RenderMask {
    start: usize,
    end: usize,
    replacement: String,
}

pub struct RecipeExecutor<'a> {
    indexer: &'a Indexer,
    config_map: HashMap<String, LanguageConfig>,
    root_map: HashMap<String, String>,
}

impl<'a> RecipeExecutor<'a> {
    pub fn new(indexer: &'a Indexer, root_map: HashMap<String, String>) -> Self {
        let mut config_map = HashMap::new();
        for config in get_language_configs() {
            for &ext in config.file_extensions {
                config_map.insert(ext.to_string(), config.clone());
            }
        }
        Self { indexer, config_map, root_map }
    }
    
    pub fn execute_full(&self, recipe: &Recipe) -> Result<ContextOutput<FileContent>> {
        let file_ids = self.resolve_files(&recipe.operations)?;
        let mut output_files = Vec::new();

        for file_id in file_ids {
            if let Some(file_node) = self.indexer.index.files.values().find(|f| f.id == file_id) {
                let transform = self.resolve_transform(recipe, &file_node.relative_path);
                let file_content = self.process_file(file_node, transform)?;
                output_files.push(file_content);
            }
        }

        output_files.sort_by(|a, b| a.metadata.path.cmp(&b.metadata.path));

        Ok(ContextOutput {
            target: recipe.name.clone(),
            files: output_files,
        })
    }

    pub fn execute_metadata(&self, recipe: &Recipe) -> Result<ContextOutput<FileContextMetadata>> {
        let file_ids = self.resolve_files(&recipe.operations)?;
        let mut output_files = Vec::new();

        for file_id in file_ids {
            if let Some(file_node) = self.indexer.index.files.values().find(|f| f.id == file_id) {
                let transform = self.resolve_transform(recipe, &file_node.relative_path);
                let file_content = self.process_file(file_node, transform)?;
                output_files.push(file_content.metadata);
            }
        }

        output_files.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(ContextOutput {
            target: recipe.name.clone(),
            files: output_files,
        })
    }

    pub fn get_file_content(&self, file_id: FileId, recipe: &Recipe) -> Result<Option<String>> {
        if let Some(file_node) = self.indexer.index.files.values().find(|f| f.id == file_id) {
            let transform = self.resolve_transform(recipe, &file_node.relative_path);
            let file_content = self.process_file(file_node, transform)?;
            Ok(Some(file_content.content))
        } else {
            Ok(None)
        }
    }

    // --- PHASE 1: DISCOVERY ---

    fn resolve_files(&self, operations: &[RecipeOperation]) -> Result<HashSet<FileId>> {
        let mut working_set: HashSet<FileId> = HashSet::new();

        for op in operations {
            match op {
                RecipeOperation::AddFiles { pattern } => {
                    // Pattern matching logic updated for Relative Paths
                    // The user provides a glob pattern.
                    // We match it against `FileNode.relative_path`.
                    // Note: If the user provides an absolute path from drag-drop,
                    // the GUI frontend must normalize it, OR we try to handle it here.
                    // For robustness, we handle both:
                    // 1. If pattern looks absolute, we convert it to relative if possible via path_map?
                    //    No, path_map goes Abs->ID.
                    //    Instead, we assume pattern is a Glob.
                    //    If the user drags `C:\Project\src\main.rs`, the GUI should send `src/main.rs`.
                    //    This is handled in Phase 5 (GUI). Here we assume Globs.

                    let normalized_pattern = pattern.replace('\\', "/");
                    let matcher = Glob::new(&normalized_pattern)?.compile_matcher();

                    for file in self.indexer.index.files.values() {
                        if matcher.is_match(&file.relative_path) {
                            working_set.insert(file.id);
                        }
                    }
                }
                RecipeOperation::RemoveFiles { pattern } => {
                    let normalized_pattern = pattern.replace('\\', "/");
                    let matcher = Glob::new(&normalized_pattern)?.compile_matcher();
                    let to_remove: Vec<FileId> = working_set
                        .iter()
                        .filter(|&&fid| {
                            if
                                let Some(file) = self.indexer.index.files
                                    .values()
                                    .find(|f| f.id == fid)
                            {
                                matcher.is_match(&file.relative_path)
                            } else {
                                false
                            }
                        })
                        .cloned()
                        .collect();

                    for fid in to_remove {
                        working_set.remove(&fid);
                    }
                }
                RecipeOperation::BlastRadius { symbol, max_depth, exclude_tests } => {
                    let depth_opt = if *max_depth == 0 { None } else { Some(*max_depth as usize) };
                    let mut found_ids = Vec::new();

                    // A. Attempt to find matching symbols directly
                    if let Some(ids) = self.indexer.lookup.symbol_map.get(symbol) {
                        let walker = GraphWalker::new(
                            &self.indexer.index,
                            &self.indexer.reverse_graph,
                            TraversalMode::Context,
                            depth_opt
                        );
                        found_ids = walker.walk_deep(ids);
                    } else {
                        // B. Fallback: Check if 'symbol' is a File Path (Relative)
                        let matching_file_id = self.indexer.index.files
                            .values()
                            .find(
                                |f|
                                    f.relative_path == *symbol || f.relative_path.ends_with(symbol)
                            )
                            .map(|f| f.id);

                        if let Some(fid) = matching_file_id {
                            let mut seed_ids = Vec::new();
                            for sym in self.indexer.index.symbols.values() {
                                if sym.file_id == fid {
                                    seed_ids.push(sym.id);
                                }
                            }
                            let walker = GraphWalker::new(
                                &self.indexer.index,
                                &self.indexer.reverse_graph,
                                TraversalMode::Context,
                                depth_opt
                            );
                            found_ids = walker.walk_deep(&seed_ids);
                        }
                    }

                    for sym_id in found_ids {
                        if let Some(sym) = self.indexer.index.symbols.get(&sym_id) {
                            if *exclude_tests && sym.is_test {
                                continue;
                            }
                            if !sym.is_external {
                                working_set.insert(sym.file_id);
                            }
                        }
                    }
                }
            }
        }

        Ok(working_set)
    }

    // --- PHASE 2: TRANSFORMATION ---

    fn resolve_transform<'b>(
        &self,
        recipe: &'b Recipe,
        file_path: &str
    ) -> Option<&'b FileTransform> {
        recipe.transforms
            .iter()
            .find(|(path_key, _)| { file_path.ends_with(*path_key) || file_path == **path_key })
            .map(|(_, t)| t)
    }

    fn process_file(
        &self,
        file_node: &crate::models::FileNode,
        transform: Option<&FileTransform>
    ) -> Result<FileContent> {
        // We need Absolute Path to read the file.
        // Use the id_map for O(1) lookup.
        let absolute_path = self.indexer.id_map
            .get(&file_node.id)
            .ok_or_else(||
                anyhow::anyhow!("File path not found in runtime map for ID {}", file_node.id)
            )?;

        let raw_content = fs
            ::read_to_string(absolute_path)
            .with_context(|| format!("Failed to read file: {:?}", absolute_path))?;

        let ext = absolute_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("txt")
            .to_string();

        let final_content = if let Some(tr) = transform {
            let masks = self.calculate_masks(file_node.id, tr, &ext);
            self.apply_masks(&raw_content, masks)
        } else {
            raw_content.clone()
        };

        let line_count = final_content.lines().count();
        let relevant_lines = vec![LineRange { start: 1, end: line_count.max(1) }];
        let root_name = self.root_map.get(&file_node.root_id).cloned();

        let metadata = FileContextMetadata {
            file_id: file_node.id,
            path: file_node.relative_path.clone(), // Return relative path for UI cleanliness
            root_name,
            language: ext,
            is_test: file_node.is_test,
            relevant_lines,
        };

        Ok(FileContent {
            metadata,
            content: final_content,
        })
    }

    fn calculate_masks(
        &self,
        file_id: FileId,
        transform: &FileTransform,
        ext: &str
    ) -> Vec<RenderMask> {
        let mut masks = Vec::new();

        let default_template = "{ /* ... {} body hidden ... */ }";
        let template = self.config_map
            .get(ext)
            .map(|c| c.skeleton_template)
            .unwrap_or(default_template);

        let file_symbols: Vec<_> = self.indexer.index.symbols
            .values()
            .filter(|s| s.file_id == file_id)
            .collect();

        for sym in file_symbols {
            let body_start = match sym.body_start {
                Some(bs) => bs,
                None => {
                    continue;
                }
            };

            if body_start >= sym.range_end {
                continue;
            }

            let should_mask = match transform {
                FileTransform::Skeletonize(targets) => targets.contains(&sym.name),
                FileTransform::FocusOn(targets) => {
                    let is_container = matches!(
                        sym.kind,
                        SymbolKind::Container | SymbolKind::Module
                    );
                    let is_target = targets.contains(&sym.name);
                    !is_container && !is_target
                }
            };

            if should_mask {
                masks.push(RenderMask {
                    start: body_start,
                    end: sym.range_end,
                    replacement: template.replace("{}", &sym.name),
                });
            }
        }

        masks
    }

    fn apply_masks(&self, source: &str, mut masks: Vec<RenderMask>) -> String {
        if masks.is_empty() {
            return source.to_string();
        }

        masks.sort_by_key(|m| m.start);

        let mut filtered_masks: Vec<RenderMask> = Vec::new();
        let mut max_end_so_far = 0;

        for mask in masks {
            if !source.is_char_boundary(mask.start) || !source.is_char_boundary(mask.end) {
                continue;
            }

            if mask.start >= max_end_so_far {
                max_end_so_far = mask.end;
                filtered_masks.push(mask);
            }
        }

        let mut result = String::with_capacity(source.len());
        let mut last_pos = 0;

        for mask in filtered_masks {
            if mask.start > last_pos {
                result.push_str(&source[last_pos..mask.start]);
            }
            result.push_str(&mask.replacement);
            last_pos = mask.end;
        }

        if last_pos < source.len() {
            result.push_str(&source[last_pos..]);
        }

        result
    }
}