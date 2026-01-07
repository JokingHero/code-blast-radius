mod common;
use common::TestWorkspace;
use blast_radius_engine::resolution::Indexer;

use crate::common::get_calls;

#[test]
fn test_namespace_import_resolution() {
    let workspace = TestWorkspace::new();
    
    // 1. Target File (The "Container" Module)
    // This file acts as a module that exports functions.
    // Our logic treats this file effectively as a "Singleton Class" named "(module) utils".
    workspace.create_file("utils.ts", r#"
        export function add(a, b) { return a + b; }
        export function subtract(a, b) { return a - b; }
    "#);

    // 2. Consumer File (Using Namespace Import)
    // "import * as MathLib" should create a local variable type hint:
    // MathLib -> (module) utils
    workspace.create_file("main.ts", r#"
        import * as MathLib from "./utils"; 

        function runCalculation() {
            // The analyzer sees "MathLib.add".
            // 1. It looks at local variables for 'runCalculation', doesn't find MathLib.
            // 2. It walks up to the Module parent, finds 'MathLib' mapped to '(module) utils'.
            // 3. It looks inside '(module) utils' for a method named 'add'.
            // 4. It finds it and links 'runCalculation' -> 'add'.
            MathLib.add(10, 5);
        }
    "#);

    let mut indexer = Indexer::new();
    common::run_pipeline(&mut indexer, &workspace.path);

    // 3. Verification
    // We expect the 'runCalculation' symbol to have a resolved call to 'add'.
    
    let run_ids = indexer.lookup.symbol_map.get("runCalculation").expect("Should find function runCalculation");
    let run_id = run_ids[0];

    let add_ids = indexer.lookup.symbol_map.get("add").expect("Should find function add");
    let add_id = add_ids[0];
    
    // Get resolved calls from runCalculation
    let resolved = get_calls(&indexer.index, run_id);

    // Assert linkage
    assert!(resolved.contains(&add_id), "Namespace call MathLib.add() failed to resolve to add()");
}

#[test]
fn test_namespace_resolution_deep_scope() {
    let workspace = TestWorkspace::new();

    workspace.create_file("logger.ts", "export function log(msg) {}");

    // Test that the scope walking works (Function -> Class -> Module)
    workspace.create_file("app.ts", r#"
        import * as Logger from "./logger";

        class App {
            start() {
                // Logger is not defined in start(), nor in App class.
                // It is defined in the Module scope.
                // The resolver must walk up 2 levels to find the type hint.
                Logger.log("Starting...");
            }
        }
    "#);

    let mut indexer = Indexer::new();
    common::run_pipeline(&mut indexer, &workspace.path);

    let start_id = indexer.lookup.symbol_map.get("start").unwrap()[0];
    let log_id = indexer.lookup.symbol_map.get("log").unwrap()[0];

    let resolved = get_calls(&indexer.index, start_id);
    
    assert!(resolved.contains(&log_id), "Deeply nested method failed to resolve module-level namespace import");
}