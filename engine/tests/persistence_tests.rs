mod common;
use common::TestWorkspace;
use rfc_engine::{models::StagingArea, resolution::Indexer};

fn has_func(index: &rfc_engine::models::WorkspaceIndex, name: &str) -> bool {
    index.symbols.values().any(|s| s.name == name)
}

#[test]
fn test_persistence_lifecycle() {
    let workspace = TestWorkspace::new();
    let index_file = workspace.path.join(".index");

    workspace.create_file("math.ts", r#"
        function add(a, b) { return a + b; }
    "#);

    // Run 1
    {
        let mut indexer = Indexer::new();
        let mut staging = rfc_engine::models::StagingArea::default();
        indexer.scan(&workspace.path, &mut staging);
        indexer.resolve_references(&mut staging);
        indexer.save(&index_file).expect("Failed to save index");
    }

    // Run 2
    {
        let loaded_indexer = Indexer::load_from_file(&index_file).expect("Failed to load");
        assert!(has_func(&loaded_indexer.index, "add"), "Loaded index missing 'add'");
    }
}

#[test]
fn test_incremental_updates() {
    let workspace = TestWorkspace::new();
    let index_file = workspace.path.join(".index");

    // Phase 1
    workspace.create_file("logic.py", "def init_system():\n    pass");
    {
        let mut indexer = Indexer::new();
        let mut staging = StagingArea::default(); // 1. Create staging
        indexer.scan(&workspace.path, &mut staging);
        indexer.save(&index_file).unwrap();
    }

    // Phase 2
    workspace.create_file("logic.py", "def init_system_v2():\n    pass");
    workspace.create_file("utils.py", "def helper():\n    pass");

    {
        let mut indexer = Indexer::load_from_file(&index_file).unwrap();
        
        assert!(has_func(&indexer.index, "init_system"));
        assert!(!has_func(&indexer.index, "helper"));

        // 2. Create a FRESH staging area for the update scan
        let mut staging = StagingArea::default(); 
        indexer.scan(&workspace.path, &mut staging);

        assert!(has_func(&indexer.index, "helper"), "Incremental scan missed new file");
        assert!(has_func(&indexer.index, "init_system_v2"), "Incremental scan missed modified function");
    }
}

#[test]
fn test_corrupted_index_recovery() {
    let workspace = TestWorkspace::new();
    let index_file = workspace.path.join("corrupt.index");
    workspace.create_file("corrupt.index", "This is not a valid rkyv archive");
    
    let indexer = Indexer::load_from_file(&index_file).expect("Should recover from corruption");
    assert!(indexer.index.files.is_empty());
}