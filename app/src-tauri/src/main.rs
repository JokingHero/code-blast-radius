#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;
use std::path::PathBuf;
use blast_radius_engine::recipes::executor::RecipeExecutor;
use blast_radius_engine::recipes::models::Recipe;
use tauri::Manager;

// Engine Imports
use blast_radius_engine::workspace::WorkspaceManager;
use nucleo_matcher::{Matcher, Config, Utf32String};

// Local Modules
mod settings;
use settings::SettingsState;

// --- State Definitions ---

struct AppState {
    // The Single Source of Truth
    // If None, no workspace is loaded.
    // If Some, the Manager handles whether it's file-backed or in-memory.
    manager: Mutex<Option<WorkspaceManager>>,
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
    // 'project' = Saved .cblast file
    // 'ad-hoc' = Single folder, no file
    // 'unsaved-workspace' = Multiple roots, no file yet
    mode: String, 
}

#[derive(serde::Serialize)]
struct AppSettingsDTO {
    recent: Vec<String>,
    last_opened: Option<String>
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
        roots: manager.config.roots.iter().map(|p| p.to_string_lossy().to_string()).collect(),
        mode,
    }
}

// --- Commands ---

#[tauri::command]
async fn get_global_settings(state: tauri::State<'_, SettingsState>) -> Result<AppSettingsDTO, String> {
    let settings = state.settings.lock().unwrap();
    Ok(AppSettingsDTO {
        recent: settings.recent_workspaces.clone(),
        last_opened: settings.last_opened_path.clone()
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
    settings_state: tauri::State<'_, SettingsState>
) -> Result<WorkspaceConfigDTO, String> {
    let target_path = PathBuf::from(&path);
    if !target_path.exists() {
        return Err("Path does not exist".into());
    }

    // Determine Mode based on Path Type using the new Engine API
    let manager = if target_path.is_dir() {
        // Mode: Ad-Hoc / In-Memory
        WorkspaceManager::new_in_memory(vec![target_path])
            .map_err(|e| e.to_string())?
    } else {
        // Mode: Project File (.cblast)
        WorkspaceManager::from_file(target_path.clone())
            .map_err(|e| e.to_string())?
    };

    // Update Recents
    settings_state.update_recent(path.clone());

    let dto = map_to_dto(&manager);
    
    // Update State
    *state.manager.lock().unwrap() = Some(manager);
    
    Ok(dto)
}

#[tauri::command]
async fn save_current_workspace(
    path: String, 
    state: tauri::State<'_, AppState>, 
    settings_state: tauri::State<'_, SettingsState>
) -> Result<WorkspaceConfigDTO, String> {
    let mut guard = state.manager.lock().unwrap();
    let manager = guard.as_mut().ok_or("No active workspace")?;
    
    let target_path = PathBuf::from(&path);
    
    // Engine handles promotion from Memory -> File, preserving Recipes
    manager.save_as(target_path.clone()).map_err(|e| e.to_string())?;

    settings_state.update_recent(path);

    Ok(map_to_dto(manager))
}

#[tauri::command]
async fn add_root_to_workspace(root_path: String, state: tauri::State<'_, AppState>) -> Result<WorkspaceConfigDTO, String> {
    let mut guard = state.manager.lock().unwrap();
    let manager = guard.as_mut().ok_or("No active workspace")?;

    let path = PathBuf::from(root_path);
    if !path.exists() {
        return Err("Path does not exist".into());
    }

    manager.add_root(path); // Auto-syncs and re-resolves inside engine

    // If it's already a saved project, auto-save the config change to disk
    if manager.backing_file.is_some() {
        manager.save().map_err(|e| e.to_string())?;
    }

    Ok(map_to_dto(manager))
}

#[tauri::command]
async fn remove_root_from_workspace(root_path: String, state: tauri::State<'_, AppState>) -> Result<WorkspaceConfigDTO, String> {
    let mut guard = state.manager.lock().unwrap();
    let manager = guard.as_mut().ok_or("No active workspace")?;

    let path = PathBuf::from(root_path);
    manager.remove_root(path);

    if manager.backing_file.is_some() {
        manager.save().map_err(|e| e.to_string())?;
    }

    Ok(map_to_dto(manager))
}

#[tauri::command]
async fn refresh_workspace(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.manager.lock().unwrap();
    let manager = guard.as_mut().ok_or("No active workspace")?;
    
    manager.sync();

    // If backed by file, save the updated index (cache the fresh scan)
    if manager.backing_file.is_some() {
         manager.save().map_err(|e| e.to_string())?;
    }
    
    Ok(())
}

#[tauri::command]
async fn search_symbols(query: String, state: tauri::State<'_, AppState>) -> Result<Vec<SearchResult>, String> {
    let guard = state.manager.lock().unwrap();
    let manager = guard.as_ref().ok_or("Workspace not loaded")?;
    let indexer = &manager.indexer;
    
    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut results = Vec::new();
    let query_utf32 = Utf32String::from(query.as_str());

    // Search Symbols
    for sym in indexer.index.symbols.values() {
        if let Some(score) = matcher.fuzzy_match(Utf32String::from(sym.name.as_str()).slice(..), query_utf32.slice(..)) {
            let file_path = indexer.index.files.values()
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
    let guard = state.manager.lock().unwrap();
    let manager = guard.as_ref().ok_or("Workspace not loaded")?;
    let indexer = &manager.indexer;

    // 1. Deserialize
    let recipe: Recipe = serde_json::from_value(recipe_json)
        .map_err(|e| format!("Invalid recipe format: {}", e))?;

    // 2. Execute
    let executor = RecipeExecutor::new(indexer);
    let output = executor.execute(&recipe)
        .map_err(|e| format!("Execution failed: {}", e))?;

    // 3. Output
    serde_json::to_string_pretty(&output).map_err(|e| e.to_string())
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
            manager: Mutex::new(None)
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            set_workspace,
            add_root_to_workspace,
            remove_root_from_workspace,
            save_current_workspace,
            search_symbols,
            execute_recipe,
            refresh_workspace,
            get_global_settings,
            clear_recent_history
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}