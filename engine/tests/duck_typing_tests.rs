mod common;
use common::TestWorkspace;
use rfc_engine::indexer::Indexer;
use rfc_engine::query::traversal::find_related_symbols;

#[test]
fn test_polymorphism_and_specificity() {
    let workspace = TestWorkspace::new();

    // 1. Define multiple shapes with overlapping method names (area)
    workspace.create_file("shapes.ts", r#"
        class Circle {
            getRadius() { return 10; }
            area() { return Math.PI * 100; }
        }

        class Square {
            getSide() { return 10; }
            area() { return 100; }
        }
    "#);

    // 2. Define a generic consumer (Ambiguous)
    workspace.create_file("ambiguous.ts", r#"
        function calculate(shape) {
            return shape.area();
        }
    "#);

    // 3. Define a specific consumer (Narrows down via fingerprint)
    workspace.create_file("specific.ts", r#"
        function calculateCircle(shape) {
            const r = shape.getRadius();
            return shape.area();
        }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    let circle_id = indexer.index.symbol_map.get("Circle").unwrap()[0];
    let square_id = indexer.index.symbol_map.get("Square").unwrap()[0];
    
    // --- Test Case 1: Ambiguity ---
    let ambig_id = indexer.index.symbol_map.get("calculate").unwrap()[0];
    let ambig_res = indexer.index.resolved_calls.get(&ambig_id).unwrap();
    
    // It matches both because both have "area"
    assert!(ambig_res.contains(&circle_id), "Should include Circle for .area()");
    assert!(ambig_res.contains(&square_id), "Should include Square for .area()");

    // --- Test Case 2: Specificity ---
    let spec_id = indexer.index.symbol_map.get("calculateCircle").unwrap()[0];
    let spec_res = indexer.index.resolved_calls.get(&spec_id).unwrap();

    println!("DEBUG: calculateCircle resolves to: {:?}", 
        spec_res.iter().map(|id| indexer.index.symbols[id].name.as_str()).collect::<Vec<_>>());
    // It matches Circle because it has both "area" and "getRadius"
    assert!(spec_res.contains(&circle_id), "Should include Circle for .getRadius() + .area()");
    
    // It should NOT match Square because Square lacks "getRadius"
    assert!(!spec_res.contains(&square_id), "Should NOT include Square (missing getRadius)");

    // --- Test Case 3: Context Retrieval ---
    let related = find_related_symbols(&indexer.index, "calculateCircle").unwrap();
    let names: Vec<String> = related.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();

    assert!(names.contains(&"calculateCircle".to_string()));
    assert!(names.contains(&"Circle".to_string()));
    assert!(!names.contains(&"Square".to_string()), "Context should be clean of irrelevant Square implementation");
}

#[test]
fn test_shared_interface_duck_typing() {
    let workspace = TestWorkspace::new();

    // Test that it works across multiple files
    workspace.create_file("protocol.ts", r#"
        interface ILogger {
            log(msg: string): void;
            error(msg: string): void;
        }
    "#);

    workspace.create_file("app.ts", r#"
        function run(logger) {
            logger.log("running");
            logger.error("failed");
        }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    let run_id = indexer.index.symbol_map.get("run").unwrap()[0];
    let interface_id = indexer.index.symbol_map.get("ILogger").unwrap()[0];

    let resolutions = indexer.index.resolved_calls.get(&run_id).unwrap();
    
    assert!(resolutions.contains(&interface_id), "The function 'run' should be linked to interface 'ILogger' via fingerprint");
}