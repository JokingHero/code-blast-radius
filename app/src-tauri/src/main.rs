#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;
use std::path::PathBuf;
use blast_radius_engine::resolution::{Indexer, pipeline::Pipeline};
use blast_radius_engine::query::traversal::find_related_symbols;
use blast_radius_engine::query::output::generate_context_output;
use blast_radius_engine::workspace::WorkspaceManager; // Use Engine's workspace
use nucleo_matcher::{Matcher, Config, Utf32String};

// Global App State
struct AppState {
    indexer: Mutex<Option<Indexer>>,
    // We store the active config path so "refresh" knows what to reload
    // If None, we are in Single-Folder mode.
    active_workspace_file: Mutex<Option<PathBuf>>,
    // If workspace_file is None, we use this for Single-Folder refresh
    active_folder_path: Mutex<Option<PathBuf>>,
}

#[derive(serde::Serialize)]
struct SearchResult {
    name: String,
    kind: String,
    path: String,
    score: u16,
}

#[tauri::command]
async fn set_workspace(path: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let target_path = PathBuf::from(&path);
    if !target_path.exists() {
        return Err("Path does not exist".into());
    }

    let mut indexer;
    let workspace_file_entry;
    let folder_path_entry;

    if target_path.is_file() && path.ends_with(".cblast") {
        // === MULTI-ROOT MODE (Managed by WorkspaceManager) ===
        // Uses: <project>.cblast.index
        let mut manager = WorkspaceManager::new(target_path.clone())
            .map_err(|e| e.to_string())?;
        
        // Ensure everything is up to date (scans all roots, re-resolves)
        manager.sync();
        manager.save().map_err(|e| e.to_string())?;

        indexer = manager.indexer;
        
        workspace_file_entry = Some(target_path);
        folder_path_entry = None;
    } else if target_path.is_dir() {
        // === SINGLE FOLDER MODE (Ad-Hoc View) ===
        // We isolate this view by using a separate index file.
        // This prevents overwriting a multi-root 'index.bin' that might exist
        // if this folder is also part of a larger CLI/Workspace setup.
        indexer = Indexer::new();
        
        let cblast_dir = target_path.join(".cblast");
        let _ = std::fs::create_dir_all(&cblast_dir);
        
        // CHANGED: Use a distinct filename for ad-hoc GUI sessions
        let index_path = cblast_dir.join("index.local.bin");

        // Try load existing local index
        if index_path.exists() {
            if let Ok(loaded) = Indexer::load_from_file(&index_path) {
                indexer = loaded;
            }
        }

        // Run Scan/Resolve (Scopes strictly to this folder)
        let mut pipeline = Pipeline::new();
        pipeline.run(&mut indexer, &target_path);
        
        // Save to local index only
        let _ = indexer.save(&index_path);

        workspace_file_entry = None;
        folder_path_entry = Some(target_path);
    } else {
        return Err("Invalid path. Must be a folder or .cblast file".into());
    }

    // Update State
    *state.indexer.lock().unwrap() = Some(indexer);
    *state.active_workspace_file.lock().unwrap() = workspace_file_entry;
    *state.active_folder_path.lock().unwrap() = folder_path_entry;

    Ok(())
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
        // CHANGED: Match the filename used in set_workspace
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
async fn resolve_recipe(
    target_symbol: String, 
    no_tests: bool, 
    state: tauri::State<'_, AppState>
) -> Result<String, String> {
    let state_guard = state.indexer.lock().unwrap();
    let indexer = state_guard.as_ref().ok_or("Workspace not loaded")?;

    let symbol_ids = find_related_symbols(
        &indexer.index,
        &indexer.lookup,
        &indexer.reverse_graph,
        &target_symbol
    ).ok_or("Symbol not found")?;

    // Filter Logic (Simple version for now)
    let final_ids: Vec<u32> = if no_tests {
        symbol_ids.into_iter().filter(|&id| {
             !indexer.index.symbols.get(&id).map(|s| s.is_test).unwrap_or(false)
        }).collect()
    } else {
        symbol_ids
    };

    let output = generate_context_output(&indexer.index, &final_ids);
    
    serde_json::to_string_pretty(&output).map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
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
            search_symbols,
            resolve_recipe,
            refresh_workspace
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}