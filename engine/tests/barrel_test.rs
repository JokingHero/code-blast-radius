mod common;
use common::TestWorkspace;
use rfc_engine::indexer::Indexer;

#[test]
fn test_wildcard_barrel_resolution() {
    let workspace = TestWorkspace::new();

    // 1. The Implementation
    workspace.create_file("math/add.ts", "export function add(a, b) { return a + b; }");

    // 2. The Barrel (Wildcard)
    workspace.create_file("math/index.ts", "export * from './add';");

    // 3. The Consumer (Imports from the barrel)
    workspace.create_file("main.ts", r#"
        import { add } from './math/index';
        function run() { add(1, 2); }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    let add_id = indexer.index.symbol_map.get("add").expect("Should find 'add'")[0];
    let run_id = indexer.index.symbol_map.get("run").expect("Should find 'run'")[0];

    let resolutions = indexer.index.resolved_calls.get(&run_id).expect("run should have resolutions");
    
    // Assert that 'run' links DIRECTLY to 'math/add.ts', traversing through 'index.ts'
    assert!(resolutions.contains(&add_id), "Should resolve 'add' through a wildcard barrel");
    
    let sym = &indexer.index.symbols[&add_id];
    let file = &indexer.index.files.values().find(|f| f.id == sym.file_id).unwrap();
    assert!(file.path.contains("add.ts"), "Symbol should be located in the implementation file, not the barrel");
}

#[test]
fn test_named_barrel_resolution() {
    let workspace = TestWorkspace::new();

    workspace.create_file("auth/logic.ts", "export function login() {}");
    // Explicitly naming the export in the barrel
    workspace.create_file("auth/index.ts", "export { login } from './logic';");
    workspace.create_file("app.ts", r#"
        import { login } from './auth/index';
        function init() { login(); }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    let login_id = indexer.index.symbol_map.get("login").unwrap()[0];
    let init_id = indexer.index.symbol_map.get("init").unwrap()[0];

    let resolutions = indexer.index.resolved_calls.get(&init_id).unwrap();
    assert!(resolutions.contains(&login_id), "Should resolve through named re-export");
}

#[test]
fn test_directory_index_inference() {
    let workspace = TestWorkspace::new();

    // Note: Consumer imports from './utils' (a directory)
    // The engine should automatically look for './utils/index.ts'
    workspace.create_file("utils/index.ts", "export function helper() {}");
    workspace.create_file("main.ts", r#"
        import { helper } from './utils';
        function start() { helper(); }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    let helper_id = indexer.index.symbol_map.get("helper").unwrap()[0];
    let start_id = indexer.index.symbol_map.get("start").unwrap()[0];

    let resolutions = indexer.index.resolved_calls.get(&start_id).unwrap();
    assert!(resolutions.contains(&helper_id), "Should infer index.ts when importing a directory");
}

#[test]
fn test_multi_hop_barrel() {
    let workspace = TestWorkspace::new();

    // deep/a.ts -> deep/index.ts -> root/index.ts -> main.ts
    workspace.create_file("deep/a.ts", "export function deepFunc() {}");
    workspace.create_file("deep/index.ts", "export * from './a';");
    workspace.create_file("index.ts", "export * from './deep';");
    workspace.create_file("main.ts", r#"
        import { deepFunc } from './index';
        function run() { deepFunc(); }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    let deep_id = indexer.index.symbol_map.get("deepFunc").unwrap()[0];
    let run_id = indexer.index.symbol_map.get("run").unwrap()[0];

    let resolutions = indexer.index.resolved_calls.get(&run_id).unwrap();
    assert!(resolutions.contains(&deep_id), "Should resolve across multiple barrel hops");
}

#[test]
fn test_circular_barrel_safety() {
    let workspace = TestWorkspace::new();

    // A -> B -> A (Circular)
    workspace.create_file("a.ts", "export * from './b';");
    workspace.create_file("b.ts", "export * from './a';");
    workspace.create_file("main.ts", r#"
        import { nonexistent } from './a';
        function run() { nonexistent(); }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    
    // This call should not stack overflow/hang
    indexer.resolve_references();

    let run_id = indexer.index.symbol_map.get("run").unwrap()[0];
    let resolutions = indexer.index.resolved_calls.get(&run_id);
    
    // It should simply fail to find the symbol, not crash
    assert!(resolutions.is_none() || resolutions.unwrap().is_empty());
}