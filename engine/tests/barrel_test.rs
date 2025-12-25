mod common;
use common::TestWorkspace;
use rfc_engine::indexer::Indexer;

#[test]
fn test_diamond_export_resolution() {
    let workspace = TestWorkspace::new();

    // 1. The Source
    workspace.create_file("leaf.ts", "export function leafFunc() { return 'hello'; }");

    // 2. Two branches re-exporting the same thing (Diamond)
    workspace.create_file("branch_a.ts", "export * from './leaf';");
    workspace.create_file("branch_b.ts", "export * from './leaf';");

    // 3. Consumer using both (This triggers the cache for leafFunc)
    workspace.create_file("main.ts", r#"
        import { leafFunc } from './branch_a';
        import { leafFunc as second } from './branch_b';
        function run() { 
            leafFunc(); 
            second();
        }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    let leaf_id = indexer.index.symbol_map.get("leafFunc").unwrap()[0];
    let run_id = indexer.index.symbol_map.get("run").unwrap()[0];

    let resolutions = indexer.index.resolved_calls.get(&run_id).unwrap();
    
    // Assert both calls in 'run' resolved to the same leaf implementation
    assert!(resolutions.contains(&leaf_id));
    // Verify our resolution path didn't duplicate symbols
    let leaf_count = resolutions.iter().filter(|&&id| id == leaf_id).count();
    assert_eq!(leaf_count, 1, "Should resolve to a unique ID even if reached via multiple paths");
}

#[test]
fn test_named_export_priority() {
    let workspace = TestWorkspace::new();

    // Two files with same function name
    workspace.create_file("correct.ts", "export function target() {}");
    workspace.create_file("wrong.ts", "export function target() {}");

    // Barrel that has a wildcard AND a specific named export
    // In TS, a named export should take priority over wildcard re-exports
    workspace.create_file("barrel.ts", r#"
        export * from './wrong';
        export { target } from './correct';
    "#);

    workspace.create_file("main.ts", r#"
        import { target } from './barrel';
        function run() { target(); }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    let run_id = indexer.index.symbol_map.get("run").unwrap()[0];
    let correct_id = indexer.index.symbols.iter()
        .find(|(_, s)| s.name == "target" && indexer.index.files.get(&indexer.index.files.values().find(|f| f.id == s.file_id).unwrap().path).unwrap().path.contains("correct.ts"))
        .unwrap().0;

    let resolutions = indexer.index.resolved_calls.get(&run_id).unwrap();
    
    assert!(resolutions.contains(correct_id), "Named export should take priority over wildcard re-export");
}

#[test]
fn test_deep_cycle_detection() {
    let workspace = TestWorkspace::new();

    // A -> B -> C -> A (The Long Loop)
    workspace.create_file("a.ts", "export * from './b';");
    workspace.create_file("b.ts", "export * from './c';");
    workspace.create_file("c.ts", "export * from './a';");
    
    workspace.create_file("main.ts", r#"
        import { ghost } from './a';
        function run() { ghost(); }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    
    // This must not hang or stack overflow
    indexer.resolve_references();

    let run_id = indexer.index.symbol_map.get("run").unwrap()[0];
    let resolutions = indexer.index.resolved_calls.get(&run_id);
    
    assert!(resolutions.is_none() || resolutions.unwrap().is_empty(), "Circular resolution should fail gracefully");
}

#[test]
fn test_mixed_barrel_and_local() {
    let workspace = TestWorkspace::new();

    // File defines 'localFunc' and re-exports 'remoteFunc'
    workspace.create_file("remote.ts", "export function remoteFunc() {}");
    workspace.create_file("barrel.ts", r#"
        export * from './remote';
        export function localFunc() {}
    "#);

    workspace.create_file("main.ts", r#"
        import { localFunc, remoteFunc } from './barrel';
        function run() { 
            localFunc();
            remoteFunc();
        }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    let run_id = indexer.index.symbol_map.get("run").unwrap()[0];
    let resolutions = indexer.index.resolved_calls.get(&run_id).unwrap();

    assert_eq!(resolutions.len(), 2, "Should find both the local and the remote function through the barrel");
}