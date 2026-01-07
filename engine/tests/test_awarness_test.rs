mod common;
use common::TestWorkspace;
use blast_radius_engine::resolution::{Indexer, pipeline::Pipeline};
use blast_radius_engine::query::traversal::{find_related_symbols, generate_context_from_ids};

#[test]
fn test_path_based_test_detection() {
    let workspace = TestWorkspace::new();

    // 1. Create a production file
    workspace.create_file("src/auth.ts", "export function login() {}");
    
    // 2. Create various test file patterns
    workspace.create_file("tests/login_test.ts", "function test_login_flow() {}");
    workspace.create_file("src/auth.spec.ts", "function auth_spec() {}");
    workspace.create_file("__tests__/internal.ts", "function internal_test() {}");

    let mut indexer = Indexer::new();
    let pipeline = Pipeline::new();
    pipeline.scan(&mut indexer, &workspace.path, Some("root_1"));

    // Verify FileNode flags
    let files = &indexer.index.files;
    let find_file = |name: &str| files.values().find(|f| f.relative_path.contains(name)).unwrap();

    assert!(!find_file("src/auth.ts").is_test, "Production file should not be flagged as test");
    assert!(find_file("tests/login_test.ts").is_test, "Files in tests/ should be flagged");
    assert!(find_file("src/auth.spec.ts").is_test, ".spec.ts should be flagged");
    assert!(find_file("__tests__/internal.ts").is_test, "__tests__ folder should be flagged");
}

#[test]
fn test_inline_symbol_test_detection() {
    let workspace = TestWorkspace::new();

    // 1. Rust style inline tests
    workspace.create_file("lib.rs", r#"
        fn add(a: i32, b: i32) -> i32 { a + b }

        #[test]
        fn test_add_logic() {
            assert_eq!(add(2, 2), 4);
        }
    "#);

    // 2. JavaScript style "it" blocks
    workspace.create_file("component.js", r#"
        function render() { return "<div></div>"; }
        
        it("renders correctly", () => {
            render();
        });
    "#);

    let mut indexer = Indexer::new();
    let pipeline = Pipeline::new();
    pipeline.scan(&mut indexer, &workspace.path, Some("root_1"));

    let get_sym = |name: &str| {
        let id = indexer.lookup.symbol_map.get(name).unwrap()[0];
        indexer.index.symbols.get(&id).unwrap()
    };

    // Rust assertions
    assert!(!get_sym("add").is_test);
    assert!(get_sym("test_add_logic").is_test, "Rust #[test] should trigger is_test flag");

    // JS assertions (Note: your parser config might name the 'it' block 'anonymous' or the inner text depending on grammar)
    // Assuming the name is captured as 'it' or the first arg
    if let Some(ids) = indexer.lookup.symbol_map.get("it") {
        let sym = indexer.index.symbols.get(&ids[0]).unwrap();
        assert!(sym.is_test, "JS it() blocks should trigger is_test flag");
    }
}

#[test]
fn test_context_filtering_logic() {
    let workspace = TestWorkspace::new();

    // Setup: Production code
    workspace.create_file("math.ts", "export function multiply(a, b) { return a * b; }");
    
    // Setup: Test code calling production code
    workspace.create_file("math.test.ts", r#"
        import { multiply } from "./math";
        function test_multiply() {
            const res = multiply(2, 5);
        }
    "#);

    let mut indexer = Indexer::new();
    let mut pipeline = Pipeline::new();
    // Use explicit root ID
    pipeline.scan(&mut indexer, &workspace.path, Some("root_1"));

    // Manual Hydration
    let mut active_roots = std::collections::HashMap::new();
    active_roots.insert("root_1".to_string(), workspace.path.clone());
    let (pm, im) = pipeline.hydrate_maps(&indexer.index, &active_roots);
    indexer.path_map = pm;
    indexer.id_map = im;

    // Run resolution
    let mut staging = pipeline.hydrate_staging(&indexer.index);
    let root_paths = vec![workspace.path.clone()];
    pipeline.resolve(&mut indexer, &mut staging, &root_paths);

    // 1. Find related symbols for "multiply"
    // This will find "multiply" (target) and "test_multiply" (upstream caller)
    let related_ids = find_related_symbols(&indexer.index, &indexer.lookup, &indexer.reverse_graph, "multiply", None).expect("Should find symbols");

    // 2. Generate context WITH tests (default/false)
    let context_with_tests = generate_context_from_ids(&indexer.index, &related_ids, &indexer.id_map, true, false);
    assert!(context_with_tests.contains("multiply"), "Should contain prod code");
    assert!(context_with_tests.contains("test_multiply"), "Should contain test code when not excluded");
    assert!(context_with_tests.contains("math.test.ts"), "Should show test file header");

    // 3. Generate context WITHOUT tests (true)
    let context_no_tests = generate_context_from_ids(&indexer.index, &related_ids, &indexer.id_map, true, true);
    assert!(context_no_tests.contains("multiply"), "Should still contain prod code");
    assert!(!context_no_tests.contains("test_multiply"), "Should EXCLUDE test function");
    assert!(!context_no_tests.contains("math.test.ts"), "Should EXCLUDE test file header");
    assert!(context_no_tests.contains("Note: Test files and functions have been excluded"), "Should contain exclusion note");
}

#[test]
fn test_python_test_detection() {
    let workspace = TestWorkspace::new();

    workspace.create_file("logic.py", "def add(a, b): return a + b");
    workspace.create_file("test_logic.py", "def test_add(): assert add(1, 1) == 2");

    let mut indexer = Indexer::new();
    let pipeline = Pipeline::new();
    pipeline.scan(&mut indexer, &workspace.path, Some("root_1"));
    
    // Use stricter matching to ensure we don't accidentally grab "test_logic.py"
    // when looking for "logic.py"
    let prod_file = indexer.index.files.values()
        .find(|f| f.relative_path.ends_with("/logic.py") || f.relative_path.ends_with("\\logic.py") || f.relative_path == "logic.py")
        .expect("Should find logic.py");
        
    let test_file = indexer.index.files.values()
        .find(|f| f.relative_path.contains("test_logic.py"))
        .expect("Should find test_logic.py");

    assert!(!prod_file.is_test, "logic.py should NOT be flagged as a test");
    assert!(test_file.is_test, "test_logic.py should be flagged as a test");
}