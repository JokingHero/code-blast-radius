mod common;
use common::TestWorkspace;
use rfc_engine::indexer::Indexer;
use std::fs;

#[test]
fn test_ghost_data_removal() {
    let workspace = TestWorkspace::new();
    let index_file = workspace.path.join(".index");

    // 1. Initial State: File with function 'old_function'
    workspace.create_file("temp.js", "function old_function() {}");

    {
        let mut indexer = Indexer::new();
        indexer.scan(&workspace.path);
        indexer.save(&index_file).unwrap();
        
        let graph = indexer.export_graph();
        // FIX: Check values().any() instead of contains_key
        assert!(graph.values().any(|f| f.name == "old_function"), "Setup failed: old_function not found");
    }

    // 2. Modification: Change 'old_function' to 'new_function'
    // ISSUE B check: 'old_function' should disappear.
    workspace.create_file("temp.js", "function new_function() {}");

    {
        let mut indexer = Indexer::load_from_file(&index_file).unwrap();
        indexer.scan(&workspace.path);
        
        let graph = indexer.export_graph();
        
        // FIX: Check values().any()
        assert!(graph.values().any(|f| f.name == "new_function"), "New function not found");
        assert!(!graph.values().any(|f| f.name == "old_function"), "Ghost Data: old_function still exists after rename!");
    }

    // 3. Deletion: Remove 'temp.js' entirely
    // ISSUE A check: 'new_function' should disappear.
    fs::remove_file(workspace.path.join("temp.js")).unwrap();

    {
        let mut indexer = Indexer::load_from_file(&index_file).unwrap();
        indexer.scan(&workspace.path);
        
        let graph = indexer.export_graph();
        
        // FIX: Check values().any()
        assert!(!graph.values().any(|f| f.name == "new_function"), "Ghost Data: function still exists after file deletion!");
        assert!(graph.is_empty(), "Graph should be empty");
    }
}