mod common;
use common::TestWorkspace;
use rfc_engine::indexer::Indexer;
use rfc_engine::analyzer::find_related_symbols; 

// Helper to find ID by name
fn has_func(index: &rfc_engine::models::WorkspaceIndex, name: &str) -> bool {
    index.symbol_map.contains_key(name)
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
    // Essential for imports to work:
    indexer.resolve_references();

    // Verify functions exist
    assert!(has_func(&indexer.index, "add"), "Function 'add' not found");
    assert!(has_func(&indexer.index, "calculateTotal"), "Function 'calculateTotal' not found");

    // Verify Semantic Cluster Finding (Bidirectional)
    let related = find_related_symbols(&indexer.index, "add");
    assert!(related.is_some(), "Related symbols not found");
    let symbol_ids = related.unwrap();
    
    let names: Vec<String> = symbol_ids.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();

    // Debug print to see what we actually got
    println!("Found symbols: {:?}", names);

    // We expect 4 symbols now:
    // 1. add (Target)
    // 2. calculateTotal (Caller)
    // 3. (module) utils (Parent of add)
    // 4. (module) main (Parent of calculateTotal)
    assert_eq!(symbol_ids.len(), 4, "Expected 4 symbols: target, caller, and their module parents");
    
    assert!(names.contains(&"add".to_string()));
    assert!(names.contains(&"calculateTotal".to_string()));
    
    // Check for module presence loosely since the exact name depends on OS path separation sometimes
    assert!(names.iter().any(|n| n.contains("(module) utils")));
    assert!(names.iter().any(|n| n.contains("(module) main")));
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

    let id = indexer.index.symbol_map.get("my_rust_func").unwrap()[0];
    let sym = indexer.index.symbols.get(&id).unwrap();
    let docs = sym.doc_comment.as_ref().unwrap();
    
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

    assert!(has_func(&indexer.index, "py_func"));
    assert!(has_func(&indexer.index, "js_func"));
}