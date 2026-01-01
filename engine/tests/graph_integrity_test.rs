mod common;
use common::TestWorkspace;
use rfc_engine::resolution::{Indexer, pipeline::Pipeline};
use rfc_engine::models::{EdgeKind, StagingArea, SymbolKind};

#[test]
fn test_graph_structural_integrity() {
    let workspace = TestWorkspace::new();
    
    // Create a scenario with deep nesting and interactions
    workspace.create_file("src/main.ts", r#"
        class Container {
            childMethod() { return 1; }
        }
        function independent() {}
    "#);

    let mut indexer = Indexer::new();
    let mut pipeline = Pipeline::new();
    pipeline.run(&mut indexer, &workspace.path);

    let graph = &indexer.index.graph;

    // 1. Check Parent-Child Consistency
    for symbol in indexer.index.symbols.values() {
        if let Some(parent_id) = symbol.parent_id {
            // If I have a parent, the parent MUST have a Contains edge to me
            let parent_edges = graph.get(&parent_id).expect("Parent should exist in graph");
            
            let has_link = parent_edges.iter().any(|e| 
                e.target_id == symbol.id && e.kind == EdgeKind::Contains
            );
            
            assert!(has_link, "Symbol {} has parent {} but parent has no Contains edge", symbol.name, parent_id);
        }
    }

    // 2. Check Module Consistency
    for symbol in indexer.index.symbols.values() {
        if symbol.kind != SymbolKind::Module && symbol.kind != SymbolKind::External {
            // Every non-module symbol must effectively belong to a module (file)
            // This is checked via parent_id recursion usually, but here we check
            // if the file_id matches the module symbol for that file.
            
            let mod_sym = indexer.index.symbols.values().find(|s| 
                s.file_id == symbol.file_id && s.kind == SymbolKind::Module
            );
            
            assert!(mod_sym.is_some(), "Symbol {} exists in file {} but no Module symbol found", symbol.name, symbol.file_id);
        }
    }
}