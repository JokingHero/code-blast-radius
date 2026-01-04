#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;
use std::path::PathBuf;
use blast_radius_engine::recipes::executor::RecipeExecutor;
use blast_radius_engine::recipes::models::Recipe;
use tauri::Manager; // Required for app.manage()

// Engine Imports
use blast_radius_engine::resolution::{Indexer, pipeline::Pipeline};
use blast_radius_engine::workspace::WorkspaceManager;
use nucleo_matcher::{Matcher, Config, Utf32String};

// Local Modules
mod settings;
use settings::SettingsState;

// --- State Definitions ---

struct AppState {
    indexer: Mutex<Option<Indexer>>,
    // We store the active config path so "refresh" knows what to reload
    // If None, we are in Single-Folder (Ad-Hoc) mode or Unsaved Workspace mode.
    active_workspace_file: Mutex<Option<PathBuf>>,
    // If workspace_file is None, we use this for Single-Folder refresh
    active_folder_path: Mutex<Option<PathBuf>>,
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
    // 'project' = .cblast file
    // 'ad-hoc' = single folder open
    // 'unsaved-workspace' = multiple roots, no file yet
    mode: String, 
}

#[derive(serde::Serialize)]
struct AppSettingsDTO {
    recent: Vec<String>,
    last_opened: Option<String>
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

    // 1. Update Persistence (Phase 2)
    settings_state.update_recent(path.clone());

    let mut indexer;
    let config_dto;

    if target_path.is_file() && path.ends_with(".cblast") {
        // === MULTI-ROOT PROJECT MODE ===
        let mut manager = WorkspaceManager::new(target_path.clone())
            .map_err(|e| e.to_string())?;
        
        // Refresh analysis on load
        manager.sync();
        manager.save().map_err(|e| e.to_string())?;

        config_dto = WorkspaceConfigDTO {
            name: manager.config.name.clone(),
            roots: manager.config.roots.iter().map(|p| p.to_string_lossy().to_string()).collect(),
            mode: "project".to_string(),
        };

        indexer = manager.indexer;
        
        *state.active_workspace_file.lock().unwrap() = Some(target_path);
        *state.active_folder_path.lock().unwrap() = None;

    } else if target_path.is_dir() {
        // === AD-HOC SINGLE FOLDER MODE ===
        indexer = Indexer::new();
        
        // Ensure local index cache directory exists
        let cblast_dir = target_path.join(".cblast");
        let _ = std::fs::create_dir_all(&cblast_dir);
        let index_path = cblast_dir.join("index.local.bin");

        // Try load existing cache
        if index_path.exists() {
            if let Ok(loaded) = Indexer::load_from_file(&index_path) {
                indexer = loaded;
            }
        }

        // Run Scan
        let mut pipeline = Pipeline::new();
        pipeline.run(&mut indexer, &target_path);
        let _ = indexer.save(&index_path);

        config_dto = WorkspaceConfigDTO {
            name: target_path.file_name().unwrap_or_default().to_string_lossy().to_string(),
            roots: vec![target_path.to_string_lossy().to_string()],
            mode: "ad-hoc".to_string(),
        };

        *state.active_workspace_file.lock().unwrap() = None;
        *state.active_folder_path.lock().unwrap() = Some(target_path);
    } else {
        return Err("Invalid path. Must be a directory or .cblast file".into());
    }

    *state.indexer.lock().unwrap() = Some(indexer);
    Ok(config_dto)
}

#[tauri::command]
async fn save_current_workspace(path: String, state: tauri::State<'_, AppState>, settings_state: tauri::State<'_, SettingsState>) -> Result<WorkspaceConfigDTO, String> {
    let mut indexer_guard = state.indexer.lock().unwrap();
    let indexer = indexer_guard.as_mut().ok_or("No active workspace")?;
    
    let target_path = PathBuf::from(&path);
    
    // 1. Create a Manager to handle the save logic
    // We construct it manually using the current in-memory Indexer state
    let config = blast_radius_engine::workspace::WorkspaceConfig {
        name: target_path.file_stem().unwrap_or_default().to_string_lossy().to_string(),
        roots: indexer.index.roots.iter().map(PathBuf::from).collect(),
        recipes: std::collections::HashMap::new(),
    };

    // 2. Serialize Config to .cblast
    let config_json = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(&target_path, config_json).map_err(|e| e.to_string())?;

    // 3. Serialize Index to .cblast.index
    let index_path = target_path.with_extension("cblast.index");
    indexer.save(&index_path).map_err(|e| e.to_string())?;

    // 4. Update App State
    *state.active_workspace_file.lock().unwrap() = Some(target_path.clone());
    *state.active_folder_path.lock().unwrap() = None; // No longer just a folder

    // 5. Update Recents
    settings_state.update_recent(path.clone());

    Ok(WorkspaceConfigDTO {
        name: config.name,
        roots: config.roots.iter().map(|p| p.to_string_lossy().to_string()).collect(),
        mode: "project".to_string(),
    })
}

#[tauri::command]
async fn add_root_to_workspace(root_path: String, state: tauri::State<'_, AppState>) -> Result<WorkspaceConfigDTO, String> {
    let workspace_file_opt = state.active_workspace_file.lock().unwrap().clone();
    let new_root = PathBuf::from(&root_path);

    if !new_root.exists() {
        return Err("Path does not exist".into());
    }

    // CASE A: Existing Project (.cblast)
    if let Some(ws_path) = workspace_file_opt {
        let mut manager = WorkspaceManager::new(ws_path).map_err(|e| e.to_string())?;
        manager.add_root(new_root);
        manager.save().map_err(|e| e.to_string())?;

        *state.indexer.lock().unwrap() = Some(manager.indexer);

        return Ok(WorkspaceConfigDTO {
            name: manager.config.name,
            roots: manager.config.roots.iter().map(|p| p.to_string_lossy().to_string()).collect(),
            mode: "project".to_string(),
        });
    }

    // CASE B: Ad-Hoc / Unsaved Workspace
    // We modify the in-memory Indexer directly
    let mut indexer_guard = state.indexer.lock().unwrap();
    if let Some(indexer) = indexer_guard.as_mut() {
        let mut pipeline = Pipeline::new();
        
        // 1. Scan the new root
        // This updates index.roots inside the scanner logic
        pipeline.scan(indexer, &new_root);
        
        // 2. Re-hydrate and Resolve relationships across ALL roots
        let mut staging = pipeline.hydrate_staging(&indexer.index);
        pipeline.resolve(indexer, &mut staging);

        return Ok(WorkspaceConfigDTO {
            name: "Unsaved Workspace".to_string(),
            roots: indexer.index.roots.clone(),
            mode: "unsaved-workspace".to_string(),
        });
    }

    Err("No active session to add to.".into())
}

#[tauri::command]
async fn remove_root_from_workspace(root_path: String, state: tauri::State<'_, AppState>) -> Result<WorkspaceConfigDTO, String> {
    let workspace_file_opt = state.active_workspace_file.lock().unwrap().clone();
    let target_root = PathBuf::from(&root_path);

    // CASE A: Existing Project (.cblast) -> Use Manager to save changes
    if let Some(ws_path) = workspace_file_opt {
        let mut manager = WorkspaceManager::new(ws_path).map_err(|e| e.to_string())?;
        manager.remove_root(target_root);
        manager.save().map_err(|e| e.to_string())?;

        *state.indexer.lock().unwrap() = Some(manager.indexer);

        return Ok(WorkspaceConfigDTO {
            name: manager.config.name,
            roots: manager.config.roots.iter().map(|p| p.to_string_lossy().to_string()).collect(),
            mode: "project".to_string(),
        });
    }

    // CASE B: Ad-Hoc / Unsaved -> Modify In-Memory Indexer
    let mut indexer_guard = state.indexer.lock().unwrap();
    if let Some(indexer) = indexer_guard.as_mut() {
        // Use the Indexer's remove_root capability directly
        indexer.remove_root(&target_root);
        
        // Re-resolve remaining graph
        let mut pipeline = Pipeline::new();
        let mut staging = pipeline.hydrate_staging(&indexer.index);
        pipeline.resolve(indexer, &mut staging);

        let mode = if indexer.index.roots.len() > 1 { 
            "unsaved-workspace" 
        } else { 
            "ad-hoc" 
        };

        return Ok(WorkspaceConfigDTO {
            name: "Workspace".to_string(),
            roots: indexer.index.roots.clone(),
            mode: mode.to_string(),
        });
    }

    Err("No active session.".into())
}

#[tauri::command]
async fn refresh_workspace(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let workspace_opt = state.active_workspace_file.lock().unwrap().clone();
    let folder_opt = state.active_folder_path.lock().unwrap().clone();

    if let Some(ws_path) = workspace_opt {
        // Refresh Multi-Root
        let mut manager = WorkspaceManager::new(ws_path)
            .map_err(|e| e.to_string())?;
        manager.sync();
        manager.save().map_err(|e| e.to_string())?;
        *state.indexer.lock().unwrap() = Some(manager.indexer);
    } else if let Some(folder_path) = folder_opt {
        // Refresh Single Folder
        let index_path = folder_path.join(".cblast").join("index.local.bin");
        
        let mut indexer = Indexer::load_from_file(&index_path).unwrap_or(Indexer::new());
        
        let mut pipeline = Pipeline::new();
        pipeline.run(&mut indexer, &folder_path);
        let _ = indexer.save(&index_path);

        *state.indexer.lock().unwrap() = Some(indexer);
    } else {
        return Err("No workspace loaded".into());
    }

    Ok(())
}

#[tauri::command]
async fn search_symbols(query: String, state: tauri::State<'_, AppState>) -> Result<Vec<SearchResult>, String> {
    let state_guard = state.indexer.lock().unwrap();
    let indexer = state_guard.as_ref().ok_or("Workspace not loaded")?;
    
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
    let state_guard = state.indexer.lock().unwrap();
    let indexer = state_guard.as_ref().ok_or("Workspace not loaded")?;

    // 1. Deserialize the JSON from Frontend into the Engine's Recipe struct
    let recipe: Recipe = serde_json::from_value(recipe_json)
        .map_err(|e| format!("Invalid recipe format: {}", e))?;

    // 2. Run the Executor
    let executor = RecipeExecutor::new(indexer);
    let output = executor.execute(&recipe)
        .map_err(|e| format!("Execution failed: {}", e))?;

    // 3. Return Context Output
    serde_json::to_string_pretty(&output).map_err(|e| e.to_string())
}

// --- Entry Point ---

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // Initialize Settings with AppHandle (requires 'use tauri::Manager')
            // This reads/creates AppData/code.blast.radius/settings.json
            let settings_state = SettingsState::new(app.handle());
            app.manage(settings_state);
            Ok(())
        })
        .manage(AppState { 
            indexer: Mutex::new(None),
            active_workspace_file: Mutex::new(None),
            active_folder_path: Mutex::new(None)
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