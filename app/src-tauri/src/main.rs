#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::{Emitter, Manager};
use tokio::sync::RwLock;
// File Watching
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::Mutex;

// Engine Imports
use blast_radius_engine::models::FileId; // u32
use blast_radius_engine::recipes::executor::RecipeExecutor;
use blast_radius_engine::recipes::models::{Recipe, RecipeOperation};
use blast_radius_engine::workspace::WorkspaceManager;
use nucleo_matcher::{Config, Matcher, Utf32String};

// Local Modules
mod settings;
use settings::SettingsState;

// --- State Definitions ---

struct AppState {
    manager: RwLock<Option<WorkspaceManager>>,
    watcher: Mutex<Option<RecommendedWatcher>>,
}

// --- DTOs ---

#[derive(serde::Serialize)]
struct SearchResult {
    name: String,
    kind: String,
    path: String,
    score: u16,
}

#[derive(serde::Serialize)]
struct ResolvedPathDTO {
    original: String,
    // If successfully resolved to an indexed file:
    relative_path: Option<String>,
    root_id: Option<String>,
    // If not found in index, is it at least inside a root?
    is_indexed: bool,
}

// Update the WorkspaceConfigDTO to include Root IDs (needed for UI mapping)
#[derive(serde::Serialize, Clone)]
struct RootConfigDTO {
    id: String,
    path: String,
    name: String,
}

#[derive(serde::Serialize, Clone)]
struct WorkspaceConfigDTO {
    name: String,
    roots: Vec<RootConfigDTO>,
    mode: String,
}

#[derive(serde::Serialize)]
struct AppSettingsDTO {
    recent: Vec<String>,
    last_opened: Option<String>,
}

// --- Helpers ---

fn map_to_dto(manager: &WorkspaceManager) -> WorkspaceConfigDTO {
    let mode = if manager.backing_file.is_some() {
        "project".to_string()
    } else if manager.config.roots.len() > 1 {
        "unsaved-workspace".to_string()
    } else {
        "ad-hoc".to_string()
    };

    let roots = manager
        .config
        .roots
        .iter()
        .map(|r| {
            let path_str = r.path.to_string_lossy().to_string();
            // Simple heuristic for a name: last component of the path
            let name = r
                .path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path_str.clone());

            RootConfigDTO {
                id: r.id.clone(),
                path: path_str,
                name,
            }
        })
        .collect();

    WorkspaceConfigDTO {
        name: manager.config.name.clone(),
        roots,
        mode,
    }
}

fn setup_watcher(app: &tauri::AppHandle, roots: Vec<PathBuf>) -> Option<RecommendedWatcher> {
    let app_handle = app.clone();
    let last_emit = std::sync::Arc::new(std::sync::Mutex::new(std::time::Instant::now()));

    let event_handler = move |res: notify::Result<Event>| match res {
        Ok(event) => {
            if let EventKind::Access(_) = event.kind {
                return;
            }

            let is_relevant = event.paths.iter().any(|p| {
                let path_str = p.to_string_lossy();
                !path_str.contains(".cblast")
                    && !path_str.contains(".git")
                    && !path_str.contains("node_modules")
            });

            if is_relevant {
                let mut last = last_emit.lock().unwrap();
                if last.elapsed().as_millis() > 500 {
                    let _ = app_handle.emit("workspace:dirty", ());
                    *last = std::time::Instant::now();
                }
            }
        }
        Err(e) => eprintln!("Watch error: {:?}", e),
    };

    let mut watcher = notify::recommended_watcher(event_handler).ok()?;

    for root in roots {
        if let Err(e) = watcher.watch(&root, RecursiveMode::Recursive) {
            eprintln!("Failed to watch root {:?}: {}", root, e);
        }
    }

    Some(watcher)
}

/// Normalizes a Recipe received from the UI.
/// The UI might send Absolute Paths (e.g. from Drag & Drop) or Relative Paths (e.g. typed manually).
/// This function converts Absolute paths to Relative paths if they reside within a Workspace Root,
/// and validates Relative paths against the Roots to ensure they refer to actual files.
fn normalize_recipe(manager: &WorkspaceManager, mut recipe: Recipe) -> Recipe {
    for op in &mut recipe.operations {
        match op {
            RecipeOperation::AddFiles { pattern } | RecipeOperation::RemoveFiles { pattern } => {
                let path_buf = PathBuf::from(&*pattern);

                // Case 1: Handle Absolute Paths (e.g. from Drag & Drop)
                if path_buf.is_absolute() {
                    // Only try to resolve if the file actually exists on disk
                    if path_buf.exists() {
                        let canonical =
                            std::fs::canonicalize(&path_buf).unwrap_or(path_buf.clone());

                        // Try to find which root contains this path
                        for root in &manager.config.roots {
                            if canonical.starts_with(&root.path) {
                                if let Ok(rel) = canonical.strip_prefix(&root.path) {
                                    // Successfully mapped Absolute -> Relative
                                    *pattern = rel.to_string_lossy().replace('\\', "/");
                                    break;
                                }
                            }
                        }
                    }
                }
                // Case 2: Handle Relative Paths (e.g. typed manually)
                else {
                    // Do NOT check path_buf.exists() here, as that relies on the process CWD
                    // (Current Working Directory), which is unpredictable in a GUI app.

                    // Skip glob patterns, we can't resolve existence for them
                    if pattern.contains('*') || pattern.contains('?') {
                        continue;
                    }

                    // Check if this relative path exists inside any known root
                    for root in &manager.config.roots {
                        let candidate = root.path.join(&path_buf);
                        if candidate.exists() {
                            // It is a valid file in this root.
                            // We normalize separators (Windows \ -> /) to ensure consistency.
                            *pattern = path_buf.to_string_lossy().replace('\\', "/");
                            break;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    recipe
}
// --- Commands ---

#[tauri::command]
async fn get_global_settings(
    state: tauri::State<'_, SettingsState>,
) -> Result<AppSettingsDTO, String> {
    let settings = state.settings.lock().unwrap();
    Ok(AppSettingsDTO {
        recent: settings.recent_workspaces.clone(),
        last_opened: settings.last_opened_path.clone(),
    })
}

#[tauri::command]
async fn clear_recent_history(state: tauri::State<'_, SettingsState>) -> Result<(), String> {
    state.clear_recent();
    Ok(())
}

#[tauri::command]
async fn set_workspace(
    path: String,
    state: tauri::State<'_, AppState>,
    settings_state: tauri::State<'_, SettingsState>,
    app_handle: tauri::AppHandle,
) -> Result<WorkspaceConfigDTO, String> {
    let target_path = PathBuf::from(&path);

    if !target_path.exists() {
        settings_state.remove_recent(&path);
        return Err("ERR_WORKSPACE_NOT_FOUND".into());
    }

    let manager = if target_path.is_dir() {
        // Ad-hoc mode
        WorkspaceManager::new_in_memory(vec![target_path]).map_err(|e| e.to_string())?
    } else {
        // File-backed mode
        WorkspaceManager::from_file(target_path.clone()).map_err(|e| e.to_string())?
    };

    settings_state.update_recent(path.clone());

    let dto = map_to_dto(&manager);
    // extract paths from RootConfig for watcher
    let roots = manager
        .config
        .roots
        .iter()
        .map(|r| r.path.clone())
        .collect();

    let mut guard = state.manager.write().await;
    *guard = Some(manager);

    let mut watcher_guard = state.watcher.lock().unwrap();
    *watcher_guard = setup_watcher(&app_handle, roots);

    Ok(dto)
}

#[tauri::command]
async fn save_current_workspace(
    path: String,
    state: tauri::State<'_, AppState>,
    settings_state: tauri::State<'_, SettingsState>,
) -> Result<WorkspaceConfigDTO, String> {
    let mut guard = state.manager.write().await;
    let manager = guard.as_mut().ok_or("No active workspace")?;

    let target_path = PathBuf::from(&path);
    manager
        .save_as(target_path.clone())
        .map_err(|e| e.to_string())?;

    settings_state.update_recent(path);

    Ok(map_to_dto(manager))
}

#[tauri::command]
async fn add_root_to_workspace(
    root_path: String,
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<WorkspaceConfigDTO, String> {
    let mut guard = state.manager.write().await;
    let manager = guard.as_mut().ok_or("No active workspace")?;

    let path = PathBuf::from(root_path);
    if !path.exists() {
        return Err("Path does not exist".into());
    }

    manager.add_root(path);

    if manager.backing_file.is_some() {
        manager.save().map_err(|e| e.to_string())?;
    }

    let dto = map_to_dto(manager);
    let roots = manager
        .config
        .roots
        .iter()
        .map(|r| r.path.clone())
        .collect();

    let mut watcher_guard = state.watcher.lock().unwrap();
    *watcher_guard = setup_watcher(&app_handle, roots);

    Ok(dto)
}

#[tauri::command]
async fn remove_root_from_workspace(
    root_path: String,
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<WorkspaceConfigDTO, String> {
    let mut guard = state.manager.write().await;
    let manager = guard.as_mut().ok_or("No active workspace")?;

    let path = PathBuf::from(root_path);
    manager.remove_root(path);

    if manager.backing_file.is_some() {
        manager.save().map_err(|e| e.to_string())?;
    }

    let dto = map_to_dto(manager);
    let roots = manager
        .config
        .roots
        .iter()
        .map(|r| r.path.clone())
        .collect();

    let mut watcher_guard = state.watcher.lock().unwrap();
    *watcher_guard = setup_watcher(&app_handle, roots);

    Ok(dto)
}

#[tauri::command]
async fn refresh_workspace(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.manager.write().await;
    let manager = guard.as_mut().ok_or("No active workspace")?;

    manager.sync();

    if manager.backing_file.is_some() {
        manager.save().map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
async fn search_symbols(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    let guard = state.manager.read().await;
    let manager = guard.as_ref().ok_or("Workspace not loaded")?;
    let index = &manager.index;

    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut results = Vec::new();
    let query_utf32 = Utf32String::from(query.as_str());

    for (name, ids) in &index.symbol_map {
        if let Some(score) = matcher.fuzzy_match(
            Utf32String::from(name.as_str()).slice(..),
            query_utf32.slice(..),
        ) {
            // Just use the first file occurrence
            if let Some(&file_id) = ids.first() {
                if let Some(file_node) = index.files.get(&file_id) {
                    
                    // Try to find the physical definition first (Function, Class, etc.)
                    let kind = file_node
                        .defs
                        .iter()
                        .find(|d| d.name == *name)
                        .map(|d| format!("{:?}", d.kind))
                        // If not found, check Synthetic Definitions (Routes, DI, etc.)
                        .or_else(|| {
                            if file_node.synthetic_defs.contains(name) {
                                // Extract the "Concept Type" from the string.
                                // Examples: 
                                // "route:GET:/api/users" -> "ROUTE"
                                // "di:UserService"       -> "DI"
                                // "view:UserCard"        -> "VIEW"
                                let concept_type = name
                                    .split(':')
                                    .next()
                                    .unwrap_or("Concept")
                                    .to_uppercase();
                                
                                Some(concept_type)
                            } else {
                                None
                            }
                        })
                        .unwrap_or_else(|| "Unknown".to_string());

                    results.push(SearchResult {
                        name: name.clone(),
                        kind,
                        path: file_node.path.clone(),
                        score,
                    });
                }
            }
        }
    }

    // Search Files (Match against Relative Path)
    for file in index.files.values() {
        let display_name = file.path.clone();

        if let Some(score) = matcher.fuzzy_match(
            Utf32String::from(display_name.as_str()).slice(..),
            query_utf32.slice(..),
        ) {
            results.push(SearchResult {
                name: display_name.clone(),
                kind: "File".to_string(),
                path: display_name,
                score,
            });
        }
    }

    results.sort_by(|a, b| b.score.cmp(&a.score));
    results.truncate(20);
    Ok(results)
}

fn get_root_map(manager: &WorkspaceManager) -> HashMap<String, String> {
    manager
        .config
        .roots
        .iter()
        .map(|r| (r.id.clone(), r.path.to_string_lossy().to_string()))
        .collect()
}

#[tauri::command]
async fn execute_recipe(
    recipe_json: serde_json::Value,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let guard = state.manager.read().await;
    let manager = guard.as_ref().ok_or("Workspace not loaded")?;
    let mut recipe: Recipe =
        serde_json::from_value(recipe_json).map_err(|e| format!("Invalid recipe format: {}", e))?;

    // Normalize paths (Abs -> Rel)
    recipe = normalize_recipe(manager, recipe);
    let root_map = get_root_map(manager);
    // manager.index instead of manager.indexer
    let executor = RecipeExecutor::new(&manager.index, root_map);

    // Metadata only!
    let output = executor
        .execute_metadata(&recipe)
        .map_err(|e| format!("Execution failed: {}", e))?;

    serde_json::to_string(&output).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_file_content(
    file_id: FileId,
    recipe_json: serde_json::Value,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let guard = state.manager.read().await;
    let manager = guard.as_ref().ok_or("Workspace not loaded")?;
    let mut recipe: Recipe =
        serde_json::from_value(recipe_json).map_err(|e| format!("Invalid recipe format: {}", e))?;

    recipe = normalize_recipe(manager, recipe);
    let root_map = get_root_map(manager);
    let executor = RecipeExecutor::new(&manager.index, root_map);

    match executor.get_file_content(file_id, &recipe) {
        Ok(Some(content)) => Ok(content),
        Ok(None) => Err("File not found".into()),
        Err(e) => Err(format!("Failed to read file: {}", e)),
    }
}

#[tauri::command]
async fn generate_xml_bundle(
    recipe_json: serde_json::Value,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let guard = state.manager.read().await;
    let manager = guard.as_ref().ok_or("Workspace not loaded")?;
    let mut recipe: Recipe =
        serde_json::from_value(recipe_json).map_err(|e| format!("Invalid recipe format: {}", e))?;

    recipe = normalize_recipe(manager, recipe);
    let root_map = get_root_map(manager);
    let executor = RecipeExecutor::new(&manager.index, root_map);

    // Full content execution
    let output = executor
        .execute_full(&recipe)
        .map_err(|e| format!("Execution failed: {}", e))?;

    Ok(output.to_xml())
}

#[tauri::command]
async fn get_saved_recipes(state: tauri::State<'_, AppState>) -> Result<Vec<Recipe>, String> {
    let guard = state.manager.read().await;
    let manager = guard.as_ref().ok_or("No active workspace")?;
    let recipes: Vec<Recipe> = manager.config.recipes.values().cloned().collect();
    Ok(recipes)
}

#[tauri::command]
async fn save_named_recipe(
    recipe_json: serde_json::Value,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut guard = state.manager.write().await;
    let manager = guard.as_mut().ok_or("No active workspace")?;
    let mut recipe: Recipe =
        serde_json::from_value(recipe_json).map_err(|e| format!("Invalid recipe format: {}", e))?;

    // Normalize before saving so the saved recipe is portable
    recipe = normalize_recipe(manager, recipe);

    let name = recipe.name.clone();
    manager.config.recipes.insert(name, recipe);

    if manager.backing_file.is_some() {
        manager.save().map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
async fn delete_named_recipe(
    name: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut guard = state.manager.write().await;
    let manager = guard.as_mut().ok_or("No active workspace")?;
    if manager.config.recipes.remove(&name).is_some() {
        if manager.backing_file.is_some() {
            manager.save().map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

#[tauri::command]
async fn resolve_paths(
    paths: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ResolvedPathDTO>, String> {
    let guard = state.manager.read().await;
    let manager = guard.as_ref().ok_or("Workspace not loaded")?;
    let index = &manager.index;

    let mut results = Vec::new();

    for path_str in paths {
        let raw_path = PathBuf::from(&path_str);
        let abs_path = if raw_path.exists() {
            std::fs::canonicalize(&raw_path).unwrap_or(raw_path)
        } else {
            raw_path
        };

        // Strip Windows UNC prefix (\\?\) for consistent matching
        // The engine logic usually stores clean paths, so we must match clean paths.
        let to_clean_path = |p: &PathBuf| -> PathBuf {
            let s = p.to_string_lossy().to_string();
            if s.starts_with(r"\\?\") {
                PathBuf::from(&s[4..])
            } else {
                p.clone()
            }
        };

        let clean_lookup = to_clean_path(&abs_path);
        let mut found_match = false;

        for root in &manager.config.roots {
            let clean_root = to_clean_path(&root.path);

            if clean_lookup.starts_with(&clean_root) {
                if let Ok(rel) = clean_lookup.strip_prefix(&clean_root) {
                    
                    // Normalize to Forward Slashes for Index Lookup
                    // The Engine's BoundaryIndex strictly uses "/" as a separator.
                    let rel_str = rel.to_string_lossy().replace('\\', "/");

                    // We check path_map (which handles fuzzy matches) AND exact file paths
                    let mut indexed_file_node = None;

                    // A. Direct exact match check
                    for file in index.files.values() {
                        if file.path == rel_str && file.root_id == root.id {
                            indexed_file_node = Some(file);
                            break;
                        }
                    }

                    // B. Fallback to path_map if A failed (though A should suffice for exact paths)
                    if indexed_file_node.is_none() {
                        if let Some(ids) = index.path_map.get(&rel_str) {
                            for &id in ids {
                                if let Some(f) = index.files.get(&id) {
                                    // Ensure we matched the file in THIS root
                                    if f.root_id == root.id {
                                        indexed_file_node = Some(f);
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    if let Some(file_node) = indexed_file_node {
                        results.push(ResolvedPathDTO {
                            original: path_str.clone(),
                            relative_path: Some(file_node.path.clone()),
                            root_id: Some(file_node.root_id.clone()),
                            is_indexed: true,
                        });
                        found_match = true;
                    } else {
                        // File exists physically in root, but not in index (e.g. .gitignore or new file)
                        results.push(ResolvedPathDTO {
                            original: path_str.clone(),
                            relative_path: Some(rel_str),
                            root_id: Some(root.id.clone()),
                            is_indexed: false,
                        });
                        found_match = true;
                    }

                    if found_match { break; }
                }
            }
        }

        if !found_match {
            // Completely outside workspace
            results.push(ResolvedPathDTO {
                original: path_str,
                relative_path: None,
                root_id: None,
                is_indexed: false,
            });
        }
    }

    Ok(results)
}

// --- Entry Point ---
fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let settings_state = SettingsState::new(app.handle());
            app.manage(settings_state);
            Ok(())
        })
        .manage(AppState {
            manager: RwLock::new(None),
            watcher: Mutex::new(None),
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            set_workspace,
            add_root_to_workspace,
            remove_root_from_workspace,
            save_current_workspace,
            search_symbols,
            refresh_workspace,
            get_global_settings,
            clear_recent_history,
            get_saved_recipes,
            save_named_recipe,
            delete_named_recipe,
            execute_recipe,
            get_file_content,
            generate_xml_bundle,
            resolve_paths,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
