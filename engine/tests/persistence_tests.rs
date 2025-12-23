mod common;
use common::TestWorkspace;
use rfc_engine::indexer::Indexer;

#[test]
fn test_persistence_lifecycle() {
    // 1. Setup Workspace
    let workspace = TestWorkspace::new();
    let index_file = workspace.path.join(".index");

    workspace.create_file("math.ts", r#"
        function add(a, b) { return a + b; }
    "#);

    // 2. Run 1: Initial Scan & Save
    {
        let mut indexer = Indexer::new();
        indexer.scan(&workspace.path);
        
        // Verify we found the function
        let graph = indexer.export_graph();
        assert!(graph.contains_key("add"), "Initial scan failed to find 'add'");

        // Save to disk
        indexer.save(&index_file).expect("Failed to save index");
        assert!(index_file.exists(), "Index file was not created");
    }

    // 3. Run 2: Load from Disk (Instant Startup Simulation)
    {
        // We load from the file we just saved
        let loaded_indexer = Indexer::load_from_file(&index_file)
            .expect("Failed to load index");
        
        let graph = loaded_indexer.export_graph();
        
        // Verify memory map worked
        assert!(graph.contains_key("add"), "Loaded index missing 'add' function");
    }
}

#[test]
fn test_incremental_updates() {
    let workspace = TestWorkspace::new();
    let index_file = workspace.path.join(".index");

    // Phase 1: Create initial file
    workspace.create_file("logic.py", "def init_system():\n    pass");
    
    {
        let mut indexer = Indexer::new();
        indexer.scan(&workspace.path);
        indexer.save(&index_file).unwrap();
    }

    // Phase 2: Modify file AND add new file
    // - logic.py is modified (hash changes)
    // - utils.py is added (new file)
    workspace.create_file("logic.py", "def init_system_v2():\n    pass");
    workspace.create_file("utils.py", "def helper():\n    pass");

    {
        // Load previous state
        let mut indexer = Indexer::load_from_file(&index_file).unwrap();
        
        // Before scanning, the index should NOT have 'helper' and SHOULD have 'init_system'
        let old_graph = indexer.export_graph();
        assert!(old_graph.contains_key("init_system"));
        assert!(!old_graph.contains_key("helper"));

        // Incremental Scan
        indexer.scan(&workspace.path);

        // Verify updates
        let new_graph = indexer.export_graph();
        
        // 1. New file detected
        assert!(new_graph.contains_key("helper"), "Incremental scan missed new file");
        
        // 2. Modified file detected
        assert!(new_graph.contains_key("init_system_v2"), "Incremental scan missed modified function");
    }
}

#[test]
fn test_corrupted_index_recovery() {
    let workspace = TestWorkspace::new();
    let index_file = workspace.path.join("corrupt.index");

    // Create a dummy file acting as a corrupted index
    workspace.create_file("corrupt.index", "This is not a valid rkyv archive");

    // The loader should detect corruption and return a fresh, empty Indexer
    let indexer = Indexer::load_from_file(&index_file).expect("Should recover from corruption");
    
    assert!(indexer.index.files.is_empty());
    assert!(indexer.index.symbols.is_empty());
}