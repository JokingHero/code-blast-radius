use crate::models::BoundaryIndex;
use crate::recipes::models::{Recipe, RecipeOperation};
use crate::resolution::persistence::PersistenceManager;
use crate::resolution::scanner::FileScanner;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

// --- Config Structs ---

/// Represents a source code root folder within the workspace.
#[derive(Debug, Clone)]
pub struct RootConfig {
    pub id: String,
    pub path: PathBuf,
}

/// The logical configuration of a workspace.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceConfig {
    pub name: String,
    pub roots: Vec<RootConfig>,
    pub recipes: HashMap<String, Recipe>,
}

// --- Persistence DTOs ---
// These ensure the JSON file remains portable (using relative paths) 
// and decoupled from the internal runtime representation.

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

// --- Manager ---

/// The central coordinator for the Engine.
/// Manages configuration, the high-performance search index, and disk I/O.
pub struct WorkspaceManager {
    /// If Some, this workspace is linked to a `.cblast` file on disk.
    /// If None, it is an ad-hoc in-memory session.
    pub backing_file: Option<PathBuf>,
    
    pub config: WorkspaceConfig,
    
    /// The high-performance, inverted index of the codebase.
    pub index: BoundaryIndex,
}

impl WorkspaceManager {
    
    /// Helper: Resolves a path to an absolute PathBuf, handling Windows UNC prefixes.
    /// This ensures consistent path hashing and lookups in the index.
    fn resolve_path_safe(path: PathBuf) -> PathBuf {
        let absolute = if path.is_absolute() {
            path
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(path)
        };
        
        // Canonicalize to resolve symlinks and '..' components
        let canonical = fs::canonicalize(&absolute).unwrap_or(absolute);

        let s = canonical.to_string_lossy();
        if s.starts_with(r"\\?\") {
            PathBuf::from(&s[4..])
        } else {
            canonical
        }
    }

    /// Loads a workspace from a `.cblast` file.
    /// 
    /// This process:
    /// 1. Reads the JSON config.
    /// 2. Attempts to load the binary index (`.cblast.index`).
    ///    *Note: If the index version mismatches the app version, a fresh index is returned.*
    /// 3. Syncs the index with the file system (Incremental or Full Rebuild).
    /// 4. Prunes stale data from recipes to ensure consistency.
    pub fn from_file(path: PathBuf) -> Result<Self> {
        let abs_path = Self::resolve_path_safe(path);
        let base_dir = abs_path.parent().unwrap_or_else(|| Path::new("."));

        // 1. Load Config (JSON)
        let content = fs::read_to_string(&abs_path)
            .context(format!("Could not read workspace file: {:?}", abs_path))?;
        
        let persisted: PersistedWorkspaceConfig =
            serde_json::from_str(&content).context("Failed to parse workspace JSON")?;

        // Reconstruct roots with absolute paths
        let mut roots = Vec::new();
        for pr in persisted.roots {
            // Handle cross-platform path separators
            let os_path = if cfg!(windows) {
                pr.path.replace('/', "\\")
            } else {
                pr.path
            };
            
            let raw = PathBuf::from(os_path);
            let final_path = if raw.is_relative() {
                base_dir.join(raw)
            } else {
                raw
            };
            
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

        // 2. Load Index (Binary / Zero-Copy)
        let index_path = abs_path.with_extension("cblast.index");
        let persistence = PersistenceManager::new();
        
        // This load_index call implicitly handles version checking.
        // If versions differ, it returns BoundaryIndex::new(), triggering a full scan in sync().
        let index = persistence
            .load_index(&index_path)
            .unwrap_or_else(|_| BoundaryIndex::new());

        let mut manager = Self {
            backing_file: Some(abs_path),
            config,
            index,
        };

        // 3. Sync
        // Scans the file system. If index was empty (rebuild), this populates it.
        // If index was loaded, this updates only changed files (blake3 hash check).
        manager.sync();

        // 4. Sanitize
        // Ensure that recipes do not point to symbols or files that no longer exist
        // (e.g., after a git pull or a version upgrade that changed parsing logic).
        manager.prune_stale_data();

        // Optional: We could auto-save the index here if it changed, 
        // but we generally defer disk writes to explicit user actions 
        // to avoid "magic" side effects.

        Ok(manager)
    }

    /// Creates a temporary in-memory workspace. Useful for "Open Folder" mode.
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
            name: roots
                .first()
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

        // Perform initial scan
        manager.sync();
        
        Ok(manager)
    }

    /// Saves the current workspace to its backing file.
    pub fn save(&self) -> Result<()> {
        let path = self
            .backing_file
            .as_ref()
            .ok_or_else(|| anyhow!("Cannot save in-memory workspace. Use save_as()"))?;
        
        let base_dir = path.parent().unwrap_or(Path::new("."));

        // 1. Serialize Config to JSON (Portable Paths)
        let persisted_roots: Vec<PersistedRoot> = self
            .config
            .roots
            .iter()
            .map(|r| {
                // Calculate relative path for portability
                let rel = pathdiff::diff_paths(&r.path, base_dir).unwrap_or_else(|| r.path.clone());
                PersistedRoot {
                    id: r.id.clone(),
                    path: rel.to_string_lossy().replace('\\', "/"), // Force forward slash for JSON
                }
            })
            .collect();

        let p_config = PersistedWorkspaceConfig {
            name: self.config.name.clone(),
            roots: persisted_roots,
            recipes: self.config.recipes.clone(),
        };

        fs::write(path, serde_json::to_string_pretty(&p_config)?)
            .context("Failed to write workspace config")?;

        // 2. Serialize Index to Binary (rkyv)
        let index_path = path.with_extension("cblast.index");
        let persistence = PersistenceManager::new();
        persistence.save_index(&self.index, &index_path)?;

        Ok(())
    }

    /// Associates an in-memory workspace with a file path and saves it.
    pub fn save_as(&mut self, path: PathBuf) -> Result<()> {
        let abs = Self::resolve_path_safe(path);
        self.backing_file = Some(abs);
        
        // Update name based on filename
        if let Some(stem) = self.backing_file.as_ref().unwrap().file_stem() {
            self.config.name = stem.to_string_lossy().to_string();
        }
        
        self.save()
    }

    /// Adds a new root folder to the workspace and immediately scans it.
    pub fn add_root(&mut self, path: PathBuf) {
        let abs = Self::resolve_path_safe(path);
        
        // Prevent duplicate roots
        if self.config.roots.iter().any(|r| abs.starts_with(&r.path)) {
            return; 
        }

        let new_root = RootConfig {
            id: Uuid::new_v4().to_string(),
            path: abs,
        };
        self.config.roots.push(new_root.clone());

        // Scan *only* the new root for efficiency
        let scanner = FileScanner::new();
        scanner.scan(&new_root.path, &mut self.index, &new_root.id);
    }

    /// Removes a root folder and cleans up the index.
    pub fn remove_root(&mut self, path: PathBuf) {
        let abs = Self::resolve_path_safe(path);
        
        if let Some(idx) = self.config.roots.iter().position(|r| r.path == abs) {
            let root_id = self.config.roots[idx].id.clone();
            self.config.roots.remove(idx);

            // Cleanup files in index associated with this root
            let ids_to_remove: Vec<_> = self
                .index
                .files
                .values()
                .filter(|f| f.root_id == root_id)
                .map(|f| f.id)
                .collect();

            for id in ids_to_remove {
                self.index.files.remove(&id);
            }

            // Rebuild maps (Symbol, Path, Usage) to ensure consistency.
            // Unlike adding, removing invalidates cross-references so we clear maps.
            self.index.symbol_map.clear();
            self.index.path_map.clear();
            self.index.usage_map.clear();
            
            // Re-populate maps from remaining files
            // (Note: This could be optimized to not re-scan files, just re-map, 
            // but re-scanning ensures absolute correctness)
            let scanner = FileScanner::new();
            for root in &self.config.roots {
                // In a future optimization, split scan() from rebuild_maps()
                scanner.scan(&root.path, &mut self.index, &root.id);
            }
        }
    }

    /// Refresh the workspace: Re-scans all roots to detect file changes.
    pub fn sync(&mut self) {
        let scanner = FileScanner::new();
        for root in &self.config.roots {
            scanner.scan(&root.path, &mut self.index, &root.id);
        }
    }

    /// Helper for the Executor to map Root IDs back to absolute paths on disk.
    pub fn get_root_map(&self) -> HashMap<String, String> {
        self.config
            .roots
            .iter()
            .map(|r| (r.id.clone(), r.path.to_string_lossy().to_string()))
            .collect()
    }

    /// Removes invalid symbols and files from recipes based on the current index state.
    /// This is crucial when loading old workspaces into a new version of the app,
    /// or after significant code refactoring.
    fn prune_stale_data(&mut self) {
        let mut modification_log = Vec::new();

        for (recipe_name, recipe) in self.config.recipes.iter_mut() {
            // 1. Prune Operations
            let initial_ops = recipe.operations.len();
            recipe.operations.retain(|op| {
                match op {
                    RecipeOperation::BlastRadius { symbol, .. } => {
                        // Keep operation if the symbol exists in the map
                        // OR if it matches a known file path suffix
                        let is_symbol = self.index.symbol_map.contains_key(symbol);
                        let is_file = !is_symbol && self.index.files.values().any(|f| f.path.ends_with(symbol));
                        
                        is_symbol || is_file
                    }
                    // Keep pattern-based operations (AddFiles/RemoveFiles).
                    // Even if they match nothing *now*, they represent a persistent intent (e.g. "include *.rs").
                    _ => true, 
                }
            });
            
            if recipe.operations.len() != initial_ops {
                modification_log.push(format!(
                    "Pruned {} dead operations from '{}'", 
                    initial_ops - recipe.operations.len(), 
                    recipe_name
                ));
            }

            // 2. Prune Transforms
            let initial_trans = recipe.transforms.len();
            recipe.transforms.retain(|path_suffix, _| {
                // Only keep transforms for files that actually exist in the index
                self.index.files.values().any(|f| f.path.ends_with(path_suffix))
            });
            
            if recipe.transforms.len() != initial_trans {
                modification_log.push(format!(
                    "Pruned {} dead transforms from '{}'", 
                    initial_trans - recipe.transforms.len(), 
                    recipe_name
                ));
            }
        }

        // Only log if we are in a context where stdout is visible (CLI), 
        // or just for general debugging.
        if !modification_log.is_empty() {
            println!("Workspace Sanitization Report:");
            for log in modification_log {
                println!(" - {}", log);
            }
        }
    }
}