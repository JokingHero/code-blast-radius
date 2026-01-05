#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use tokio::sync::RwLock;
use blast_radius_engine::recipes::executor::RecipeExecutor;
use blast_radius_engine::recipes::models::Recipe;
use tauri::{Manager, Emitter};

// File Watching
use notify::{RecommendedWatcher, RecursiveMode, Watcher, Event, EventKind};
use std::sync::Mutex;

// Engine Imports
use blast_radius_engine::workspace::WorkspaceManager;
use nucleo_matcher::{ Matcher, Config, Utf32String };

// Local Modules
mod settings;
use settings::SettingsState;

// --- State Definitions ---

struct AppState {
    manager: RwLock<Option<WorkspaceManager>>,
    // Watcher is stored in a std Mutex because we replace it atomically 
    // and notify's API is synchronous for creation/dropping.
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

#[derive(serde::Serialize, Clone)]
struct WorkspaceConfigDTO {
    name: String,
    roots: Vec<String>,
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

    WorkspaceConfigDTO {
        name: manager.config.name.clone(),
        roots: manager.config.roots
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect(),
        mode,
    }
}

/// Sets up a file watcher for the given roots.
/// Returns a new Watcher which will emit "workspace:dirty" events on changes.
fn setup_watcher(app: &tauri::AppHandle, roots: Vec<PathBuf>) -> Option<RecommendedWatcher> {
    let app_handle = app.clone();
    
    // Define the event handler callback
    let event_handler = move |res: notify::Result<Event>| {
        match res {
            Ok(event) => {
                // 1. Ignore Access events (reads shouldn't mark workspace dirty)
                if let EventKind::Access(_) = event.kind {
                    return;
                }

                // 2. Filter loop-causing paths
                // We must ignore .cblast (where we write indexes) and common noise folders
                let is_relevant = event.paths.iter().any(|p| {
                    let path_str = p.to_string_lossy();
                    !path_str.contains(".cblast") && 
                    !path_str.contains(".git") &&
                    !path_str.contains("node_modules")
                });

                if is_relevant {
                    // Emit event to frontend. Payload is empty.
                    // The frontend just sets isDirty = true.
                    let _ = app_handle.emit("workspace:dirty", ()); 
                }
            },
            Err(e) => eprintln!("Watch error: {:?}", e),
        }
    };

    // Create the watcher
    let mut watcher = notify::recommended_watcher(event_handler).ok()?;

    // Register all roots recursively
    for root in roots {
        if let Err(e) = watcher.watch(&root, RecursiveMode::Recursive) {
            eprintln!("Failed to watch root {:?}: {}", root, e);
        }
    }

    Some(watcher)
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
    app_handle: tauri::AppHandle, 
) -> Result<WorkspaceConfigDTO, String> {
    let target_path = PathBuf::from(&path);

    if !target_path.exists() {
        settings_state.remove_recent(&path);
        return Err("ERR_WORKSPACE_NOT_FOUND".into());
    }

    let manager = if target_path.is_dir() {
        WorkspaceManager::new_in_memory(vec![target_path]).map_err(|e| e.to_string())?
    } else {
        WorkspaceManager::from_file(target_path.clone()).map_err(|e| e.to_string())?
    };

    settings_state.update_recent(path.clone());

    let dto = map_to_dto(&manager);
    
    // Extract roots for watcher before moving manager into lock
    let roots = manager.config.roots.clone();

    // 1. Update Manager State
    let mut guard = state.manager.write().await;
    *guard = Some(manager);

    // 2. Setup Watcher (Replaces any existing one)
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
    let roots = manager.config.roots.clone();

    // Update Watcher to include new root
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
    let roots = manager.config.roots.clone();

    // Update Watcher to remove old root
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

    for sym in indexer.index.symbols.values() {
        if
            let Some(score) = matcher.fuzzy_match(
                Utf32String::from(sym.name.as_str()).slice(..),
                query_utf32.slice(..)
            )
        {
            let file_path = indexer.index.files
                .values()
                .find(|f| f.id == sym.file_id)
                .map(|f| f.path.clone())
                .unwrap_or_default();

            results.push(SearchResult {
                name: sym.name.clone(),
                kind: format!("{:?}", sym.kind),
                path: file_path,
                score,
            });
        }
    }

    results.sort_by(|a, b| b.score.cmp(&a.score));
    results.truncate(10);
    Ok(results)
}

#[tauri::command]
async fn execute_recipe(
    recipe_json: serde_json::Value,
    state: tauri::State<'_, AppState>
) -> Result<String, String> {
    let guard = state.manager.read().await;
    let manager = guard.as_ref().ok_or("Workspace not loaded")?;
    let indexer = &manager.indexer;

    let recipe: Recipe = serde_json
        ::from_value(recipe_json)
        .map_err(|e| format!("Invalid recipe format: {}", e))?;

    let executor = RecipeExecutor::new(indexer);
    let output = executor.execute(&recipe).map_err(|e| format!("Execution failed: {}", e))?;

    serde_json::to_string_pretty(&output).map_err(|e| e.to_string())
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

    let recipe: Recipe = serde_json
        ::from_value(recipe_json)
        .map_err(|e| format!("Invalid recipe format: {}", e))?;

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
        .invoke_handler(
            tauri::generate_handler![
                set_workspace,
                add_root_to_workspace,
                remove_root_from_workspace,
                save_current_workspace,
                search_symbols,
                execute_recipe,
                refresh_workspace,
                get_global_settings,
                clear_recent_history,
                get_saved_recipes,
                save_named_recipe,
                delete_named_recipe
            ]
        )
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}