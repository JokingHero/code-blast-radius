mod common;
use common::TestWorkspace;
use blast_radius_engine::resolution::{Indexer, pipeline::Pipeline};
use std::fs;

#[test]
fn test_ghost_data_removal() {
    let workspace = TestWorkspace::new();
    let index_file = workspace.path.join(".index");

    // 1. Initial State
    workspace.create_file("temp.js", "function old_function() {}");

    {
        let mut indexer = Indexer::new();
        let pipeline = Pipeline::new();
        pipeline.scan(&mut indexer, &workspace.path);
        indexer.save(&index_file).unwrap();
        
        // Check symbol existence directly in index
        assert!(indexer.index.symbols.values().any(|f| f.name == "old_function"), "Setup failed: old_function not found");
    }

    // 2. Modification
    workspace.create_file("temp.js", "function new_function() {}");

    {
        let mut indexer = Indexer::load_from_file(&index_file).unwrap();
        let pipeline = Pipeline::new();
        pipeline.scan(&mut indexer, &workspace.path);
        
        assert!(indexer.index.symbols.values().any(|f| f.name == "new_function"), "New function not found");
        assert!(!indexer.index.symbols.values().any(|f| f.name == "old_function"), "Ghost Data: old_function still exists after rename!");
    }

    // 3. Deletion
    fs::remove_file(workspace.path.join("temp.js")).unwrap();

    {
        let mut indexer = Indexer::load_from_file(&index_file).unwrap();
        let pipeline = Pipeline::new();
        pipeline.scan(&mut indexer, &workspace.path);
        
        assert!(!indexer.index.symbols.values().any(|f| f.name == "new_function"), "Ghost Data: function still exists after file deletion!");
        assert!(indexer.index.symbols.is_empty(), "Graph should be empty");
    }
}