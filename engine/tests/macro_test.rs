mod common;
use common::TestWorkspace;
use rfc_engine::indexer::Indexer;

#[test]
fn test_rust_macro_definitions() {
    let workspace = TestWorkspace::new();

    // 1. Define a standard macro
    workspace.create_file("src/macros.rs", r#"
        macro_rules! create_handler {
            ($name:ident) => {
                fn $name() { println!("handled"); }
            }
        }
    "#);

    // 2. Use the macro to define a function (The Heuristic)
    workspace.create_file("src/handlers.rs", r#"
        create_handler!(LoginHandler);
    "#);

    // 3. Define a generic lazy static (Specific Pattern)
    workspace.create_file("src/config.rs", r#"
        lazy_static! {
            pub static ref GLOBAL_CONFIG: HashMap<u32, String> = HashMap::new();
        }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    // Assert macro definition found
    assert!(indexer.index.symbol_map.contains_key("create_handler"));

    // Assert heuristic worked (LoginHandler found inside macro invocation)
    assert!(indexer.index.symbol_map.contains_key("LoginHandler"));

    // Assert specific pattern worked (GLOBAL_CONFIG found)
    assert!(indexer.index.symbol_map.contains_key("GLOBAL_CONFIG"));
}