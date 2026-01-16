use blast_radius_engine::workspace::WorkspaceManager;
use blast_radius_engine::recipes::executor::RecipeExecutor;
use blast_radius_engine::recipes::models::{Recipe, RecipeOperation, FileTransform};
use std::collections::HashMap;
use tempfile::tempdir;
use std::fs;

#[test]
fn test_recipe_globbing_and_filtering() {
    let dir = tempdir().unwrap();
    let root_path = dir.path().to_path_buf();
    
    // 1. Setup files
    fs::create_dir_all(root_path.join("src")).unwrap();
    fs::write(root_path.join("src/auth.ts"), "function login() {}").unwrap();
    fs::write(root_path.join("src/utils.ts"), "function help() {}").unwrap();
    fs::write(root_path.join("src/auth.test.ts"), "function test_login() {}").unwrap();
    fs::write(root_path.join("README.md"), "# Hello").unwrap();

    let manager = WorkspaceManager::new_in_memory(vec![root_path.clone()]).expect("Failed to init");
    let index = &manager.index;

    // 2. Define Recipe
    let recipe = Recipe {
        name: "Source Only".to_string(),
        description: None,
        operations: vec![
            RecipeOperation::AddFiles { pattern: "**/*.ts".to_string() },
            RecipeOperation::RemoveFiles { pattern: "**/*.test.ts".to_string() },
        ],
        transforms: HashMap::new()
    };

    let root_map = manager.get_root_map();
    let executor = RecipeExecutor::new(index, root_map);
    let output = executor.execute_full(&recipe).expect("Execution failed");

    // 3. Assertions
    let paths: Vec<String> = output.files.iter().map(|f| f.metadata.path.clone()).collect();
    
    // Paths are relative
    assert!(paths.iter().any(|p| p.ends_with("src/auth.ts")), "Should include auth.ts");
    assert!(paths.iter().any(|p| p.ends_with("src/utils.ts")), "Should include utils.ts");
    
    assert!(!paths.iter().any(|p| p.ends_with("src/auth.test.ts")), "Should exclude auth.test.ts");
    assert!(!paths.iter().any(|p| p.ends_with("README.md")), "Should exclude README.md");
}

#[test]
fn test_recipe_transformation_focus_mode() {
    let dir = tempdir().unwrap();
    let root_path = dir.path().to_path_buf();

    fs::write(root_path.join("logic.ts"), r#"
        function keepMe() { 
            return "I am visible"; 
        }
        
        function hideMe() { 
            return "I should be hidden"; 
        }
    "#).unwrap();

    let manager = WorkspaceManager::new_in_memory(vec![root_path.clone()]).expect("Failed to init");
    
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

    let root_map = manager.get_root_map();
    let executor = RecipeExecutor::new(&manager.index, root_map);
    let output = executor.execute_full(&recipe).expect("Execution failed");

    assert!(!output.files.is_empty(), "Recipe matched no files.");
    let file_ctx = &output.files[0];
    let content = &file_ctx.content; 

    // 3. Assertions
    assert!(content.contains("function keepMe() {"), "Header for keepMe missing");
    assert!(content.contains("return \"I am visible\""), "Body for keepMe should be visible");

    assert!(content.contains("function hideMe()"), "Header for hideMe missing");
    assert!(!content.contains("return \"I should be hidden\""), "Body for hideMe should be masked");

    assert!(content.contains("/* ..."), "Mask comment should be present");
}

#[test]
fn test_recipe_drift_safety() {
    let dir = tempdir().unwrap();
    let root_path = dir.path().to_path_buf();
    let file_path = root_path.join("drift.ts");

    // 1. Initial State
    fs::write(&file_path, r#"
        function target() { return "original"; }
    "#).unwrap();

    let mut manager = WorkspaceManager::new_in_memory(vec![root_path.clone()]).expect("Failed to init");
    // Initial scan happens in new_in_memory

    // 2. Modify File (Prepend lines to shift byte offsets)
    let new_content = r#"
        // Line 1: Shift content down
        // Line 2: Shift content down
        function target() { return "modified"; }
    "#;
    fs::write(&file_path, new_content).unwrap();

    // 3. Re-scan (The Critical Step to Fix Drift)
    manager.sync();

    // 4. Execute Recipe
    let mut transforms = HashMap::new();
    // Use relative path for key
    transforms.insert("drift.ts".to_string(), FileTransform::Skeletonize(vec!["target".to_string()]));

    let recipe = Recipe {
        name: "Drift Test".to_string(),
        description: None,
        operations: vec![
            RecipeOperation::AddFiles { pattern: "**/*.ts".to_string() },
        ],
        transforms
    };

    let root_map = manager.get_root_map();
    let executor = RecipeExecutor::new(&manager.index, root_map);
    let output = executor.execute_full(&recipe).expect("Execution failed");

    assert!(!output.files.is_empty(), "Recipe matched no files.");
    let content = &output.files[0].content;

    // 5. Assertions
    assert!(content.contains("// Line 1"), "Output should reflect updated file content");
    assert!(content.contains("function target()"), "Function header should be present");
    assert!(!content.contains("return \"modified\""), "Target body should be masked"); 
    assert!(content.contains("/* ..."), "Mask comment should appear");
}