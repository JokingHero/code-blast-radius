use blast_radius_engine::models::BoundaryIndex;
use blast_radius_engine::resolution::persistence::PersistenceManager;
use blast_radius_engine::resolution::scanner::FileScanner;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_ghost_data_removal() {
    // Setup environment
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let workspace_path = temp_dir.path();
    let index_file = workspace_path.join(".cblast.index");
    let root_id = "root_1";

    // Components
    let persistence = PersistenceManager::new();
    let scanner = FileScanner::new();

    // 1. Initial State
    // Create a file with an "old" function
    let file_path = workspace_path.join("temp.js");
    fs::write(&file_path, "function old_function() {}").expect("Failed to write file");

    {
        let mut index = BoundaryIndex::new();

        // Scan
        scanner.scan(workspace_path, &mut index, root_id);

        // Persist to verify serialization doesn't mess up state later
        persistence
            .save_index(&index, &index_file)
            .expect("Failed to save index");

        // Verify Initial State: "old_function" should exist
        assert!(
            index.symbol_map.contains_key("old_function"),
            "Setup failed: old_function not found in symbol_map"
        );

        // Verify mapping to a file
        let ids = index.symbol_map.get("old_function").unwrap();
        assert!(!ids.is_empty(), "old_function has no file IDs");
    }

    // 2. Modification (Ghost Data Check #1)
    // Rename the function in the file. "old_function" should vanish.
    fs::write(&file_path, "function new_function() {}").expect("Failed to modify file");

    {
        // Simulate a fresh session by loading from disk
        let mut index = persistence
            .load_index(&index_file)
            .expect("Failed to load index");

        // Re-Scan (Incremental update)
        scanner.scan(workspace_path, &mut index, root_id);

        // Assert New State
        assert!(
            index.symbol_map.contains_key("new_function"),
            "new_function not found after modification"
        );

        // Assert Ghost Data Removal
        assert!(
            !index.symbol_map.contains_key("old_function"),
            "Ghost Data: old_function still exists in symbol_map after rename!"
        );

        // Persist state for next step
        persistence
            .save_index(&index, &index_file)
            .expect("Failed to save index");
    }

    // 3. Deletion (Ghost Data Check #2)
    // Delete the file entirely. All symbols from it should vanish.
    fs::remove_file(&file_path).expect("Failed to delete file");

    {
        // Load from disk
        let mut index = persistence
            .load_index(&index_file)
            .expect("Failed to load index");

        // Re-Scan
        scanner.scan(workspace_path, &mut index, root_id);

        // Assertions
        assert!(
            !index.symbol_map.contains_key("new_function"),
            "Ghost Data: new_function still exists after file deletion!"
        );

        // Ensure the file entry itself is removed from the files map
        // Note: index.files keys are FileIds, values are FileBoundary
        let file_still_exists = index.files.values().any(|f| f.path == "temp.js");
        assert!(
            !file_still_exists,
            "Ghost Data: FileBoundary still exists in index.files after deletion"
        );

        // Since we only had one file, the index should now be effectively empty
        // regarding user content (files map might contain nothing, but we check specifically for our root)
        let root_file_count = index
            .files
            .values()
            .filter(|f| f.root_id == root_id)
            .count();
        assert_eq!(root_file_count, 0, "Index should be empty for this root");
    }
}
