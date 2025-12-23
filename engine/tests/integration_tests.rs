mod common;
use common::TestWorkspace;
use rfc_engine::indexer::Indexer;
use rfc_engine::analyzer::find_call_chain_ids;

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

    // Verify Chain Finding
    let chain = find_call_chain_ids(&indexer.index, "add");
    assert!(chain.is_some(), "Call chain not found");
    let chain_vec = chain.unwrap();
    
    assert_eq!(chain_vec.len(), 2);
    
    // Resolve IDs back to names to verify order
    let name_0 = &indexer.index.symbols.get(&chain_vec[0]).unwrap().name;
    let name_1 = &indexer.index.symbols.get(&chain_vec[1]).unwrap().name;
    
    assert_eq!(name_0, "calculateTotal");
    assert_eq!(name_1, "add");
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