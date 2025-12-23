mod common;
use common::TestWorkspace;
use rfc_engine::indexer::Indexer;
use rfc_engine::analyzer::find_call_chain_ids;

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

    // 4. Verify Call Chain
    let chain = find_call_chain_ids(&indexer.index, "duplicate_func");
    
    assert!(chain.is_some(), "Should find a chain for duplicate_func");
    let c = chain.unwrap();
    
    // Check IDs
    assert_eq!(c.len(), 2);
    
    // Helper to get name from ID
    let get_name = |id| indexer.index.symbols.get(&id).unwrap().name.as_str();
    
    assert_eq!(get_name(c[0]), "main");
    assert_eq!(get_name(c[1]), "duplicate_func");
}