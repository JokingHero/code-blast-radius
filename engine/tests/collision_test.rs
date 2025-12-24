mod common; // This looks for engine/tests/common.rs
use common::TestWorkspace;
use rfc_engine::indexer::Indexer;
use rfc_engine::analyzer::find_related_symbols;

#[test]
fn test_name_collision() {
    let workspace = TestWorkspace::new();
    
    // 1. Setup: Two files defining 'duplicate_func'
    workspace.create_file("a.js", "function duplicate_func() { return 'A'; }");
    workspace.create_file("b.js", "function duplicate_func() { return 'B'; }");
    
    // 2. Setup: A consumer that calls it
    workspace.create_file("main.js", r#"
        function main() { 
            duplicate_func(); 
        }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    // 3. Verify Graph has 3 nodes
    assert_eq!(indexer.index.symbols.len(), 3, "Graph should contain 3 unique nodes");

    // 4. Verify Semantic Cluster (Bidirectional)
    // find_related_symbols will now find the target AND everything connected to it
    let ids = find_related_symbols(&indexer.index, "duplicate_func");
    
    assert!(ids.is_some(), "Should find related symbols for duplicate_func");
    let symbol_ids = ids.unwrap();
    
    // It should find "main" (upstream) and "duplicate_func" (the targets)
    assert!(symbol_ids.len() >= 2);
    
    // Helper to get name from ID
    let get_name = |id| indexer.index.symbols.get(&id).unwrap().name.as_str();
    
    let names: std::collections::HashSet<&str> = symbol_ids.iter().map(|id| get_name(*id)).collect();
    
    assert!(names.contains("main"), "Should have found the caller 'main'");
    assert!(names.contains("duplicate_func"), "Should have found the target 'duplicate_func'");
}