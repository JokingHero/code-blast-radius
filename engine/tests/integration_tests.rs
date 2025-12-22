mod common;
use common::TestWorkspace;

use rfc_engine::indexer::Indexer;
use rfc_engine::analyzer::find_call_chain;
// We don't need build_codebase_graph import anymore

#[test]
fn test_typescript_call_chain() {
    let workspace = TestWorkspace::new();
    
    workspace.create_file("utils.ts", r#"
        /**
         * Adds two numbers
         */
        export function add(a: number, b: number) {
            return a + b;
        }
    "#);

    workspace.create_file("main.ts", r#"
        import { add } from "./utils";

        function calculateTotal() {
            const res = add(10, 20);
            console.log(res);
        }
    "#);

    // --- CHANGED LOGIC HERE ---
    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    let graph = indexer.export_graph();
    // --------------------------

    assert!(graph.contains_key("add"), "Function 'add' was not found in graph");
    assert!(graph.contains_key("calculateTotal"), "Function 'calculateTotal' was not found");

    let main_func = graph.get("calculateTotal").unwrap();
    assert!(
        main_func.calls.contains(&"add".to_string()), 
        "Analyzer failed to detect that calculateTotal calls add"
    );

    let chain = find_call_chain(&graph, "add");
    assert!(chain.is_some());
    let chain_vec = chain.unwrap();
    assert_eq!(chain_vec, vec!["calculateTotal", "add"]);
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

    let func = graph.get("my_rust_func").expect("Rust function not found");
    
    assert!(func.documentation.is_some());
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

    assert!(graph.contains_key("py_func"));
    assert!(graph.contains_key("js_func"));
}