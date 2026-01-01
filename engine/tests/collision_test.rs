mod common;
use common::TestWorkspace;
use rfc_engine::resolution::Indexer;
use rfc_engine::query::traversal::find_related_symbols;
use std::collections::HashSet;

#[test]
fn test_name_collision() {
    let workspace = TestWorkspace::new();
    
    // 1. Setup: Two files defining 'duplicate_func'
    workspace.create_file("a.js", "function duplicate_func() { return 'A'; }");
    workspace.create_file("b.js", "function duplicate_func() { return 'B'; }");
    
    // 2. Setup: A consumer that calls it
    workspace.create_file("main.js", r#"
        // This is a top-level call
        duplicate_func(); 

        function main() { 
            duplicate_func(); // This is inside a function
        }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    assert!(indexer.index.symbols.len() > 3, "Graph should contain more than 3 unique nodes");

    assert!(indexer.lookup.symbol_map.contains_key("duplicate_func"), "Should have indexed 'duplicate_func'");
    assert!(indexer.lookup.symbol_map.contains_key("main"), "Should have indexed 'main'");

    let ids_option = find_related_symbols(&indexer, "duplicate_func");
    
    assert!(ids_option.is_some(), "Should find related symbols for duplicate_func");
    let symbol_ids = ids_option.unwrap();
    
    let names: HashSet<&str> = symbol_ids.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.as_str())
        .collect();
    
    println!("DEBUG: Found names: {:?}", names);

    assert!(names.contains("main"), "Should have found 'main'");
    assert!(names.contains("duplicate_func"), "Should have found 'duplicate_func'");
    assert!(names.contains("(module) main"), "Should have found '(module) main'");
    
    assert!(names.contains("(module) a") || names.contains("(module) b"), 
        "Should include context from the module(s) defining duplicate_func. Found: {:?}", names);
}