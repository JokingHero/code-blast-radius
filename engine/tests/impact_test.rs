mod common;
use common::TestWorkspace;
use rfc_engine::indexer::Indexer;

#[test]
fn test_file_impact_analysis() {
    let workspace = TestWorkspace::new();

    // 1. A leaf file
    workspace.create_file("utils.ts", "export function add(a, b) { return a + b; }");

    // 2. Two files that import it
    workspace.create_file("math_logic.ts", "import { add } from './utils';");
    workspace.create_file("ui_component.ts", "import { add } from './utils';");

    // 3. An unrelated file
    workspace.create_file("styles.css", "body { color: red; }");

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    // 4. Run Impact Analysis on utils.ts
    let target = workspace.path.join("utils.ts");
    let impacted = indexer.get_impacted_files(&target);

    // Should find math_logic and ui_component
    assert_eq!(impacted.len(), 2);
    
    let impacted_str = impacted.join(" ");
    assert!(impacted_str.contains("math_logic.ts"));
    assert!(impacted_str.contains("ui_component.ts"));
    assert!(!impacted_str.contains("styles.css"));
}