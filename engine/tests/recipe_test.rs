mod common;
use common::TestWorkspace;
use blast_radius_engine::resolution::Indexer;
use blast_radius_engine::recipes::executor::RecipeExecutor;
use blast_radius_engine::recipes::models::{Recipe, RecipeOperation, FileTransform};
use std::collections::HashMap;

#[test]
fn test_recipe_globbing_and_filtering() {
    let workspace = TestWorkspace::new();

    // 1. Setup files
    workspace.create_file("src/auth.ts", "function login() {}");
    workspace.create_file("src/utils.ts", "function help() {}");
    workspace.create_file("src/auth.test.ts", "function test_login() {}");
    workspace.create_file("README.md", "# Hello");

    let mut indexer = Indexer::new();
    common::run_pipeline(&mut indexer, &workspace.path);

    // 2. Define Recipe
    // Intent: Include all TS files, but exclude tests.
    let recipe = Recipe {
        name: "Source Only".to_string(),
        description: None,
        operations: vec![
            RecipeOperation::AddFiles { pattern: "**/*.ts".to_string() },
            RecipeOperation::RemoveFiles { pattern: "**/*.test.ts".to_string() },
        ],
        transforms: HashMap::new()
    };

    let root_map = HashMap::from([("root_1".to_string(), "test_root".to_string())]);
    let executor = RecipeExecutor::new(&indexer, root_map);
    let output = executor.execute_full(&recipe).expect("Execution failed");

    // 3. Assertions
    let paths: Vec<String> = output.files.iter().map(|f| f.metadata.path.clone()).collect();
    
    // Should match
    assert!(paths.iter().any(|p| p.ends_with("src/auth.ts")), "Should include auth.ts");
    assert!(paths.iter().any(|p| p.ends_with("src/utils.ts")), "Should include utils.ts");
    
    // Should NOT match
    assert!(!paths.iter().any(|p| p.ends_with("src/auth.test.ts")), "Should exclude auth.test.ts");
    assert!(!paths.iter().any(|p| p.ends_with("README.md")), "Should exclude README.md");
}

#[test]
fn test_recipe_transformation_focus_mode() {
    let workspace = TestWorkspace::new();

    // 1. Setup file with two functions
    workspace.create_file("logic.ts", r#"
        function keepMe() { 
            return "I am visible"; 
        }
        
        function hideMe() { 
            return "I should be hidden"; 
        }
    "#);

    let mut indexer = Indexer::new();
    common::run_pipeline(&mut indexer, &workspace.path);

    // 2. Define Recipe with FocusOn
    let mut transforms = HashMap::new();
    transforms.insert("logic.ts".to_string(), FileTransform::FocusOn(vec!["keepMe".to_string()]));

    let recipe = Recipe {
        name: "Focus Logic".to_string(),
        description: None,
        operations: vec![
            RecipeOperation::AddFiles { pattern: "**/logic.ts".to_string() },
        ],
        transforms
    };

    let root_map = HashMap::from([("root_1".to_string(), "test_root".to_string())]);
    let executor = RecipeExecutor::new(&indexer, root_map);
    let output = executor.execute_full(&recipe).expect("Execution failed");

    assert!(!output.files.is_empty(), "Recipe matched no files. Check glob pattern.");

    let file_ctx = &output.files[0];
    let content = &file_ctx.content; // Content is directly on FileContent

    // 3. Assertions

    // keepMe: Should see body
    assert!(content.contains("function keepMe() {"), "Header for keepMe missing");
    assert!(content.contains("return \"I am visible\""), "Body for keepMe should be visible");

    // hideMe: Should see header
    assert!(content.contains("function hideMe()"), "Header for hideMe missing");
    assert!(!content.contains("return \"I should be hidden\""), "Body for hideMe should be masked");

    // Check for the skeleton syntax (/* ... */) instead of specific wording
    assert!(content.contains("/* ..."), "Mask comment should be present");
}

#[test]
fn test_recipe_drift_safety() {
    let workspace = TestWorkspace::new();
    let file_path = "drift.ts";

    // 1. Initial State
    workspace.create_file(file_path, r#"
        function target() { return "original"; }
    "#);

    let mut indexer = Indexer::new();

    // Initial Scan
    common::run_pipeline(&mut indexer, &workspace.path);

    // 2. Modify File (Prepend lines to shift byte offsets)
    let new_content = r#"
        // Line 1: Shift content down
        // Line 2: Shift content down
        function target() { return "modified"; }
    "#;
    workspace.create_file(file_path, new_content);

    // 3. Re-scan (The Critical Step to Fix Drift)
    common::run_pipeline(&mut indexer, &workspace.path);

    // 4. Execute Recipe
    let mut transforms = HashMap::new();
    transforms.insert(file_path.to_string(), FileTransform::Skeletonize(vec!["target".to_string()]));

    let recipe = Recipe {
        name: "Drift Test".to_string(),
        description: None,
        operations: vec![
            RecipeOperation::AddFiles { pattern: "**/*.ts".to_string() },
        ],
        transforms
    };

    let root_map = HashMap::from([("root_1".to_string(), "test_root".to_string())]);
    let executor = RecipeExecutor::new(&indexer, root_map);
    let output = executor.execute_full(&recipe).expect("Execution failed");

    assert!(!output.files.is_empty(), "Recipe matched no files.");
    let content = &output.files[0].content;

    // 5. Assertions
    // Ensure we are operating on the NEW content
    assert!(content.contains("// Line 1"), "Output should reflect updated file content");

    // Ensure mask was applied to the correct location in the NEW content
    assert!(content.contains("function target()"), "Function header should be present");
    assert!(!content.contains("return \"modified\""), "Target body should be masked"); 
    // Check for the skeleton syntax
    assert!(content.contains("/* ..."), "Mask comment should appear");
}