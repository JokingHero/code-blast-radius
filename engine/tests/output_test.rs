mod common;
use common::TestWorkspace;
use rfc_engine::models::StagingArea;
use rfc_engine::resolution::Indexer;
use rfc_engine::query::output::generate_context_output;

#[test]
fn test_output_json_structure_and_merging() {
    let workspace = TestWorkspace::new();
    
    // 1. Construct file content with precise byte gaps.
    // The merge threshold in output.rs is 200 bytes.
    
    // Small gap (5 newlines).
    // Line 1: funcA...
    // Line 2-5: empty
    // Line 6: funcB...
    let small_gap = "\n\n\n\n\n"; 
    
    // Large gap (> 250 bytes) -> Should split
    // This creates one very long line on Line 7
    let large_gap = format!("\n// {}\n", "-".repeat(250)); 

    let content = format!(
        "function funcA() {{ return 'A'; }}{}function funcB() {{ return 'B'; }}{}function funcC() {{ return 'C'; }}", 
        small_gap, 
        large_gap
    );
    
    workspace.create_file("main.ts", &content);

    // 2. Index the file
    let mut indexer = Indexer::new();
    let mut staging = StagingArea::default(); 
    indexer.scan(&workspace.path, &mut staging);
    
    // 3. Get Symbol IDs
    let id_a = indexer.lookup.symbol_map.get("funcA").expect("funcA missing")[0];
    let id_b = indexer.lookup.symbol_map.get("funcB").expect("funcB missing")[0];
    let id_c = indexer.lookup.symbol_map.get("funcC").expect("funcC missing")[0];

    // 4. Generate Output passing all 3 IDs
    let ids = vec![id_a, id_b, id_c];
    let output = generate_context_output(&indexer.index, &ids);

    // 5. Assertions
    
    // Target Name
    assert_eq!(output.target, "funcA");

    // Files
    assert_eq!(output.files.len(), 1);
    let file_ctx = &output.files[0];

    // Metadata
    assert!(file_ctx.path.ends_with("main.ts"));
    assert_eq!(file_ctx.language, "ts");
    assert!(!file_ctx.is_test);
    
    // Content equality
    assert_eq!(file_ctx.content, content);

    // Range Merging Logic
    // funcA and funcB are separated by `small_gap` (5 bytes) -> MERGED
    // funcB and funcC are separated by `large_gap` (>200 bytes) -> SPLIT
    // Expected result: 2 Ranges.
    
    // DEBUG OUTPUT
    println!("Found {} ranges:", file_ctx.relevant_lines.len());
    for (i, r) in file_ctx.relevant_lines.iter().enumerate() {
        println!("Range {}: Lines {}-{}", i, r.start, r.end);
    }

    assert_eq!(file_ctx.relevant_lines.len(), 2, "Expected 2 ranges: [A+B] and [C]");

    // Verify Range 1 (A + B)
    let r1 = &file_ctx.relevant_lines[0];
    assert_eq!(r1.start, 1, "Range 1 should start at line 1 (funcA)");
    // funcA (line 1) + 5 newlines = funcB starts on line 6.
    assert!(r1.end >= 6, "Range 1 should cover funcB (at least line 6)");

    // Verify Range 2 (C)
    let r2 = &file_ctx.relevant_lines[1];
    // It must start after the large gap.
    // funcB ends on line 6. large_gap is line 7. funcC starts on line 8.
    // So start must be > r1.end
    assert!(r2.start > r1.end, "Range 2 ({}) should start strictly after Range 1 ({})", r2.start, r1.end);
}

#[test]
fn test_output_multi_file() {
    let workspace = TestWorkspace::new();

    workspace.create_file("utils.ts", "export function help() {}");
    workspace.create_file("main.ts", "import { help } from './utils'; function main() { help(); }");

    let mut indexer = Indexer::new();
    let mut staging = StagingArea::default(); 
    indexer.scan(&workspace.path, &mut staging);
    
    let id_main = indexer.lookup.symbol_map.get("main").unwrap()[0];
    let id_help = indexer.lookup.symbol_map.get("help").unwrap()[0];

    let output = generate_context_output(&indexer.index, &[id_main, id_help]);

    assert_eq!(output.target, "main");
    assert_eq!(output.files.len(), 2, "Should return context for 2 files");

    // Verify both paths are present
    let paths: Vec<String> = output.files.iter().map(|f| f.path.clone()).collect();
    assert!(paths.iter().any(|p| p.contains("utils.ts")));
    assert!(paths.iter().any(|p| p.contains("main.ts")));
}