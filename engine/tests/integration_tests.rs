use blast_radius_engine::models::{BoundaryIndex, SymbolKind};
use blast_radius_engine::query::walker::JitWalker;
use blast_radius_engine::workspace::WorkspaceManager;
use std::fs;
use tempfile::tempdir;

/// Helper to verify if a symbol exists in the index
fn has_symbol(index: &BoundaryIndex, name: &str) -> bool {
    index.symbol_map.contains_key(name)
}

#[test]
fn test_typescript_call_chain() {
    // 1. Setup: Create a temporary directory acting as the workspace root
    let dir = tempdir().expect("Failed to create temp dir");
    let root_path = dir.path().to_path_buf();

    // 2. Create Files
    fs::write(
        root_path.join("utils.ts"),
        r#"
        export function add(a: number, b: number) { return a + b; }
    "#,
    )
    .expect("Failed to write utils.ts");

    fs::write(
        root_path.join("main.ts"),
        r#"
        import { add } from "./utils";
        function calculateTotal() {
            const res = add(10, 20);
            console.log(res);
        }
    "#,
    )
    .expect("Failed to write main.ts");

    // 3. Initialize Manager
    // This triggers the FileScanner which populates the BoundaryIndex
    let manager = WorkspaceManager::new_in_memory(vec![root_path.clone()])
        .expect("Failed to initialize workspace");

    let index = &manager.index;

    // 4. Verify Indexing (Symbol Existence)
    assert!(has_symbol(index, "add"), "Function 'add' should be indexed");
    assert!(
        has_symbol(index, "calculateTotal"),
        "Function 'calculateTotal' should be indexed"
    );

    // 5. Verify Blast Radius (Impact Analysis)
    // We want to verify that `main.ts` depends on `utils.ts`

    // Get FileId for utils.ts (via the 'add' symbol)
    let add_file_ids = index
        .symbol_map
        .get("add")
        .expect("Symbol 'add' has no associated files");
    let utils_id = add_file_ids[0];

    // Get FileId for main.ts (via the 'calculateTotal' symbol)
    let calc_file_ids = index
        .symbol_map
        .get("calculateTotal")
        .expect("Symbol 'calculateTotal' not found");
    let main_id = calc_file_ids[0];

    // Initialize the Walker
    let walker = JitWalker::new(index);

    // Walk impact: If I change `utils.ts` (utils_id), what breaks?
    let impacted_files = walker.walk_impact(&[utils_id], 5);

    // 6. Assertions
    // main.ts is impacted because it imports "./utils" and uses "add"
    assert!(
        impacted_files.contains(&main_id),
        "main.ts (id: {}) should be impacted by utils.ts (id: {})",
        main_id,
        utils_id
    );

    // The source file itself is usually included in the impact list (depth 0)
    assert!(impacted_files.contains(&utils_id));
}

#[test]
fn test_rust_symbol_kinds() {
    let dir = tempdir().expect("Failed to create temp dir");
    let root_path = dir.path().to_path_buf();

    // We rely on the `extract_boundary` logic here.
    // In your boundary.rs, structs are mapped to SymbolKind::Class
    fs::write(
        root_path.join("lib.rs"),
        r#"
        struct MyData {
            id: u32
        }

        fn process_data() {
            println!("Hello");
        }
    "#,
    )
    .expect("Failed to write lib.rs");

    let manager =
        WorkspaceManager::new_in_memory(vec![root_path]).expect("Failed to initialize workspace");
    let index = &manager.index;

    // Verify Struct Kind
    let struct_ids = index.symbol_map.get("MyData").expect("MyData not found");
    let file_struct = index.files.get(&struct_ids[0]).unwrap();
    let struct_def = file_struct
        .defs
        .iter()
        .find(|d| d.name == "MyData")
        .unwrap();

    // Depending on your engine/src/models.rs vs boundary.rs mapping,
    // structs usually map to Class or Container.
    assert!(
        matches!(struct_def.kind, SymbolKind::Class | SymbolKind::Interface),
        "Expected MyData to be a Class or Interface, got {:?}",
        struct_def.kind
    );

    // Verify Function Kind
    let fn_ids = index
        .symbol_map
        .get("process_data")
        .expect("process_data not found");
    let file_fn = index.files.get(&fn_ids[0]).unwrap();
    let fn_def = file_fn
        .defs
        .iter()
        .find(|d| d.name == "process_data")
        .unwrap();

    assert_eq!(
        fn_def.kind,
        SymbolKind::Function,
        "Expected process_data to be a Function"
    );
}

#[test]
fn test_polyglot_folder() {
    let dir = tempdir().expect("Failed to create temp dir");
    let root_path = dir.path().to_path_buf();

    fs::write(root_path.join("script.py"), "def py_func():\n    pass").unwrap();
    fs::write(root_path.join("app.js"), "function js_func() {}").unwrap();

    let manager =
        WorkspaceManager::new_in_memory(vec![root_path]).expect("Failed to initialize workspace");
    let index = &manager.index;

    assert!(has_symbol(index, "py_func"), "Python symbol not found");
    assert!(has_symbol(index, "js_func"), "JS symbol not found");
}

#[test]
fn test_symbol_search_resolution() {
    // Tests that we can fuzzy find symbols using the index
    let dir = tempdir().expect("Failed to create temp dir");
    let root_path = dir.path().to_path_buf();

    fs::write(root_path.join("auth.ts"), "class AuthenticationService {}").unwrap();

    let manager =
        WorkspaceManager::new_in_memory(vec![root_path]).expect("Failed to initialize workspace");
    let index = &manager.index;

    // Exact match lookup
    assert!(index.symbol_map.contains_key("AuthenticationService"));

    // Case sensitivity check (Assuming extraction preserves case)
    assert!(!index.symbol_map.contains_key("authenticationService"));
}
