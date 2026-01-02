mod common;
use common::{TestWorkspace, get_calls};
use blast_radius_engine::resolution::{Indexer, pipeline::Pipeline};

#[test]
fn test_incremental_graph_integrity() {
    let workspace = TestWorkspace::new();
    let index_file = workspace.path.join(".index");

    // 1. Create a file with a relationship
    workspace.create_file("math.ts", "export function add(a, b) { return a + b; }");
    workspace.create_file("main.ts", r#"
        import { add } from "./math";
        export function main() { add(1, 2); }
    "#);

    // --- SESSION 1: Initial Scan ---
    {
        let mut indexer = Indexer::new();
        let mut pipeline = Pipeline::new();
        pipeline.run(&mut indexer, &workspace.path);
        
        let main_id = indexer.lookup.symbol_map.get("main").unwrap()[0];
        let calls = get_calls(&indexer.index, main_id);
        assert!(!calls.is_empty(), "Session 1: Edge should exist");

        indexer.save(&index_file).unwrap();
    }

    // --- SESSION 2: Load & Rescan (Simulating 'Open Workspace') ---
    {
        let mut indexer = Indexer::load_from_file(&index_file).unwrap();
        let mut pipeline = Pipeline::new();

        // RUN PIPELINE AGAIN
        // In the current buggy implementation:
        // 1. Scanner sees hash match -> skips parse -> StagingArea empty.
        // 2. Resolver sees empty StagingArea -> clears Graph -> creates NO edges.
        pipeline.run(&mut indexer, &workspace.path);

        let main_id = indexer.lookup.symbol_map.get("main").unwrap()[0];
        let calls = get_calls(&indexer.index, main_id);
        
        // This ASSERTION ensures we fixed the bug
        assert!(!calls.is_empty(), "Session 2: Edge must persist after incremental scan!");
    }
}