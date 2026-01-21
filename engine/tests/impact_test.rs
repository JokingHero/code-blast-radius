use blast_radius_engine::models::BoundaryIndex;
use blast_radius_engine::query::walker::JitWalker;
use blast_radius_engine::resolution::scanner::FileScanner;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_file_impact_analysis() {
    // 0. Setup Workspace
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let workspace_path = temp_dir.path();
    let root_id = "root_1";

    // 1. A leaf file
    fs::write(
        workspace_path.join("utils.ts"),
        "export function add(a, b) { return a + b; }",
    )
    .unwrap();

    // 2. Two files that import it
    fs::write(
        workspace_path.join("math_logic.ts"),
        "import { add } from './utils';",
    )
    .unwrap();

    fs::write(
        workspace_path.join("ui_component.ts"),
        "import { add } from './utils';",
    )
    .unwrap();

    // 3. An unrelated file
    fs::write(workspace_path.join("styles.css"), "body { color: red; }").unwrap();

    // 4. Scan
    let mut index = BoundaryIndex::new();
    let scanner = FileScanner::new();
    scanner.scan(workspace_path, &mut index, root_id);

    // 5. Identify Target (Resolve "utils.ts" to FileId)
    // In the new architecture, we work with FileIds internally.
    let target_node = index
        .files
        .values()
        .find(|f| f.path == "utils.ts")
        .expect("Target file utils.ts not found in index");

    let target_id = target_node.id;

    // 6. Run Impact Analysis via JitWalker
    // The Walker calculates dependencies on-the-fly based on Symbol matches and Import paths.
    let walker = JitWalker::new(&index);
    let impacted_ids = walker.walk_impact(&[target_id], 5); // Depth 5

    // 7. Verify Results
    // Map IDs back to paths for assertions
    let impacted_paths: Vec<String> = impacted_ids
        .iter()
        .filter_map(|id| index.files.get(id).map(|f| f.path.clone()))
        .collect();

    // Debug output if test fails
    println!("Impacted paths: {:?}", impacted_paths);

    // The Walker returns the seed file (utils.ts) plus downstream dependencies
    assert!(
        impacted_paths.len() >= 3,
        "Should find at least utils, math_logic, and ui_component"
    );

    assert!(
        impacted_paths.contains(&"utils.ts".to_string()),
        "Result should contain the source file"
    );
    assert!(
        impacted_paths.contains(&"math_logic.ts".to_string()),
        "Result should contain math_logic.ts"
    );
    assert!(
        impacted_paths.contains(&"ui_component.ts".to_string()),
        "Result should contain ui_component.ts"
    );

    assert!(
        !impacted_paths.contains(&"styles.css".to_string()),
        "Result should NOT contain styles.css"
    );
}
