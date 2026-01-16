use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use anyhow::{Result, Context, anyhow};
use uuid::Uuid;
use crate::recipes::models::Recipe;
use crate::models::{BoundaryIndex}; // Removed unused FileId
use crate::resolution::scanner::FileScanner;
use crate::resolution::persistence::PersistenceManager;

// --- Config Structs ---
#[derive(Debug, Clone)]
pub struct RootConfig {
    pub id: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct WorkspaceConfig {
    pub name: String,
    pub roots: Vec<RootConfig>,
    pub recipes: HashMap<String, Recipe>,
}

// JSON Persistence Model
#[derive(Serialize, Deserialize)]
struct PersistedRoot {
    id: String,
    path: String, // Portable relative path
}

#[derive(Serialize, Deserialize)]
struct PersistedWorkspaceConfig {
    name: String,
    roots: Vec<PersistedRoot>,
    recipes: HashMap<String, Recipe>,
}

pub struct WorkspaceManager {
    // None = In-Memory only, Some = Linked to a .cblast file
    pub backing_file: Option<PathBuf>,
    pub config: WorkspaceConfig,
    pub index: BoundaryIndex,
}

impl WorkspaceManager {
    /// Resolves a path to an absolute PathBuf, handling UNC prefixes on Windows.
    fn resolve_path_safe(path: PathBuf) -> PathBuf {
        let absolute = if path.is_absolute() {
            path
        } else {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(path)
        };
        let canonical = fs::canonicalize(&absolute).unwrap_or(absolute);
        
        let s = canonical.to_string_lossy();
        if s.starts_with(r"\\?\") {
            PathBuf::from(&s[4..])
        } else {
            canonical
        }
    }

    pub fn from_file(path: PathBuf) -> Result<Self> {
        let abs_path = Self::resolve_path_safe(path);
        let base_dir = abs_path.parent().unwrap_or_else(|| Path::new("."));

        // 1. Load Config
        let content = fs::read_to_string(&abs_path)
            .context(format!("Could not read workspace file: {:?}", abs_path))?;
        let persisted: PersistedWorkspaceConfig = serde_json::from_str(&content)
            .context("Failed to parse workspace JSON")?;

        let mut roots = Vec::new();
        for pr in persisted.roots {
            let os_path = if cfg!(windows) { pr.path.replace('/', "\\") } else { pr.path };
            let raw = PathBuf::from(os_path);
            let final_path = if raw.is_relative() { base_dir.join(raw) } else { raw };
            roots.push(RootConfig {
                id: pr.id,
                path: fs::canonicalize(&final_path).unwrap_or(final_path),
            });
        }

        let config = WorkspaceConfig {
            name: persisted.name,
            roots,
            recipes: persisted.recipes,
        };

        // 2. Load Index
        let index_path = abs_path.with_extension("cblast.index");
        let persistence = PersistenceManager::new();
        let index = persistence.load_index(&index_path).unwrap_or_else(|_| BoundaryIndex::new());

        let mut manager = Self {
            backing_file: Some(abs_path),
            config,
            index,
        };

        // 3. Sync (Scan for changes)
        manager.sync();

        Ok(manager)
    }

    pub fn new_in_memory(root_paths: Vec<PathBuf>) -> Result<Self> {
        let mut roots = Vec::new();
        for path in root_paths {
            let abs = Self::resolve_path_safe(path);
            roots.push(RootConfig {
                id: Uuid::new_v4().to_string(),
                path: abs,
            });
        }

        let config = WorkspaceConfig {
            name: roots.first()
                .and_then(|r| r.path.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Untitled".to_string()),
            roots: roots.clone(),
            recipes: HashMap::new(),
        };

        let mut manager = Self {
            backing_file: None,
            config,
            index: BoundaryIndex::new(),
        };

        manager.sync();
        Ok(manager)
    }

    pub fn save(&self) -> Result<()> {
        let path = self.backing_file.as_ref()
            .ok_or_else(|| anyhow!("Cannot save in-memory workspace. Use save_as()"))?;
        let base_dir = path.parent().unwrap_or(Path::new("."));

        // 1. Save Config JSON
        let persisted_roots: Vec<PersistedRoot> = self.config.roots.iter().map(|r| {
             let rel = pathdiff::diff_paths(&r.path, base_dir).unwrap_or_else(|| r.path.clone());
            PersistedRoot {
                id: r.id.clone(),
                path: rel.to_string_lossy().replace('\\', "/"),
            }
        }).collect();

        let p_config = PersistedWorkspaceConfig {
            name: self.config.name.clone(),
            roots: persisted_roots,
            recipes: self.config.recipes.clone(),
        };

        fs::write(path, serde_json::to_string_pretty(&p_config)?)?;

        // 2. Save Index Binary
        let index_path = path.with_extension("cblast.index");
        let persistence = PersistenceManager::new();
        persistence.save_index(&self.index, &index_path)?;

        Ok(())
    }

    pub fn save_as(&mut self, path: PathBuf) -> Result<()> {
        let abs = Self::resolve_path_safe(path);
        self.backing_file = Some(abs);
        if let Some(stem) = self.backing_file.as_ref().unwrap().file_stem() {
            self.config.name = stem.to_string_lossy().to_string();
        }
        self.save()
    }

    pub fn add_root(&mut self, path: PathBuf) {
        let abs = Self::resolve_path_safe(path);
        if self.config.roots.iter().any(|r| abs.starts_with(&r.path)) {
            return; // Skip nested
        }
        
        let new_root = RootConfig {
            id: Uuid::new_v4().to_string(),
            path: abs,
        };
        self.config.roots.push(new_root.clone());
        
        // Scan only the new root
        let scanner = FileScanner::new();
        scanner.scan(&new_root.path, &mut self.index, &new_root.id);
    }

    pub fn remove_root(&mut self, path: PathBuf) {
        let abs = Self::resolve_path_safe(path);
        if let Some(idx) = self.config.roots.iter().position(|r| r.path == abs) {
            let root_id = self.config.roots[idx].id.clone();
            self.config.roots.remove(idx);
            
            // Cleanup files in index
            let ids_to_remove: Vec<_> = self.index.files.values()
                .filter(|f| f.root_id == root_id)
                .map(|f| f.id)
                .collect();
            
            for id in ids_to_remove {
                self.index.files.remove(&id);
            }
            
            // Rebuild maps to clear symbols from removed files
            if let Some(first) = self.config.roots.first() {
                let scanner = FileScanner::new(); 
                scanner.scan(&first.path, &mut self.index, &first.id);
            } else {
                self.index.symbol_map.clear();
                self.index.path_map.clear();
            }
        }
    }

    pub fn sync(&mut self) {
        let scanner = FileScanner::new();
        for root in &self.config.roots {
            scanner.scan(&root.path, &mut self.index, &root.id);
        }
    }
    
    // Helper for RecipeExecutor
    pub fn get_root_map(&self) -> HashMap<String, String> {
        self.config.roots.iter()
            .map(|r| (r.id.clone(), r.path.to_string_lossy().to_string()))
            .collect()
    }
}