mod common;
use common::TestWorkspace;
use rfc_engine::indexer::Indexer;
use rfc_engine::analyzer::find_related_symbols; // Updated Import

// Helper to find ID by name
fn has_func(index: &rfc_engine::schema::WorkspaceIndex, name: &str) -> bool {
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
    
    // Should find 'add' (the target) and 'calculateTotal' (the caller)
    assert_eq!(symbol_ids.len(), 2);
    
    // Get names and sort them for a stable assertion
    let mut names: Vec<String> = symbol_ids.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();
    names.sort();
    
    assert_eq!(names[0], "add");
    assert_eq!(names[1], "calculateTotal");
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