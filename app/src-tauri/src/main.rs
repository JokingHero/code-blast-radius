#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;
use std::path::PathBuf;
use blast_radius_engine::resolution::{Indexer, pipeline::Pipeline};
use blast_radius_engine::query::traversal::find_related_symbols;
use blast_radius_engine::query::output::generate_context_output;
use nucleo_matcher::{Matcher, Config, Utf32String};

// Global App State
struct AppState {
    indexer: Mutex<Option<Indexer>>,
    root_path: Mutex<Option<PathBuf>>,
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
    let path_buf = PathBuf::from(path);
    if !path_buf.exists() {
        return Err("Path does not exist".into());
    }

    let index_path = path_buf.join(".index");
    
    // Load or Create Indexer
    let mut indexer = Indexer::load_from_file(&index_path)
        .unwrap_or_else(|_| Indexer::new());

    // Run Pipeline (Scan/Resolve)
    let mut pipeline = Pipeline::new();
    pipeline.run(&mut indexer, &path_buf);
    
    // Save
    let _ = indexer.save(&index_path);

    // Update State
    *state.root_path.lock().unwrap() = Some(path_buf);
    *state.indexer.lock().unwrap() = Some(indexer);

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
            root_path: Mutex::new(None) 
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            set_workspace,
            search_symbols,
            resolve_recipe
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}