mod common;
use common::TestWorkspace;
use rfc_engine::indexer::Indexer;
use rfc_engine::analyzer::find_call_chain;

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
    let graph = indexer.export_graph();

    // 3. Verify Graph has 3 nodes (main, a::duplicate, b::duplicate)
    // In the old broken version, this would be 2 nodes (main, duplicate).
    assert_eq!(graph.len(), 3, "Graph should contain 3 unique nodes, found {}", graph.len());

    // 4. Verify Call Chain
    // Finding 'duplicate_func' should trace back to 'main'
    // Since 'main' calls 'duplicate_func', and we loose-match, 
    // it should find a path like: main.js::main -> a.js::duplicate_func
    let chain = find_call_chain(&graph, "duplicate_func");
    
    assert!(chain.is_some(), "Should find a chain for duplicate_func");
    let c = chain.unwrap();
    
    // Assert structure: [main_node, duplicate_node]
    assert_eq!(c.len(), 2);
    assert!(c[0].contains("main"), "Chain root should be main");
    assert!(c[1].contains("duplicate_func"), "Chain leaf should be duplicate_func");
    
    println!("Chain found: {:?}", c);
}