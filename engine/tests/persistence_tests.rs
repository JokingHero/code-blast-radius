mod common;
use common::TestWorkspace;
use rfc_engine::indexer::Indexer;

// Helper to check for function presence by name
fn has_func(graph: &rfc_engine::analyzer::CodebaseGraph, name: &str) -> bool {
    graph.values().any(|f| f.name == name)
}

#[test]
fn test_persistence_lifecycle() {
    let workspace = TestWorkspace::new();
    let index_file = workspace.path.join(".index");

    workspace.create_file("math.ts", r#"
        function add(a, b) { return a + b; }
    "#);

    // 1. Run 1: Initial Scan & Save
    {
        let mut indexer = Indexer::new();
        indexer.scan(&workspace.path);
        let graph = indexer.export_graph();
        assert!(has_func(&graph, "add"), "Initial scan failed to find 'add'");
        indexer.save(&index_file).expect("Failed to save index");
    }

    // 2. Run 2: Load from Disk
    {
        let loaded_indexer = Indexer::load_from_file(&index_file)
            .expect("Failed to load index");
        let graph = loaded_indexer.export_graph();
        assert!(has_func(&graph, "add"), "Loaded index missing 'add' function");
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
        indexer.scan(&workspace.path);
        indexer.save(&index_file).unwrap();
    }

    // Phase 2
    workspace.create_file("logic.py", "def init_system_v2():\n    pass");
    workspace.create_file("utils.py", "def helper():\n    pass");

    {
        let mut indexer = Indexer::load_from_file(&index_file).unwrap();
        let old_graph = indexer.export_graph();
        assert!(has_func(&old_graph, "init_system"));
        assert!(!has_func(&old_graph, "helper"));

        indexer.scan(&workspace.path);

        let new_graph = indexer.export_graph();
        assert!(has_func(&new_graph, "helper"), "Incremental scan missed new file");
        assert!(has_func(&new_graph, "init_system_v2"), "Incremental scan missed modified function");
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