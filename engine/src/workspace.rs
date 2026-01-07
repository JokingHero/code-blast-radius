use serde::{ Deserialize, Serialize };
use std::collections::HashMap;
use std::path::{ Path, PathBuf };
use std::fs;
use anyhow::{ Result, Context, anyhow };
use uuid::Uuid;
use crate::recipes::models::Recipe;
use crate::resolution::{ Indexer, pipeline::Pipeline };

// --- Config Structs ---
#[derive(Debug, Clone)]
pub struct RootConfig {
    pub id: String,
    pub path: PathBuf,
}

// --- Runtime Configuration (In-Memory, Absolute Paths) ---
#[derive(Debug, Clone, Default)]
pub struct WorkspaceConfig {
    pub name: String,
    pub roots: Vec<RootConfig>,
    pub recipes: HashMap<String, Recipe>,
}

#[derive(Serialize, Deserialize)]
struct PersistedRoot {
    id: String,
    path: String, // Unix style relative path
}

// --- Persisted Configuration (JSON) ---
#[derive(Serialize, Deserialize)]
struct PersistedWorkspaceConfig {
    name: String,
    roots: Vec<PersistedRoot>,
    recipes: HashMap<String, Recipe>,
}

pub struct WorkspaceManager {
    // None = Unsaved (Ad-Hoc), Some = Linked to a .cblast file
    pub backing_file: Option<PathBuf>,
    pub config: WorkspaceConfig,
    pub indexer: Indexer,
}

impl WorkspaceManager {
    /// Resolves a path to an absolute PathBuf.
    fn resolve_path_safe(path: PathBuf) -> PathBuf {
        let absolute = if path.is_absolute() {
            path
        } else {
            std::env
                ::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(path)
        };
        fs::canonicalize(&absolute).unwrap_or(absolute)
    }

    /// Case 1: Load from an existing .cblast file
    pub fn from_file(path: PathBuf) -> Result<Self> {
        let abs_path = Self::resolve_path_safe(path);
        let base_dir = abs_path.parent().unwrap_or_else(|| Path::new("."));

        let content = fs
            ::read_to_string(&abs_path)
            .context(format!("Could not read workspace file: {:?}", abs_path))?;

        let persisted: PersistedWorkspaceConfig = serde_json
            ::from_str(&content)
            .context("Failed to parse workspace JSON")?;

        let mut roots = Vec::new();
        for persisted_root in persisted.roots {
            let os_path_str = if cfg!(windows) {
                persisted_root.path.replace('/', "\\")
            } else {
                persisted_root.path
            };

            let path_obj = PathBuf::from(os_path_str);
            let final_path = if path_obj.is_relative() {
                base_dir.join(path_obj)
            } else {
                path_obj
            };

            let canonical = fs::canonicalize(&final_path).unwrap_or(final_path);

            roots.push(RootConfig {
                id: persisted_root.id,
                path: canonical,
            });
        }

        let config = WorkspaceConfig {
            name: persisted.name,
            roots,
            recipes: persisted.recipes,
        };

        let index_path = abs_path.with_extension("cblast.index");
        let indexer = Indexer::load_from_file(&index_path).unwrap_or(Indexer::new());

        // PHASE 2: Hydrate the path map immediately after loading
        let mut manager = Self {
            backing_file: Some(abs_path),
            config,
            indexer,
        };

        manager.rebuild_runtime_maps();

        // Sync will scan and resolve
        manager.sync();

        Ok(manager)
    }

    /// Case 2: Create "Ad-Hoc" from one or more folders (Unsaved)
    pub fn new_in_memory(root_paths: Vec<PathBuf>) -> Result<Self> {
        let mut indexer = Indexer::new();
        let mut roots = Vec::new();

        // Apply Safe Resolution
        for path in root_paths {
            let abs_root = Self::resolve_path_safe(path);

            // Nested Root Prevention (Naive)
            let is_nested = roots.iter().any(|r: &RootConfig| abs_root.starts_with(&r.path));
            if !is_nested {
                // If existing root is nested inside new root, replace it (Merge Up)
                roots.retain(|r: &RootConfig| !r.path.starts_with(&abs_root));

                roots.push(RootConfig {
                    id: Uuid::new_v4().to_string(),
                    path: abs_root,
                });
            }
        }

        // Optimization: Single root cache loading
        if roots.len() == 1 {
            let cache_path = roots[0].path.join(".cblast").join("index.local.bin");
            if cache_path.exists() {
                if let Ok(loaded) = Indexer::load_from_file(&cache_path) {
                    indexer = loaded;
                }
            }
        }

        let config = WorkspaceConfig {
            name: roots
                .first()
                .and_then(|r| r.path.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Untitled Workspace".to_string()),
            roots,
            recipes: HashMap::new(),
        };

        let mut manager = Self {
            backing_file: None,
            config,
            indexer,
        };

        // Hydrate before initial scan to ensure any loaded cache is mapped
        manager.rebuild_runtime_maps();

        let pipeline = Pipeline::new();
        for root in &manager.config.roots {
            pipeline.scan(&mut manager.indexer, &root.path, Some(&root.id));
        }

        manager.rebuild_graph();

        Ok(manager)
    }

    pub fn save(&self) -> Result<()> {
        let path = self.backing_file
            .as_ref()
            .ok_or_else(|| anyhow!("Cannot save an in-memory workspace. Use save_as() first."))?;

        let base_dir = path.parent().unwrap_or_else(|| Path::new("."));

        let mut persisted_roots = Vec::new();
        for root in &self.config.roots {
            let relative_path = pathdiff
                ::diff_paths(&root.path, base_dir)
                .unwrap_or_else(|| root.path.clone());
            let path_str = relative_path.to_string_lossy().to_string();
            let portable_path = path_str.replace('\\', "/");

            persisted_roots.push(PersistedRoot {
                id: root.id.clone(),
                path: portable_path,
            });
        }

        let persisted_config = PersistedWorkspaceConfig {
            name: self.config.name.clone(),
            roots: persisted_roots,
            recipes: self.config.recipes.clone(),
        };

        let json = serde_json::to_string_pretty(&persisted_config)?;
        fs::write(path, json).context("Failed to write config")?;

        let index_path = path.with_extension("cblast.index");
        self.indexer.save(&index_path).context("Failed to save index")?;

        Ok(())
    }

    pub fn save_as(&mut self, path: PathBuf) -> Result<()> {
        let abs_path = Self::resolve_path_safe(path);
        self.backing_file = Some(abs_path);

        if let Some(stem) = self.backing_file.as_ref().unwrap().file_stem() {
            self.config.name = stem.to_string_lossy().to_string();
        }
        self.save()
    }

    pub fn add_root(&mut self, root_path: PathBuf) {
        let abs_root = Self::resolve_path_safe(root_path);

        // Check if already exists or nested
        let is_nested_in_existing = self.config.roots.iter().any(|r| abs_root.starts_with(&r.path));
        if is_nested_in_existing {
            println!("Skipping nested root: {:?}", abs_root);
            return;
        }

        // Check if new root is parent of existing roots (Merge Up)
        let mut to_remove = Vec::new();
        for (i, r) in self.config.roots.iter().enumerate() {
            if r.path.starts_with(&abs_root) {
                to_remove.push(i);
            }
        }

        for i in to_remove.iter().rev() {
            let id = self.config.roots[*i].id.clone();
            self.indexer.remove_root(&id);
            self.config.roots.remove(*i);
        }

        let new_root = RootConfig {
            id: Uuid::new_v4().to_string(),
            path: abs_root,
        };

        self.config.roots.push(new_root.clone());

        // Rebuild runtime maps with new root
        self.rebuild_runtime_maps();

        let pipeline = Pipeline::new();
        pipeline.scan(&mut self.indexer, &new_root.path, Some(&new_root.id));
        self.rebuild_graph();
    }

    pub fn remove_root(&mut self, root_path: PathBuf) {
        let abs_root = Self::resolve_path_safe(root_path);

        if let Some(pos) = self.config.roots.iter().position(|r| r.path == abs_root) {
            let id = self.config.roots[pos].id.clone();
            self.config.roots.remove(pos);
            self.indexer.remove_root(&id);

            // Rebuild maps AFTER removing the root to clear invalid entries
            self.rebuild_runtime_maps();
            self.rebuild_graph();
        }
    }

    pub fn sync(&mut self) {
        // Ensure path maps are up to date before scanning
        self.rebuild_runtime_maps();

        let pipeline = Pipeline::new();
        for root in &self.config.roots {
            pipeline.scan(&mut self.indexer, &root.path, Some(&root.id));
        }

        if self.backing_file.is_none() && self.config.roots.len() == 1 {
            let cache_dir = self.config.roots[0].path.join(".cblast");
            let _ = fs::create_dir_all(&cache_dir);
            let _ = self.indexer.save(&cache_dir.join("index.local.bin"));
        }

        self.rebuild_graph();
    }

    // Helper: Construct the Transient Map (AbsPath -> FileId)
    // This allows the Scanner and Resolver to work with O(1) lookups
    fn rebuild_runtime_maps(&mut self) {
        let pipeline = Pipeline::new();

        // Convert Vec<RootConfig> to HashMap for the hydrator
        let mut active_roots = HashMap::new();
        for r in &self.config.roots {
            active_roots.insert(r.id.clone(), r.path.clone());
        }

        let (pm, im) = pipeline.hydrate_maps(&self.indexer.index, &active_roots);
        self.indexer.path_map = pm;
        self.indexer.id_map = im;
    }

    fn rebuild_graph(&mut self) {
        let mut pipeline = Pipeline::new();
        let mut staging = pipeline.hydrate_staging(&self.indexer.index);
        
        // Extract paths from config to pass to the resolver
        let active_root_paths: Vec<PathBuf> = self.config.roots
            .iter()
            .map(|r| r.path.clone())
            .collect();

        pipeline.resolve(&mut self.indexer, &mut staging, &active_root_paths);
    }
}