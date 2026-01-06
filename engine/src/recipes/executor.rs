use std::collections::{ HashMap, HashSet };
use std::fs;
use std::path::Path;
use anyhow::{ Result, Context };
use globset::{ Glob };

use crate::analysis::language::{ LanguageConfig, get_language_configs };
use crate::resolution::Indexer;
use crate::query::traversal::find_related_symbols;
use crate::query::output::{ ContextOutput, FileContext, LineRange };
use crate::recipes::models::{ Recipe, RecipeOperation, FileTransform };
use crate::models::{ SymbolKind, FileId };

/// Internal struct to track text replacements
#[derive(Debug, Clone)]
struct RenderMask {
    start: usize,
    end: usize,
    replacement: String,
}

pub struct RecipeExecutor<'a> {
    indexer: &'a Indexer,
    config_map: HashMap<String, LanguageConfig>,
}

impl<'a> RecipeExecutor<'a> {
    pub fn new(indexer: &'a Indexer) -> Self {
        let mut config_map = HashMap::new();
        for config in get_language_configs() {
            for &ext in config.file_extensions {
                config_map.insert(ext.to_string(), config.clone());
            }
        }
        Self { indexer, config_map }
    }

    /// Main entry point: Executes a recipe against the current Indexer state.
    /// PRECONDITION: The caller must have run `workspace.sync()` (or pipeline.scan)
    /// immediately before calling this to ensure AST offsets match file content on disk.
    pub fn execute(&self, recipe: &Recipe) -> Result<ContextOutput> {
        // 1. THE NET: Discover relevant files based on operations
        let file_ids = self.resolve_files(&recipe.operations)?;

        // 2. THE LENS: Transform and render content
        let mut output_files = Vec::new();

        for file_id in file_ids {
            if let Some(file_node) = self.indexer.index.files.values().find(|f| f.id == file_id) {
                // Determine if there is a transform for this specific file
                // We match based on the file path stored in the index
                // Note: Index paths are usually normalized. Recipe paths might be relative.
                // We attempt a suffix match or exact match.
                let transform = recipe.transforms
                    .iter()
                    .find(|(path_key, _)| {
                        file_node.path.ends_with(*path_key) || file_node.path == **path_key
                    })
                    .map(|(_, t)| t);

                let file_context = self.process_file(file_node, transform)?;
                output_files.push(file_context);
            }
        }

        // Sort for deterministic output (by path)
        output_files.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(ContextOutput {
            target: recipe.name.clone(),
            files: output_files,
        })
    }

    // --- PHASE 1: DISCOVERY ---

    fn resolve_files(&self, operations: &[RecipeOperation]) -> Result<HashSet<FileId>> {
        let mut working_set: HashSet<FileId> = HashSet::new();

        for op in operations {
            match op {
                RecipeOperation::AddFiles { pattern } => {
                    let normalized_pattern = pattern.replace('\\', "/"); 
                    let matcher = Glob::new(&normalized_pattern)?.compile_matcher();

                    for file in self.indexer.index.files.values() {
                        let path_str = file.path.as_str();
                        
                        // Remove Windows "\\?\" prefix if it exists
                        let clean_start = if path_str.starts_with("\\\\?\\") { 4 } else { 0 };
                        let clean_path = &path_str[clean_start..];

                        let normalized_path = clean_path.replace('\\', "/");

                        if matcher.is_match(&normalized_path) {
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
                            if let Some(file) = self.indexer.index.files.values().find(|f| f.id == fid) {
                                let path_str = file.path.as_str();

                                // Remove Windows "\\?\" prefix if it exists
                                let clean_start = if path_str.starts_with("\\\\?\\") { 4 } else { 0 };
                                let clean_path = &path_str[clean_start..];

                                // Normalize slashes to match the pattern format
                                let normalized_path = clean_path.replace('\\', "/");

                                matcher.is_match(&normalized_path)
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
                    // Convert u32 to Option<usize> for the walker.
                    // 0 = Infinite (None)
                    // >0 = Specific depth limit (Some(n))
                    let depth_opt = if *max_depth == 0 { 
                        None 
                    } else { 
                        Some(*max_depth as usize) 
                    };

                    if
                        let Some(related_ids) = find_related_symbols(
                            &self.indexer.index,
                            &self.indexer.lookup,
                            &self.indexer.reverse_graph,
                            symbol, // &String works fine here as &str
                            depth_opt
                        )
                    {
                        for sym_id in related_ids {
                            if let Some(sym) = self.indexer.index.symbols.get(&sym_id) {
                                if *exclude_tests && sym.is_test {
                                    continue;
                                }
                                // Don't add external library "files" (which have ID 0 usually or virtual)
                                if !sym.is_external {
                                    working_set.insert(sym.file_id);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(working_set)
    }

    // --- PHASE 2: TRANSFORMATION ---

    fn process_file(
        &self,
        file_node: &crate::models::FileNode,
        transform: Option<&FileTransform>
    ) -> Result<FileContext> {
        let raw_content = fs
            ::read_to_string(&file_node.path)
            .with_context(|| format!("Failed to read file: {}", file_node.path))?;

        // Metadata extraction
        let ext = Path::new(&file_node.path)
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

        // Calculate line ranges (naive: generic range covering whole file for now,
        // as Recipes usually return whole files or skeletonized whole files).
        let line_count = final_content.lines().count();
        let relevant_lines = vec![LineRange { start: 1, end: line_count.max(1) }];

        Ok(FileContext {
            path: file_node.path.clone(),
            language: ext,
            is_test: file_node.is_test,
            relevant_lines,
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

        // Lookup skeleton template
        // Default to C-style if extension unknown or language not configured
        let default_template = "{ /* ... {} body hidden ... */ }";
        let template = self.config_map
            .get(ext)
            .map(|c| c.skeleton_template)
            .unwrap_or(default_template);

        // Retrieve all symbols for this file
        // We iterate directly as we don't have a file_id -> [Symbol] lookup optimized,
        // but index.symbols is flat map.
        let file_symbols: Vec<_> = self.indexer.index.symbols
            .values()
            .filter(|s| s.file_id == file_id)
            .collect();

        for sym in file_symbols {
            // We only skeletonize things that HAVE a body
            let body_start = match sym.body_start {
                Some(bs) => bs,
                None => {
                    continue;
                }
            };

            // Safety: body_start cannot be after range_end
            if body_start >= sym.range_end {
                continue;
            }

            // Decide whether to mask based on Transform logic
            let should_mask = match transform {
                FileTransform::Skeletonize(targets) => {
                    // "Deny-List": Hide ONLY these
                    targets.contains(&sym.name)
                }
                FileTransform::FocusOn(targets) => {
                    // "Allow-List": Hide everything EXCEPT these...
                    // ...BUT: Never hide Containers (Classes/Modules), because we need their shell
                    // to see the children inside. We only mask "leaves" (Functions/Methods).
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

        // 1. Sort by start position to process sequentially
        masks.sort_by_key(|m| m.start);

        // 2. Filter overlaps.
        // If we have nested masks (e.g. inner function inside outer function),
        // we likely want to keep the OUTER mask (which hides everything) and skip the inner one.
        // Since we sorted by start, an outer mask will appear before an inner mask
        // (assuming outer starts before or at inner).
        let mut filtered_masks: Vec<RenderMask> = Vec::new();
        let mut max_end_so_far = 0;

        for mask in masks {
            // Validate char boundaries to prevent panics
            if !source.is_char_boundary(mask.start) || !source.is_char_boundary(mask.end) {
                eprintln!(
                    "Warning: Skipping mask at {}-{} due to invalid char boundary",
                    mask.start,
                    mask.end
                );
                continue;
            }

            if mask.start >= max_end_so_far {
                max_end_so_far = mask.end;
                filtered_masks.push(mask);
            } else {
                // Overlap detected: This mask starts before the previous one ended.
                // Since we sorted by start, this means it's likely "inside" the previous one.
                // We skip it because the outer mask covers it.
            }
        }

        // 3. Construct String
        let mut result = String::with_capacity(source.len());
        let mut last_pos = 0;

        for mask in filtered_masks {
            // Append content before the mask
            // Safety: last_pos is updated from mask.end, which we checked is_char_boundary
            // mask.start is checked is_char_boundary
            if mask.start > last_pos {
                result.push_str(&source[last_pos..mask.start]);
            }

            // Append replacement
            result.push_str(&mask.replacement);

            last_pos = mask.end;
        }

        // Append remaining content
        if last_pos < source.len() {
            result.push_str(&source[last_pos..]);
        }

        result
    }
}