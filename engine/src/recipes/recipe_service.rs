use crate::models::{FileId};
use crate::recipes::models::{Recipe, RecipeOperation};
use crate::recipes::executor::RecipeExecutor;
use crate::workspace::WorkspaceManager;
use crate::query::output::{ContextOutput, FileContent, FileContextMetadata};
use std::path::PathBuf;
use anyhow::{Result, Context};
use serde::{Serialize, Deserialize};

/// A unified result type that can hold either lightweight metadata or full content.
#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)] // This makes JSON output clean (no "Full": {...} wrapper)
pub enum RecipeOutput {
    Full(ContextOutput<FileContent>),
    Metadata(ContextOutput<FileContextMetadata>),
}

impl RecipeOutput {
    /// Helper to convert result to XML (only works if Full, otherwise errors)
    pub fn to_xml(&self) -> Result<String> {
        match self {
            RecipeOutput::Full(output) => Ok(output.to_xml()),
            RecipeOutput::Metadata(_) => anyhow::bail!("Cannot generate XML from metadata-only output"),
        }
    }
}

pub struct RecipeService;

impl RecipeService {
    
    /// Normalizes a Recipe received from UI or CLI.
    /// Handles converting Absolute Paths (Drag & Drop) to Relative Paths.
    pub fn normalize_recipe(manager: &WorkspaceManager, mut recipe: Recipe) -> Recipe {
        for op in &mut recipe.operations {
            match op {
                RecipeOperation::AddFiles { pattern } | RecipeOperation::RemoveFiles { pattern } => {
                    let path_buf = PathBuf::from(&*pattern);

                    // Case 1: Handle Absolute Paths (e.g. from Drag & Drop / CLI args)
                    if path_buf.is_absolute() && path_buf.exists() {
                        let canonical = std::fs::canonicalize(&path_buf).unwrap_or(path_buf.clone());
                        // Try to find which root contains this path
                        for root in &manager.config.roots {
                            if canonical.starts_with(&root.path) {
                                if let Ok(rel) = canonical.strip_prefix(&root.path) {
                                    *pattern = rel.to_string_lossy().replace('\\', "/");
                                    break;
                                }
                            }
                        }
                    } 
                    // Case 2: Ensure separators are normalized for Engine
                    else {
                        // Don't check exists() for globs like "*.ts"
                        if !pattern.contains('*') && !pattern.contains('?') {
                             *pattern = pattern.replace('\\', "/");
                        }
                    }
                }
                _ => {}
            }
        }
        recipe
    }

    /// The Unified Entry Point.
    /// 1. Normalizes paths.
    /// 2. Sets up Executor.
    /// 3. Returns either Full content or Metadata based on the flag.
    pub fn execute(
        manager: &WorkspaceManager, 
        recipe: Recipe, 
        full_content: bool
    ) -> Result<RecipeOutput> {
        let normalized = Self::normalize_recipe(manager, recipe);
        let root_map = manager.get_root_map();
        let executor = RecipeExecutor::new(&manager.index, root_map);

        if full_content {
            let output = executor.execute_full(&normalized)
                .context("Failed to execute recipe (full content)")?;
            Ok(RecipeOutput::Full(output))
        } else {
            let output = executor.execute_metadata(&normalized)
                .context("Failed to execute recipe (metadata)")?;
            Ok(RecipeOutput::Metadata(output))
        }
    }

    /// Specific entry point for getting a single file's transformed content.
    /// Used by the GUI to preview what a file looks like inside a specific recipe.
    pub fn get_file_preview(
        manager: &WorkspaceManager,
        recipe: Recipe,
        file_id: FileId
    ) -> Result<Option<String>> {
        let normalized = Self::normalize_recipe(manager, recipe);
        let root_map = manager.get_root_map();
        let executor = RecipeExecutor::new(&manager.index, root_map);
        
        executor.get_file_content(file_id, &normalized)
    }
}