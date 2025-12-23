mod common;
use common::TestWorkspace;
use rfc_engine::indexer::Indexer;
use rfc_engine::analyzer::find_call_chain;

fn get_func<'a>(graph: &'a rfc_engine::analyzer::CodebaseGraph, name: &str) -> Option<&'a rfc_engine::analyzer::FunctionInfo> {
    graph.values().find(|f| f.name == name)
}

#[test]
fn test_typescript_call_chain() {
    let workspace = TestWorkspace::new();
    
    workspace.create_file("utils.ts", r#"
        export function add(a: number, b: number) { return a + b; }
    "#);

    workspace.create_file("main.ts", r#"
        import { add } from "./utils";
        function calculateTotal() {
            const res = add(10, 20);
            console.log(res);
        }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    let graph = indexer.export_graph();

    // Verify functions exist
    assert!(get_func(&graph, "add").is_some(), "Function 'add' not found");
    assert!(get_func(&graph, "calculateTotal").is_some(), "Function 'calculateTotal' not found");

    // Verify calls logic
    let main_func = get_func(&graph, "calculateTotal").unwrap();
    assert!(
        main_func.calls.contains(&"add".to_string()), 
        "Analyzer failed to detect that calculateTotal calls add"
    );

    // Verify Chain Finding
    let chain = find_call_chain(&graph, "add");
    assert!(chain.is_some());
    let chain_vec = chain.unwrap();
    
    // The chain vector contains Unique Keys now, so we check for substrings
    // Expected: [ ...calculateTotal, ...add ]
    assert_eq!(chain_vec.len(), 2);
    assert!(chain_vec[0].contains("calculateTotal"));
    assert!(chain_vec[1].contains("add"));
}

#[test]
fn test_rust_docs_extraction() {
    let workspace = TestWorkspace::new();
    workspace.create_file("lib.rs", r#"
        /// This is a doc comment
        /// It spans multiple lines
        fn my_rust_func() {
            println!("Hello");
        }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    let graph = indexer.export_graph();

    let func = get_func(&graph, "my_rust_func").expect("Rust function not found");
    let docs = func.documentation.as_ref().unwrap();
    
    assert!(docs.contains("This is a doc comment"));
    assert!(docs.contains("It spans multiple lines"));
}

#[test]
fn test_polyglot_folder() {
    let workspace = TestWorkspace::new();
    workspace.create_file("script.py", "def py_func():\n    pass");
    workspace.create_file("app.js", "function js_func() {}");

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    let graph = indexer.export_graph();

    assert!(get_func(&graph, "py_func").is_some());
    assert!(get_func(&graph, "js_func").is_some());
}