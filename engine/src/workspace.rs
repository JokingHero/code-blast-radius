use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{PathBuf};
use std::fs;
use anyhow::{Result, Context, anyhow};
use crate::recipes::models::Recipe;
use crate::resolution::{Indexer, pipeline::Pipeline};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct WorkspaceConfig {
    pub name: String,
    pub roots: Vec<PathBuf>,
    pub recipes: HashMap<String, Recipe>,
}

pub struct WorkspaceManager {
    // None = Unsaved (Ad-Hoc), Some = Linked to a .cblast file
    pub backing_file: Option<PathBuf>, 
    pub config: WorkspaceConfig,
    pub indexer: Indexer,
}

impl WorkspaceManager {
    /// Case 1: Load from an existing .cblast file
    pub fn from_file(path: PathBuf) -> Result<Self> {
        let content = fs::read_to_string(&path)
            .context(format!("Could not read workspace file: {:?}", path))?;
        
        let config: WorkspaceConfig = serde_json::from_str(&content)
            .context("Failed to parse workspace JSON")?;

        // Look for sibling index file: project.cblast -> project.cblast.index
        let index_path = path.with_extension("cblast.index");
        let indexer = Indexer::load_from_file(&index_path).unwrap_or(Indexer::new());

        // We run a sync immediately to ensure AST validity vs Disk
        let mut manager = Self { 
            backing_file: Some(path), 
            config, 
            indexer 
        };
        manager.sync(); // Optional: Might be slow on startup, maybe make this lazy?
        
        Ok(manager)
    }

    /// Case 2: Create "Ad-Hoc" from one or more folders (Unsaved)
    pub fn new_in_memory(roots: Vec<PathBuf>) -> Result<Self> {
        let mut indexer = Indexer::new();
        
        // Optimization: If it's a single root, try to load existing cache from `.cblast/index.local.bin`
        // This mimics your previous GUI logic but keeps it in the engine.
        if roots.len() == 1 {
            let cache_path = roots[0].join(".cblast").join("index.local.bin");
            if cache_path.exists() {
                if let Ok(loaded) = Indexer::load_from_file(&cache_path) {
                    indexer = loaded;
                }
            }
        }

        let abs_roots: Vec<PathBuf> = roots.into_iter()
            .map(|r| fs::canonicalize(&r).unwrap_or(r))
            .collect();

        // Initial Scan
        let mut pipeline = Pipeline::new();
        for root in &abs_roots {
            pipeline.scan(&mut indexer, root);
        }
        
        // Initial Resolve
        let mut staging = pipeline.hydrate_staging(&indexer.index);
        pipeline.resolve(&mut indexer, &mut staging);

        // Create Config
        let name = abs_roots.first()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled Workspace".to_string());

        Ok(Self {
            backing_file: None, // explicit: not saved yet
            config: WorkspaceConfig {
                name,
                roots: abs_roots,
                recipes: HashMap::new(),
            },
            indexer
        })
    }

    /// Save to the current backing file. Fails if InMemory.
    pub fn save(&self) -> Result<()> {
        let path = self.backing_file.as_ref()
            .ok_or_else(|| anyhow!("Cannot save an in-memory workspace. Use save_as() first."))?;
        
        // 1. Save Config
        let json = serde_json::to_string_pretty(&self.config)?;
        fs::write(path, json).context("Failed to write config")?;

        // 2. Save Index
        let index_path = path.with_extension("cblast.index");
        self.indexer.save(&index_path).context("Failed to save index")?;

        Ok(())
    }

    /// Promote In-Memory to File-Backed, OR Save As new path
    pub fn save_as(&mut self, path: PathBuf) -> Result<()> {
        self.backing_file = Some(path);
        // Ensure the config name matches the filename (optional UX choice)
        if let Some(stem) = self.backing_file.as_ref().unwrap().file_stem() {
            self.config.name = stem.to_string_lossy().to_string();
        }
        self.save()
    }

    // ... [add_root, remove_root, sync, rebuild_graph remain mostly the same] ...
    // ... just ensure add_root calls sync/rebuild ...
    
    pub fn add_root(&mut self, root: PathBuf) {
        let abs_root = fs::canonicalize(&root).unwrap_or(root);
        if !self.config.roots.contains(&abs_root) {
            self.config.roots.push(abs_root.clone());
            let pipeline = Pipeline::new();
            pipeline.scan(&mut self.indexer, &abs_root);
            self.rebuild_graph();
        }
    }
    
    pub fn remove_root(&mut self, root: PathBuf) {
         let abs_root = fs::canonicalize(&root).unwrap_or(root);
         if let Some(pos) = self.config.roots.iter().position(|r| *r == abs_root) {
            self.config.roots.remove(pos);
            self.indexer.remove_root(&abs_root);
            self.rebuild_graph();
         }
    }

    pub fn sync(&mut self) {
        let pipeline = Pipeline::new();
        for root in &self.config.roots {
            pipeline.scan(&mut self.indexer, root);
        }
        
        // If we are in Ad-Hoc mode (single root), let's update the local cache
        // to keep the folder-open experience fast next time.
        if self.backing_file.is_none() && self.config.roots.len() == 1 {
             let cache_dir = self.config.roots[0].join(".cblast");
             let _ = fs::create_dir_all(&cache_dir);
             let _ = self.indexer.save(&cache_dir.join("index.local.bin"));
        }

        self.rebuild_graph();
    }

    fn rebuild_graph(&mut self) {
        let mut pipeline = Pipeline::new();
        let mut staging = pipeline.hydrate_staging(&self.indexer.index);
        pipeline.resolve(&mut self.indexer, &mut staging);
    }
}