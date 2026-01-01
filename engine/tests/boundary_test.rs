mod common;
use common::TestWorkspace;
use rfc_engine::{models::StagingArea, resolution::Indexer};

use crate::common::get_calls;

#[test]
fn test_external_stub_generation() {
    let workspace = TestWorkspace::new();
    
    // 1. Create a package.json to establish "axios" as a known dependency
    workspace.create_file("package.json", r#"{
        "dependencies": {
            "axios": "^1.0.0"
        }
    }"#);

    // 2. Create code that uses it
    workspace.create_file("api.ts", r#"
        import { get } from "axios";
        
        export function fetchData() {
            return get("/users");
        }
    "#);

    // 3. Create a fake "get" function locally to ensure we DON'T resolve to it
    workspace.create_file("local_utils.ts", r#"
        export function get() { return "I am local"; }
    "#);

    let mut indexer = Indexer::new();
    let mut staging = StagingArea::default();
    indexer.scan(&workspace.path, &mut staging);
    indexer.resolve_references(&mut staging);

    // 4. Verify External Package Detection
    assert!(indexer.lookup.external_packages.contains("axios"), "Should detect axios in package.json");

    // 5. Verify Call Resolution
    let fetch_id = indexer.lookup.symbol_map.get("fetchData").unwrap()[0];
    let resolved = get_calls(&indexer.index, fetch_id);

    // Get the ID of the 'get' it resolved to
    assert!(!resolved.is_empty());
    let target_id = resolved[0];
    let target_sym = indexer.index.symbols.get(&target_id).unwrap();

    // The resolved symbol should be External, NOT the local one
    assert!(target_sym.is_external, "Should resolve to external stub");
    assert_eq!(target_sym.external_source.as_deref(), Some("axios"));
    assert_eq!(target_sym.name, "get");
}

#[test]
fn test_ignore_file_respect() {
    let workspace = TestWorkspace::new();
    
    // --- FIX IS HERE ---
    // The `ignore` crate defaults to respecting .gitignore only inside git repos.
    // We create a fake .git directory to enable this behavior in the temp workspace.
    std::fs::create_dir(workspace.path.join(".git")).expect("Failed to create .git dir");

    workspace.create_file(".gitignore", "node_modules/");
    workspace.create_file("src/main.ts", "function main() {}");
    workspace.create_file("node_modules/bad_lib.ts", "function hidden() {}");

    let mut indexer = Indexer::new();
    let mut staging = StagingArea::default();
    indexer.scan(&workspace.path, &mut staging);
    indexer.resolve_references(&mut staging);

    // Should find main.ts
    assert!(indexer.index.files.keys().any(|k| k.contains("src/main.ts")));
    
    // Should NOT find bad_lib.ts inside node_modules
    assert!(!indexer.index.files.keys().any(|k| k.contains("node_modules")), "Should respect .gitignore");
}