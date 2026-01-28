#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::path::PathBuf;
use blast_radius_engine::path_service::{PathService, ResolvedPathDTO};
use blast_radius_engine::query::search_service::{SearchService, SearchResult}; 
use blast_radius_engine::recipe_service::RecipeService;

use tauri::{Emitter, Manager};
use tokio::sync::RwLock;
// File Watching
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::Mutex;

// Engine Imports
use blast_radius_engine::recipes::models::{Recipe};
use blast_radius_engine::workspace::WorkspaceManager;

// Local Modules
mod settings;
use settings::SettingsState;

// --- State Definitions ---

struct AppState {
    manager: RwLock<Option<WorkspaceManager>>,
    watcher: Mutex<Option<RecommendedWatcher>>,
}

// --- DTOs ---

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
    let results = SearchService::search(&manager.index, &query, 20);
    Ok(results)
}

#[tauri::command]
async fn execute_recipe(
    recipe_json: serde_json::Value,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let guard = state.manager.read().await;
    let manager = guard.as_ref().ok_or("Workspace not loaded")?;
    let recipe: Recipe = serde_json::from_value(recipe_json).map_err(|e| e.to_string())?;

    // We want metadata only for the UI list
    let result = RecipeService::execute(manager, recipe, false).map_err(|e| e.to_string())?;
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_file_content(
    file_id: u32,
    recipe_json: serde_json::Value,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let guard = state.manager.read().await;
    let manager = guard.as_ref().ok_or("Workspace not loaded")?;
    let recipe: Recipe = serde_json::from_value(recipe_json).map_err(|e| e.to_string())?;

    match RecipeService::get_file_preview(manager, recipe, file_id) {
        Ok(Some(content)) => Ok(content),
        Ok(None) => Err("File not found".into()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
async fn generate_xml_bundle(
    recipe_json: serde_json::Value,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let guard = state.manager.read().await;
    let manager = guard.as_ref().ok_or("Workspace not loaded")?;
    let recipe: Recipe = serde_json::from_value(recipe_json).map_err(|e| e.to_string())?;

    let result = RecipeService::execute(manager, recipe, true).map_err(|e| e.to_string())?;
    result.to_xml().map_err(|e| e.to_string())
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

    recipe = RecipeService::normalize_recipe(manager, recipe);

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
    let results = PathService::resolve(manager, paths);

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
