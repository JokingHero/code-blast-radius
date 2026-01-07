use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use anyhow::{Result, Context, anyhow};
use crate::recipes::models::Recipe;
use crate::resolution::{Indexer, pipeline::Pipeline};

// --- Runtime Configuration (In-Memory, Absolute Paths) ---
#[derive(Debug, Clone, Default)]
pub struct WorkspaceConfig {
    pub name: String,
    pub roots: Vec<PathBuf>,
    pub recipes: HashMap<String, Recipe>,
}

// --- Persisted Configuration (JSON, Relative Paths, Forward Slashes) ---
#[derive(Serialize, Deserialize)]
struct PersistedWorkspaceConfig {
    name: String,
    roots: Vec<String>, // Stored as strings to enforce unix-style separators
    recipes: HashMap<String, Recipe>,
}

pub struct WorkspaceManager {
    // None = Unsaved (Ad-Hoc), Some = Linked to a .cblast file
    pub backing_file: Option<PathBuf>, 
    pub config: WorkspaceConfig,
    pub indexer: Indexer,
}

impl WorkspaceManager {
    
    // --- HELPER: Safe Path Resolution ---
    /// Resolves a path to an absolute PathBuf.
    /// 1. If relative, joins with CWD.
    /// 2. Attempts to canonicalize (resolve symlinks/..).
    /// 3. If canonicalization fails (e.g. restrictive permissions), falls back to the calculated absolute path.
    fn resolve_path_safe(path: PathBuf) -> PathBuf {
        let absolute = if path.is_absolute() {
            path
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(path)
        };

        // unwrap_or(absolute) is the key fix: it prevents panics or failures 
        // if the OS filesystem call fails, keeping the logical absolute path instead.
        fs::canonicalize(&absolute).unwrap_or(absolute)
    }

    /// Case 1: Load from an existing .cblast file
    pub fn from_file(path: PathBuf) -> Result<Self> {
        // We use the helper here too for the workspace file itself
        let abs_path = Self::resolve_path_safe(path);
        let base_dir = abs_path.parent().unwrap_or_else(|| Path::new("."));

        let content = fs::read_to_string(&abs_path)
            .context(format!("Could not read workspace file: {:?}", abs_path))?;
        
        // Deserialize into the DTO
        let persisted: PersistedWorkspaceConfig = serde_json::from_str(&content)
            .context("Failed to parse workspace JSON")?;

        // Convert DTO -> Runtime Config
        let mut roots = Vec::new();
        for root_str in persisted.roots {
            // 1. Handle OS separators (Convert forward slash to native if on Windows)
            let os_path_str = if cfg!(windows) {
                root_str.replace('/', "\\")
            } else {
                root_str
            };
            
            let path_obj = PathBuf::from(os_path_str);

            // 2. Resolve Relative Paths (Relative to the WORKSPACE FILE, not CWD)
            let final_path = if path_obj.is_relative() {
                base_dir.join(path_obj)
            } else {
                path_obj
            };

            // 3. Canonicalize Safe Fallback
            let canonical = fs::canonicalize(&final_path).unwrap_or(final_path);
            roots.push(canonical);
        }

        let config = WorkspaceConfig {
            name: persisted.name,
            roots,
            recipes: persisted.recipes,
        };

        // Look for sibling index file: project.cblast -> project.cblast.index
        // Using "with_extension" on "project.cblast" results in "project.cblast.index"
        // Note: standard with_extension replaces the last extension, so we append manually to be safe or ensure naming convention.
        // If file is "my.workspace.cblast", with_extension("index") makes "my.workspace.index".
        // Current logic assumes specific naming convention.
        let index_path = abs_path.with_extension("cblast.index");
        let indexer = Indexer::load_from_file(&index_path).unwrap_or(Indexer::new());

        let mut manager = Self { 
            backing_file: Some(abs_path), 
            config, 
            indexer 
        };
        
        // Ensure index matches files on disk
        manager.sync(); 
        
        Ok(manager)
    }

    /// Case 2: Create "Ad-Hoc" from one or more folders (Unsaved)
    pub fn new_in_memory(roots: Vec<PathBuf>) -> Result<Self> {
        let mut indexer = Indexer::new();
        
        // Apply Safe Resolution to inputs (Fixes CLI '.' relative paths)
        let abs_roots: Vec<PathBuf> = roots.into_iter()
            .map(Self::resolve_path_safe)
            .collect();

        // Optimization: If it's a single root, try to load existing cache from `.cblast/index.local.bin`
        if abs_roots.len() == 1 {
            let cache_path = abs_roots[0].join(".cblast").join("index.local.bin");
            if cache_path.exists() {
                if let Ok(loaded) = Indexer::load_from_file(&cache_path) {
                    indexer = loaded;
                }
            }
        }

        // Initial Scan
        let mut pipeline = Pipeline::new();
        for root in &abs_roots {
            pipeline.scan(&mut indexer, root);
        }
        
        // Initial Resolve
        let mut staging = pipeline.hydrate_staging(&indexer.index);
        pipeline.resolve(&mut indexer, &mut staging);

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
        
        let base_dir = path.parent().unwrap_or_else(|| Path::new("."));

        // Convert Runtime Config -> Persisted DTO
        let mut persisted_roots = Vec::new();
        for root in &self.config.roots {
            // 1. Calculate relative path from .cblast file to root
            let relative_path = pathdiff::diff_paths(root, base_dir).unwrap_or_else(|| root.clone());
            
            // 2. Normalize to Forward Slashes (Portable JSON)
            let path_str = relative_path.to_string_lossy().to_string();
            let portable_path = path_str.replace('\\', "/");
            
            persisted_roots.push(portable_path);
        }

        let persisted_config = PersistedWorkspaceConfig {
            name: self.config.name.clone(),
            roots: persisted_roots,
            recipes: self.config.recipes.clone(),
        };

        // 1. Save Config
        let json = serde_json::to_string_pretty(&persisted_config)?;
        fs::write(path, json).context("Failed to write config")?;

        // 2. Save Index
        let index_path = path.with_extension("cblast.index");
        self.indexer.save(&index_path).context("Failed to save index")?;

        Ok(())
    }

    /// Promote In-Memory to File-Backed, OR Save As new path
    pub fn save_as(&mut self, path: PathBuf) -> Result<()> {
        let abs_path = Self::resolve_path_safe(path);
        self.backing_file = Some(abs_path);
        
        if let Some(stem) = self.backing_file.as_ref().unwrap().file_stem() {
            self.config.name = stem.to_string_lossy().to_string();
        }
        self.save()
    }

    pub fn add_root(&mut self, root: PathBuf) {
        // Apply Safe Resolution (Fixes CLI relative paths)
        let abs_root = Self::resolve_path_safe(root);

        if !self.config.roots.contains(&abs_root) {
            self.config.roots.push(abs_root.clone());
            let pipeline = Pipeline::new();
            pipeline.scan(&mut self.indexer, &abs_root);
            self.rebuild_graph();
        }
    }
    
    pub fn remove_root(&mut self, root: PathBuf) {
         // Apply Safe Resolution so we can match what is stored in config.roots
         let abs_root = Self::resolve_path_safe(root);

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