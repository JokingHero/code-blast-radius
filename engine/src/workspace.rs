use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{PathBuf};
use std::fs;
use anyhow::{Result, Context};
use crate::recipes::models::Recipe;
use crate::resolution::{Indexer, pipeline::Pipeline};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct WorkspaceConfig {
    pub name: String,
    pub roots: Vec<PathBuf>,
    pub recipes: HashMap<String, Recipe>,
}

pub struct WorkspaceManager {
    pub config_path: PathBuf,
    pub config: WorkspaceConfig,
    pub indexer: Indexer,
}

impl WorkspaceManager {
    /// Load an existing workspace or create a new in-memory context for one
    pub fn new(config_path: PathBuf) -> Result<Self> {
        let config = if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            serde_json::from_str(&content)?
        } else {
            WorkspaceConfig::default()
        };

        // Index file is always: project.cblast -> project.cblast.index
        let index_path = config_path.with_extension("cblast.index");
        let indexer = Indexer::load_from_file(&index_path).unwrap_or(Indexer::new());

        Ok(Self { config_path, config, indexer })
    }

    pub fn save(&self) -> Result<()> {
        // 1. Save Config JSON
        let json = serde_json::to_string_pretty(&self.config)?;
        fs::write(&self.config_path, json).context("Failed to write config")?;

        // 2. Save Binary Index
        let index_path = self.config_path.with_extension("cblast.index");
        self.indexer.save(&index_path).context("Failed to save index")?;
        Ok(())
    }

    /// Adds a root, scans it, and updates the graph
    pub fn add_root(&mut self, root: PathBuf) {
        let abs_root = fs::canonicalize(&root).unwrap_or(root);
        if !self.config.roots.contains(&abs_root) {
            self.config.roots.push(abs_root.clone());
            
            // Incremental Scan of just this new root
            let pipeline = Pipeline::new();
            pipeline.scan(&mut self.indexer, &abs_root);
            
            // Re-resolve everything to link new code to old code
            self.rebuild_graph();
        }
    }

    /// Removes a root and cleans up the graph
    pub fn remove_root(&mut self, root: PathBuf) {
        let abs_root = fs::canonicalize(&root).unwrap_or(root);
        if let Some(pos) = self.config.roots.iter().position(|r| *r == abs_root) {
            self.config.roots.remove(pos);
            
            // Remove data from index
            self.indexer.remove_root(&abs_root);
            
            // Re-resolve remaining edges
            self.rebuild_graph();
        }
    }

    /// The "Refresh" Method: Rescans all roots to catch file changes
    pub fn sync(&mut self) {
        let pipeline = Pipeline::new();
        
        // 1. Scan all configured roots (Handles Add/Mod/Del of files)
        for root in &self.config.roots {
            pipeline.scan(&mut self.indexer, root);
        }

        // 2. Re-resolve relationships
        self.rebuild_graph();
    }

    fn rebuild_graph(&mut self) {
        let mut pipeline = Pipeline::new();
        // hydrate_staging is public now
        let mut staging = pipeline.hydrate_staging(&self.indexer.index);
        pipeline.resolve(&mut self.indexer, &mut staging);
    }
}