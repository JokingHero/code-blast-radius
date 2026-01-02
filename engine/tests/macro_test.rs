mod common;
use common::TestWorkspace;
use blast_radius_engine::{models::StagingArea, resolution::{Indexer, pipeline::Pipeline}};

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
    let mut pipeline = Pipeline::new();
    pipeline.run(&mut indexer, &workspace.path);

    // Assert macro definition found
    assert!(indexer.lookup.symbol_map.contains_key("create_handler"));

    // Assert heuristic worked (LoginHandler found inside macro invocation)
    assert!(indexer.lookup.symbol_map.contains_key("LoginHandler"));

    // Assert specific pattern worked (GLOBAL_CONFIG found)
    assert!(indexer.lookup.symbol_map.contains_key("GLOBAL_CONFIG"));
}

#[test]
fn test_rust_macro_definitions_with_visibility() {
    let workspace = TestWorkspace::new();

    // The heuristic should capture 'MyStruct', skipping 'pub'
    workspace.create_file("src/models.rs", r#"
        create_struct!(pub MyStruct);
    "#);

    let mut indexer = Indexer::new();
    let pipeline = Pipeline::new();
    let mut staging = StagingArea::default();
    pipeline.scan(&mut indexer, &workspace.path, &mut staging);

    // Assert 'MyStruct' is found
    assert!(indexer.lookup.symbol_map.contains_key("MyStruct"), "Failed to extract MyStruct from 'pub MyStruct'");
    
    // Assert 'pub' is NOT found as a symbol
    assert!(!indexer.lookup.symbol_map.contains_key("pub"), "Incorrectly indexed 'pub' keyword as a function name");
}