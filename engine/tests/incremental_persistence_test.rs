use blast_radius_engine::models::BoundaryIndex;
use blast_radius_engine::query::walker::JitWalker;
use blast_radius_engine::resolution::persistence::PersistenceManager;
use blast_radius_engine::resolution::scanner::FileScanner;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_incremental_graph_integrity() {
    // Setup
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let workspace_path = temp_dir.path();
    let index_file = workspace_path.join(".cblast.index");
    let root_id = "root_1";

    let persistence = PersistenceManager::new();
    let scanner = FileScanner::new();

    // 1. Create a file with a relationship
    // math.ts defines 'add'
    fs::write(
        workspace_path.join("math.ts"),
        "export function add(a, b) { return a + b; }",
    )
    .expect("Failed to write math.ts");

    // main.ts imports 'add' and calls it
    fs::write(
        workspace_path.join("main.ts"),
        r#"
        import { add } from "./math";
        export function main() { add(1, 2); }
    "#,
    )
    .expect("Failed to write main.ts");

    // --- SESSION 1: Initial Scan ---
    {
        let mut index = BoundaryIndex::new();
        scanner.scan(workspace_path, &mut index, root_id);

        // Validation: Verify main.ts references 'add'
        let main_file = index
            .files
            .values()
            .find(|f| f.path == "main.ts")
            .expect("main.ts should be indexed");

        // In the new architecture, checking 'edges' means checking 'symbol_refs'
        assert!(
            main_file.symbol_refs.contains(&"add".to_string()),
            "Session 1: main.ts should contain reference to 'add'"
        );

        persistence
            .save_index(&index, &index_file)
            .expect("Failed to save index");
    }

    // --- SESSION 2: Load & Rescan (Simulating 'Open Workspace') ---
    {
        // Load the existing index
        let mut index = persistence
            .load_index(&index_file)
            .expect("Failed to load index");

        // RUN PIPELINE AGAIN (Incremental)
        // Since files haven't changed on disk, the scanner detects hash matches.
        // It should SKIP parsing but PRESERVE the existing entries.
        scanner.scan(workspace_path, &mut index, root_id);

        // Validation 1: Data Persistence
        // If the scanner mistakenly removed "unchanged" files, this retrieval will fail.
        let main_file = index
            .files
            .values()
            .find(|f| f.path == "main.ts")
            .expect("Session 2: main.ts missing from index after incremental scan!");

        // If the scanner re-inserted a blank entry instead of preserving the old one, this fails.
        assert!(
            main_file.symbol_refs.contains(&"add".to_string()),
            "Session 2: main.ts lost its symbol refs after incremental scan!"
        );

        // Validation 2: Functional Integrity (JitWalker)
        // Ensure the relationship logic still works on the preserved data.
        let math_file = index
            .files
            .values()
            .find(|f| f.path == "math.ts")
            .expect("math.ts missing");

        let walker = JitWalker::new(&index);

        // Ask: Who is impacted by 'math.ts'?
        // The walker looks for files importing './math' OR referencing symbols defined in math.ts
        let impacted_ids = walker.walk_impact(&[math_file.id], 1);

        let impacted_paths: Vec<String> = impacted_ids
            .iter()
            .filter_map(|id| index.files.get(id).map(|f| f.path.clone()))
            .collect();

        assert!(
            impacted_paths.contains(&"main.ts".to_string()),
            "Session 2: Walker failed to link math.ts -> main.ts after incremental load"
        );
    }
}
