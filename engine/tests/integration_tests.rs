// Import the common helper module
mod common;
use common::TestWorkspace;

// Import the engine logic
use rfc_engine::analyzer::{build_codebase_graph, find_call_chain};
use rfc_engine::language::get_language_configs;

#[test]
fn test_typescript_call_chain() {
    // 1. Setup the "Pseudo Repository"
    let workspace = TestWorkspace::new();
    
    // Create a generic helper file
    workspace.create_file("utils.ts", r#"
        /**
         * Adds two numbers
         */
        export function add(a: number, b: number) {
            return a + b;
        }
    "#);

    // Create a main file that calls the helper
    workspace.create_file("main.ts", r#"
        import { add } from "./utils";

        function calculateTotal() {
            const res = add(10, 20);
            console.log(res);
        }
    "#);

    // 2. Run the Analyzer
    let configs = get_language_configs();
    let graph = build_codebase_graph(&workspace.path, &configs);

    // 3. Assertions
    
    // Check if functions were found
    assert!(graph.contains_key("add"), "Function 'add' was not found in graph");
    assert!(graph.contains_key("calculateTotal"), "Function 'calculateTotal' was not found");

    // Check call detection
    let main_func = graph.get("calculateTotal").unwrap();
    assert!(
        main_func.calls.contains(&"add".to_string()), 
        "Analyzer failed to detect that calculateTotal calls add"
    );

    // Check Reverse Call Chain Logic
    // If we ask: "How did we get to 'add'?", the answer should be "calculateTotal -> add"
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

    let configs = get_language_configs();
    let graph = build_codebase_graph(&workspace.path, &configs);

    let func = graph.get("my_rust_func").expect("Rust function not found");
    
    assert!(func.documentation.is_some());
    let docs = func.documentation.as_ref().unwrap();
    
    assert!(docs.contains("This is a doc comment"));
    assert!(docs.contains("It spans multiple lines"));
}

#[test]
fn test_polyglot_folder() {
    // Tests that we can handle a folder with mixed languages (Python + JS)
    let workspace = TestWorkspace::new();

    workspace.create_file("script.py", "def py_func():\n    pass");
    workspace.create_file("app.js", "function js_func() {}");

    let configs = get_language_configs();
    let graph = build_codebase_graph(&workspace.path, &configs);

    assert!(graph.contains_key("py_func"));
    assert!(graph.contains_key("js_func"));
}