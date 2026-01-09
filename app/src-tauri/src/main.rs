#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::collections::HashMap;
use std::path::{ PathBuf };
use tokio::sync::RwLock;
use tauri::{ Manager, Emitter };
// File Watching
use notify::{ RecommendedWatcher, RecursiveMode, Watcher, Event, EventKind };
use std::sync::Mutex;

// Engine Imports
use blast_radius_engine::workspace::WorkspaceManager;
use blast_radius_engine::recipes::executor::RecipeExecutor;
use blast_radius_engine::recipes::models::{ Recipe, RecipeOperation };
use blast_radius_engine::models::FileId; // u32
use nucleo_matcher::{ Matcher, Config, Utf32String };

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

    let roots = manager.config.roots
        .iter()
        .map(|r| {
            let path_str = r.path.to_string_lossy().to_string();
            // Simple heuristic for a name: last component of the path
            let name = r.path.file_name()
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

    let event_handler = move |res: notify::Result<Event>| {
        match res {
            Ok(event) => {
                if let EventKind::Access(_) = event.kind {
                    return;
                }

                let is_relevant = event.paths.iter().any(|p| {
                    let path_str = p.to_string_lossy();
                    !path_str.contains(".cblast") &&
                        !path_str.contains(".git") &&
                        !path_str.contains("node_modules")
                });

                if is_relevant {
                    let _ = app_handle.emit("workspace:dirty", ());
                }
            }
            Err(e) => eprintln!("Watch error: {:?}", e),
        }
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
/// The UI might send Absolute Paths (e.g. from Drag & Drop).
/// This function looks them up in the path_map and converts them to relative_path globs
/// to ensure the recipe works with the engine's Relative Path logic.
fn normalize_recipe(manager: &WorkspaceManager, mut recipe: Recipe) -> Recipe {
    for op in &mut recipe.operations {
        match op {
            RecipeOperation::AddFiles { pattern } | RecipeOperation::RemoveFiles { pattern } => {
                // Check if pattern is an Absolute Path that exists in our index
                let path_buf = PathBuf::from(&*pattern);
                // We attempt to canonicalize if it exists to match the keys in path_map
                // If it doesn't exist (e.g. deleted file), we rely on raw path lookup or leave as is
                let lookup_path = if path_buf.exists() {
                    std::fs::canonicalize(&path_buf).unwrap_or(path_buf)
                } else {
                    path_buf
                };

                if let Some(&id) = manager.indexer.path_map.get(&lookup_path) {
                    // It's a known file! Retrieve its relative path.
                    // We have to scan values because we don't have ID->Node map in O(1) inside this scope easily
                    // Actually `index.files` is Key->Node. `index.files` values are nodes.
                    if let Some(node) = manager.indexer.index.files.values().find(|f| f.id == id) {
                        *pattern = node.relative_path.clone();
                    }
                } else {
                    // Fallback: It might already be a relative glob (e.g. "src/**/*.ts")
                    // Do nothing.
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
    state: tauri::State<'_, SettingsState>
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
    app_handle: tauri::AppHandle
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
    let roots = manager.config.roots
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
    settings_state: tauri::State<'_, SettingsState>
) -> Result<WorkspaceConfigDTO, String> {
    let mut guard = state.manager.write().await;
    let manager = guard.as_mut().ok_or("No active workspace")?;

    let target_path = PathBuf::from(&path);
    manager.save_as(target_path.clone()).map_err(|e| e.to_string())?;

    settings_state.update_recent(path);

    Ok(map_to_dto(manager))
}

#[tauri::command]
async fn add_root_to_workspace(
    root_path: String,
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle
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
    let roots = manager.config.roots
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
    app_handle: tauri::AppHandle
) -> Result<WorkspaceConfigDTO, String> {
    let mut guard = state.manager.write().await;
    let manager = guard.as_mut().ok_or("No active workspace")?;

    let path = PathBuf::from(root_path);
    manager.remove_root(path);

    if manager.backing_file.is_some() {
        manager.save().map_err(|e| e.to_string())?;
    }

    let dto = map_to_dto(manager);
    let roots = manager.config.roots
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
    state: tauri::State<'_, AppState>
) -> Result<Vec<SearchResult>, String> {
    let guard = state.manager.read().await;
    let manager = guard.as_ref().ok_or("Workspace not loaded")?;
    let indexer = &manager.indexer;

    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut results = Vec::new();
    let query_utf32 = Utf32String::from(query.as_str());

    // 1. Search Symbols
    for sym in indexer.index.symbols.values() {
        if
            let Some(score) = matcher.fuzzy_match(
                Utf32String::from(sym.name.as_str()).slice(..),
                query_utf32.slice(..)
            )
        {
            // Find Relative Path
            let file_path = indexer.index.files
                .values()
                .find(|f| f.id == sym.file_id)
                .map(|f| f.relative_path.clone())
                .unwrap_or_default();

            results.push(SearchResult {
                name: sym.name.clone(),
                kind: format!("{:?}", sym.kind),
                path: file_path,
                score,
            });
        }
    }

    // 2. Search Files (Match against Relative Path)
    for file in indexer.index.files.values() {
        let display_name = file.relative_path.clone();

        if
            let Some(score) = matcher.fuzzy_match(
                Utf32String::from(display_name.as_str()).slice(..),
                query_utf32.slice(..)
            )
        {
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
    manager.config.roots.iter().map(|r| {
        let name = r.path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "root".to_string());
        (r.id.clone(), name)
    }).collect()
}

#[tauri::command]
async fn execute_recipe(
    recipe_json: serde_json::Value,
    state: tauri::State<'_, AppState>
) -> Result<String, String> {
    let guard = state.manager.read().await;
    let manager = guard.as_ref().ok_or("Workspace not loaded")?;
    let mut recipe: Recipe = serde_json
        ::from_value(recipe_json)
        .map_err(|e| format!("Invalid recipe format: {}", e))?;

    // Normalize paths (Abs -> Rel)
    recipe = normalize_recipe(manager, recipe);
    let root_map = get_root_map(manager);
    let executor = RecipeExecutor::new(&manager.indexer, root_map);

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
    state: tauri::State<'_, AppState>
) -> Result<String, String> {
    let guard = state.manager.read().await;
    let manager = guard.as_ref().ok_or("Workspace not loaded")?;
    let mut recipe: Recipe = serde_json
        ::from_value(recipe_json)
        .map_err(|e| format!("Invalid recipe format: {}", e))?;

    recipe = normalize_recipe(manager, recipe);
    let root_map = get_root_map(manager);
    let executor = RecipeExecutor::new(&manager.indexer, root_map);

    match executor.get_file_content(file_id, &recipe) {
        Ok(Some(content)) => Ok(content),
        Ok(None) => Err("File not found".into()),
        Err(e) => Err(format!("Failed to read file: {}", e)),
    }
}

#[tauri::command]
async fn generate_xml_bundle(
    recipe_json: serde_json::Value,
    state: tauri::State<'_, AppState>
) -> Result<String, String> {
    let guard = state.manager.read().await;
    let manager = guard.as_ref().ok_or("Workspace not loaded")?;
    let mut recipe: Recipe = serde_json
        ::from_value(recipe_json)
        .map_err(|e| format!("Invalid recipe format: {}", e))?;

    recipe = normalize_recipe(manager, recipe);
    let root_map = get_root_map(manager);
    let executor = RecipeExecutor::new(&manager.indexer, root_map);

    // Full content execution
    let output = executor.execute_full(&recipe).map_err(|e| format!("Execution failed: {}", e))?;

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
    state: tauri::State<'_, AppState>
) -> Result<(), String> {
    let mut guard = state.manager.write().await;
    let manager = guard.as_mut().ok_or("No active workspace")?;
    let mut recipe: Recipe = serde_json
        ::from_value(recipe_json)
        .map_err(|e| format!("Invalid recipe format: {}", e))?;

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
    state: tauri::State<'_, AppState>
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
    state: tauri::State<'_, AppState>
) -> Result<Vec<ResolvedPathDTO>, String> {
    let guard = state.manager.read().await;
    let manager = guard.as_ref().ok_or("Workspace not loaded")?;
    let indexer = &manager.indexer;

    let mut results = Vec::new();

    for path_str in paths {
        let path_buf = PathBuf::from(&path_str);
        // Ensure we are comparing canonical paths if possible
        let lookup_path = if path_buf.exists() {
            std::fs::canonicalize(&path_buf).unwrap_or(path_buf.clone())
        } else {
            path_buf.clone()
        };

        // 1. Check path_map (Absolute -> FileId)
        if let Some(&file_id) = indexer.path_map.get(&lookup_path) {
            // Found in index! Get the relative path.
            if let Some(file_node) = indexer.index.files.values().find(|f| f.id == file_id) {
                results.push(ResolvedPathDTO {
                    original: path_str,
                    relative_path: Some(file_node.relative_path.clone()),
                    root_id: Some(file_node.root_id.clone()),
                    is_indexed: true,
                });
                continue;
            }
        }

        // 2. Handle Directory Drops (Partial Matches)
        // If the user drops "src/utils/", and we have files inside it, 
        // we need to return a glob like "src/utils/**"
        // This is trickier with O(1) lookups. 
        // For V1, let's check if the path is contained within any Root.
        
        let mut found_root = None;
        let mut rel_path_candidate = None;

        for root in &manager.config.roots {
            if lookup_path.starts_with(&root.path) {
                // It is inside this root.
                if let Ok(rel) = lookup_path.strip_prefix(&root.path) {
                    // Convert to unix style for globs
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    
                    rel_path_candidate = Some(if lookup_path.is_dir() {
                        format!("{}/**", rel_str)
                    } else {
                        rel_str
                    });
                    
                    found_root = Some(root.id.clone());
                    break;
                }
            }
        }

        if let Some(root_id) = found_root {
            results.push(ResolvedPathDTO {
                original: path_str,
                relative_path: rel_path_candidate,
                root_id: Some(root_id),
                is_indexed: false, // Not a specific file in the index, but valid for a glob
            });
        } else {
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
    tauri::Builder
        ::default()
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
        .invoke_handler(
            tauri::generate_handler![
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
            ]
        )
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}