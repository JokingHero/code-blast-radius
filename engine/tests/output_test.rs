use blast_radius_engine::query::output::generate_context_output;
use blast_radius_engine::workspace::WorkspaceManager;
use std::collections::HashMap;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_output_json_structure() {
    // 1. Setup
    let dir = tempdir().expect("Failed to create temp dir");
    let root_path = dir.path().to_path_buf();

    // Construct file content
    let small_gap = "\n".repeat(5);
    let large_gap = format!("\n// {}\n", "-".repeat(250));

    let content = format!(
        "function funcA() {{ return 'A'; }}{}function funcB() {{ return 'B'; }}{}function funcC() {{ return 'C'; }}", 
        small_gap, 
        large_gap
    );

    let file_path = root_path.join("main.ts");
    fs::write(&file_path, &content).expect("Failed to write main.ts");

    // 2. Initialize Manager (Scan/Index)
    let manager = WorkspaceManager::new_in_memory(vec![root_path.clone()])
        .expect("Failed to initialize workspace");

    // 3. Get Symbol IDs
    let index = &manager.index;
    // We just need the file ID. Since all funcs are in the same file, picking any symbol works.
    let file_ids = index.symbol_map.get("funcA").expect("funcA missing");
    let file_id = file_ids[0];

    // 4. Prepare ID Map (FileId -> Absolute Path String)
    // The WorkspaceManager knows relative paths, but generate_context_output needs absolute paths to read content.
    let mut id_map = HashMap::new();
    // We know file_id maps to "main.ts" relative to root
    let abs_path_str = file_path.to_string_lossy().to_string();
    id_map.insert(file_id, abs_path_str);

    // 5. Generate Output
    let output = generate_context_output(index, &[file_id], &id_map);

    // 6. Assertions

    // Target Name (defaults to path of first file if simple list passed)
    assert!(output.target.contains("main.ts"));

    // Files
    assert_eq!(output.files.len(), 1);
    let file_ctx = &output.files[0];

    // Metadata
    assert!(file_ctx.metadata.path.ends_with("main.ts"));
    assert_eq!(file_ctx.metadata.language, "ts");
    assert!(!file_ctx.metadata.is_test);

    // Content equality
    assert_eq!(file_ctx.content, content);

    // Range Logic Check:
    // In v2, `generate_context_output` returns the FULL file by default.
    // It calculates one range: start=1, end=LineCount.
    // It does NOT perform gap-based splitting (that is now handled by RecipeExecutor transforms if needed).

    let expected_lines = content.lines().count();
    println!("Total lines: {}", expected_lines);
    println!("Ranges found: {:?}", file_ctx.metadata.relevant_lines);

    assert_eq!(
        file_ctx.metadata.relevant_lines.len(),
        1,
        "Expected 1 range covering the whole file"
    );

    let range = &file_ctx.metadata.relevant_lines[0];
    assert_eq!(range.start, 1);
    assert_eq!(range.end, expected_lines);
}

#[test]
fn test_output_multi_file() {
    let dir = tempdir().expect("Failed to create temp dir");
    let root_path = dir.path().to_path_buf();

    fs::write(root_path.join("utils.ts"), "export function help() {}").unwrap();
    fs::write(
        root_path.join("main.ts"),
        "import { help } from './utils'; function main() { help(); }",
    )
    .unwrap();

    let manager = WorkspaceManager::new_in_memory(vec![root_path.clone()])
        .expect("Failed to initialize workspace");
    let index = &manager.index;

    // Get File IDs via symbols
    let id_main_file = index.symbol_map.get("main").expect("main not found")[0];
    let id_help_file = index.symbol_map.get("help").expect("help not found")[0];

    // Build ID Map
    let mut id_map = HashMap::new();
    id_map.insert(
        id_main_file,
        root_path.join("main.ts").to_string_lossy().to_string(),
    );
    id_map.insert(
        id_help_file,
        root_path.join("utils.ts").to_string_lossy().to_string(),
    );

    // Generate output for both files
    let output = generate_context_output(index, &[id_main_file, id_help_file], &id_map);

    assert_eq!(output.files.len(), 2, "Should return context for 2 files");

    // Verify paths are present
    let paths: Vec<String> = output
        .files
        .iter()
        .map(|f| f.metadata.path.clone())
        .collect();
    assert!(paths.iter().any(|p| p.contains("utils.ts")));
    assert!(paths.iter().any(|p| p.contains("main.ts")));
}
