use blast_radius_engine::models::BoundaryIndex;
use blast_radius_engine::resolution::scanner::FileScanner;
use blast_radius_engine::resolution::persistence::PersistenceManager;
use std::fs;
use tempfile::TempDir;

// Helper to check for symbol existence in the new BoundaryIndex
fn has_func(index: &BoundaryIndex, name: &str) -> bool {
    index.symbol_map.contains_key(name)
}

#[test]
fn test_persistence_lifecycle() {
    // Setup
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let workspace_path = temp_dir.path();
    let index_file = workspace_path.join(".cblast.index");
    let root_id = "root_1";

    let persistence = PersistenceManager::new();
    let scanner = FileScanner::new();

    // Create Content
    fs::write(
        workspace_path.join("math.ts"),
        r#"function add(a, b) { return a + b; }"#
    ).expect("Failed to write math.ts");

    // Run 1: Scan and Save
    {
        let mut index = BoundaryIndex::new();
        scanner.scan(workspace_path, &mut index, root_id);
        persistence.save_index(&index, &index_file).expect("Failed to save index");
    }

    // Run 2: Load and Verify
    {
        let loaded_index = persistence.load_index(&index_file).expect("Failed to load");
        assert!(has_func(&loaded_index, "add"), "Loaded index missing 'add'");
    }
}

#[test]
fn test_incremental_updates() {
    // Setup
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let workspace_path = temp_dir.path();
    let index_file = workspace_path.join(".cblast.index");
    let root_id = "root_1";

    let persistence = PersistenceManager::new();
    let scanner = FileScanner::new();

    // Phase 1: Initial State
    fs::write(
        workspace_path.join("logic.py"), 
        "def init_system():\n    pass"
    ).expect("Failed to write logic.py");

    {
        let mut index = BoundaryIndex::new();
        scanner.scan(workspace_path, &mut index, root_id);
        persistence.save_index(&index, &index_file).expect("Failed to save index");
    }

    // Phase 2: Modify one file, Add another
    fs::write(
        workspace_path.join("logic.py"), 
        "def init_system_v2():\n    pass"
    ).expect("Failed to modify logic.py");
    
    fs::write(
        workspace_path.join("utils.py"), 
        "def helper():\n    pass"
    ).expect("Failed to write utils.py");

    {
        // 1. Load previous state
        let mut index = persistence.load_index(&index_file).expect("Failed to load");
        
        // Sanity Check: Before scanning, it should look like the old state
        assert!(has_func(&index, "init_system"), "Old symbol should be present before rescan");
        assert!(!has_func(&index, "init_system_v2"), "New symbol should not be present before rescan");
        assert!(!has_func(&index, "helper"), "New file symbol should not be present before rescan");

        // 2. Incremental Scan
        scanner.scan(workspace_path, &mut index, root_id);

        // 3. Assertions
        assert!(
            has_func(&index, "helper"), 
            "Incremental scan missed new file 'utils.py'"
        );
        assert!(
            has_func(&index, "init_system_v2"), 
            "Incremental scan missed modified function in 'logic.py'"
        );
        assert!(
            !has_func(&index, "init_system"), 
            "Incremental scan failed to remove old function from 'logic.py'"
        );
    }
}

#[test]
fn test_corrupted_index_recovery() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let workspace_path = temp_dir.path();
    let index_file = workspace_path.join("corrupt.cblast.index");
    
    // Write garbage data
    fs::write(&index_file, "This is not a valid rkyv archive").expect("Failed to write corruption");
    
    let persistence = PersistenceManager::new();
    
    // Attempt load
    let index = persistence.load_index(&index_file).expect("Should recover (return empty) from corruption, not panic");
    
    // Should result in a fresh, empty index
    assert!(index.files.is_empty(), " recovered index should be empty");
    assert!(index.symbol_map.is_empty(), " recovered symbol map should be empty");
}