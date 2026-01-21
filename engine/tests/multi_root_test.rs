use blast_radius_engine::workspace::WorkspaceManager;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_workspace_full_lifecycle() {
    // =========================================================================
    // 0. SETUP ENV
    // =========================================================================
    let dir = tempdir().unwrap();
    let env_path = dir.path().to_path_buf();

    // Create physical folders
    let root_core = env_path.join("core_lib");
    let root_app = env_path.join("app_service");

    fs::create_dir(&root_core).expect("Failed to create core dir");
    fs::create_dir(&root_app).expect("Failed to create app dir");

    // =========================================================================
    // 1. INITIALIZATION (One Root)
    // =========================================================================
    println!("--- STEP 1: Init Workspace with Core Lib ---");

    // core_lib/utils.ts -> function helper() {}
    fs::write(
        root_core.join("utils.ts"),
        "export function helper() { return 1; }",
    )
    .unwrap();

    // Use new_in_memory() first
    let mut manager = WorkspaceManager::new_in_memory(vec![]).expect("Failed to init manager");

    // Add root (This triggers add_root -> scan)
    manager.add_root(root_core.clone());

    // Assert: Helper exists using new index API
    assert!(
        manager.index.symbol_map.contains_key("helper"),
        "Helper symbol should be indexed"
    );
    assert_eq!(manager.config.roots.len(), 1);

    // =========================================================================
    // 2. EXPANSION (Add Second Root)
    // =========================================================================
    println!("--- STEP 2: Add App Service Root ---");

    // app_service/main.ts
    fs::write(
        root_app.join("main.ts"),
        r#"
    import { helper } from "./anywhere"; 
    function main() { helper(); }
"#,
    )
    .unwrap();

    manager.add_root(root_app.clone());

    // Assert: Both symbols exist
    assert!(
        manager.index.symbol_map.contains_key("main"),
        "Main symbol should be indexed"
    );
    assert!(
        manager.index.symbol_map.contains_key("helper"),
        "Helper symbol should still exist"
    );

    // =========================================================================
    // 3. FILE MODIFICATION (Refactor)
    // =========================================================================
    println!("--- STEP 3: Modify File in Core ---");

    // Rename helper -> super_helper
    fs::write(
        root_core.join("utils.ts"),
        "export function super_helper() { return 9000; }",
    )
    .unwrap();

    // Trigger Sync (Simulate "Refresh")
    manager.sync();

    // Assert: Old symbol gone, new symbol present
    assert!(
        !manager.index.symbol_map.contains_key("helper"),
        "Old symbol 'helper' should be gone"
    );
    assert!(
        manager.index.symbol_map.contains_key("super_helper"),
        "New symbol 'super_helper' should exist"
    );

    // =========================================================================
    // 4. FILE ADDITION
    // =========================================================================
    println!("--- STEP 4: Add New File to App ---");

    fs::write(root_app.join("logger.ts"), "export function log(msg) {}").unwrap();

    manager.sync();

    assert!(
        manager.index.symbol_map.contains_key("log"),
        "Newly added file should be indexed"
    );

    // =========================================================================
    // 5. FILE DELETION
    // =========================================================================
    println!("--- STEP 5: Delete File from App ---");

    fs::remove_file(root_app.join("logger.ts")).unwrap();

    manager.sync();

    assert!(
        !manager.index.symbol_map.contains_key("log"),
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
        !manager.index.symbol_map.contains_key("main"),
        "Symbols from removed root should be purged"
    );

    // Assert: Core symbols remain
    assert!(
        manager.index.symbol_map.contains_key("super_helper"),
        "Symbols from remaining root should persist"
    );

    // Assert: Config updated
    assert_eq!(manager.config.roots.len(), 1);
    // Note: Canonicalization might make paths vary slightly in string form, but here we just check count
}
