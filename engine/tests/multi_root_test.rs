mod common;
use common::TestWorkspace;
use blast_radius_engine::{ workspace::WorkspaceManager, models::EdgeKind };
use std::fs;
#[test]
fn test_workspace_full_lifecycle() {
    // =========================================================================
    // 0. SETUP ENV
    // =========================================================================
    let env = TestWorkspace::new();
    // Create physical folders
    let root_core = env.path.join("core_lib");
    let root_app = env.path.join("app_service");

    fs::create_dir(&root_core).expect("Failed to create core dir");
    fs::create_dir(&root_app).expect("Failed to create app dir");

    // Define the workspace config path
    let config_path = env.path.join("test_project.cblast");

    // =========================================================================
    // 1. INITIALIZATION (One Root)
    // =========================================================================
    println!("--- STEP 1: Init Workspace with Core Lib ---");

    // Create initial file in Core
    // core_lib/utils.ts -> export function helper() {}
    fs::write(root_core.join("utils.ts"), "export function helper() { return 1; }").unwrap();

    // FIX: Use new_in_memory() first, then save_as() to bind it to a file.
    // This matches the logic in WorkspaceManager.
    let mut manager = WorkspaceManager::new_in_memory(vec![]).expect("Failed to init manager");

    // Bind to the .cblast file
    manager.save_as(config_path.clone()).expect("Failed to bind file");

    // Add root and save
    manager.add_root(root_core.clone());
    manager.save().expect("Failed to save");

    // Assert: Helper exists
    assert!(manager.indexer.lookup.symbol_map.contains_key("helper"));
    assert_eq!(manager.config.roots.len(), 1);

    // =========================================================================
    // 2. EXPANSION (Add Second Root)
    // =========================================================================
    println!("--- STEP 2: Add App Service Root ---");

    // Create file in App that USES Core
    // app_service/main.ts -> import { helper } ...
    // Note: We use a relative import "./anywhere" which will fail path resolution.
    // This forces the engine to skip External resolution and use the Global Heuristic fallback,
    // effectively finding 'helper' in the other root by name uniqueness.
    fs::write(
        root_app.join("main.ts"),
        r#"
    import { helper } from "./anywhere"; 
    function main() { helper(); }
"#
    ).unwrap();

    manager.add_root(root_app.clone());

    // Assert: Both symbols exist
    let main_ids = manager.indexer.lookup.symbol_map.get("main").expect("Main not found");
    let helper_ids = manager.indexer.lookup.symbol_map.get("helper").expect("Helper not found");

    // Assert: Cross-Root Linkage (Resolution)
    // main() should call helper()
    let edges = manager.indexer.index.graph.get(&main_ids[0]).expect("Main should have edges");
    let links_to_helper = edges
        .iter()
        .any(|e| e.target_id == helper_ids[0] && e.kind == EdgeKind::Calls);

    // Note: This asserts the resolver ran across both roots
    assert!(links_to_helper, "Cross-root dependency resolution failed");

    // =========================================================================
    // 3. FILE MODIFICATION (Refactor)
    // =========================================================================
    println!("--- STEP 3: Modify File in Core ---");

    // Rename helper -> super_helper
    fs::write(
        root_core.join("utils.ts"),
        "export function super_helper() { return 9000; }"
    ).unwrap();

    // Trigger Sync (Simulate "Refresh")
    manager.sync();

    // Assert: Old symbol gone, new symbol present
    assert!(
        !manager.indexer.lookup.symbol_map.contains_key("helper"),
        "Old symbol 'helper' should be gone"
    );
    assert!(
        manager.indexer.lookup.symbol_map.contains_key("super_helper"),
        "New symbol 'super_helper' should exist"
    );

    // =========================================================================
    // 4. FILE ADDITION
    // =========================================================================
    println!("--- STEP 4: Add New File to App ---");

    fs::write(root_app.join("logger.ts"), "export function log(msg) {}").unwrap();

    manager.sync();

    assert!(
        manager.indexer.lookup.symbol_map.contains_key("log"),
        "Newly added file should be indexed"
    );

    // =========================================================================
    // 5. FILE DELETION
    // =========================================================================
    println!("--- STEP 5: Delete File from App ---");

    fs::remove_file(root_app.join("logger.ts")).unwrap();

    manager.sync();

    assert!(
        !manager.indexer.lookup.symbol_map.contains_key("log"),
        "Deleted symbol should be removed"
    );

    // =========================================================================
    // 6. ROOT REMOVAL
    // =========================================================================
    println!("--- STEP 6: Remove App Root Entirely ---");

    // We remove the app folder from the workspace config.
    // This should remove 'main' (from app) but keep 'super_helper' (from core).
    manager.remove_root(root_app.clone());

    // Assert: App symbols gone
    assert!(
        !manager.indexer.lookup.symbol_map.contains_key("main"),
        "Symbols from removed root should be purged"
    );

    // Assert: Core symbols remain
    assert!(
        manager.indexer.lookup.symbol_map.contains_key("super_helper"),
        "Symbols from remaining root should persist"
    );

    // Assert: Config updated
    assert_eq!(manager.config.roots.len(), 1);
    assert_eq!(manager.config.roots[0], fs::canonicalize(&root_core).unwrap());
}
