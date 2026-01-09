mod common;
use common::TestWorkspace;
use blast_radius_engine::resolution::{Indexer, pipeline::Pipeline};

#[test]
fn test_monorepo_package_subpath_resolution() {
    let workspace = TestWorkspace::new();

    // 1. Define a package in a subfolder with a specific name in package.json
    workspace.create_file("packages/ui/package.json", r#"{
        "name": "@my-org/ui",
        "version": "1.0.0"
    }"#);

    // 2. Define the source code inside that package
    workspace.create_file("packages/ui/components/Button.ts", r#"
        export function Button() { return "Click me"; }
    "#);

    // 3. Define a Consumer app in a different folder that imports via the Package Name
    // This matches the pattern: "@my-org/ui" + "/components/Button"
    workspace.create_file("apps/web/src/App.tsx", r#"
        import { Button } from "@my-org/ui/components/Button";
        
        export function App() {
            Button();
        }
    "#);

    let mut indexer = Indexer::new();
    common::run_pipeline(&mut indexer, &workspace.path);

    // --- Assertions ---

    // 1. Verify the Package Map was populated
    // The key "@my-org/ui" should point to "packages/ui" (relative or absolute depending on implementation)
    // We check existence primarily.
    assert!(indexer.lookup.package_path_map.contains_key("@my-org/ui"), "Should have indexed package name from package.json");

    // 2. Verify File Dependency Linkage
    let app_id = indexer.index.files.values()
        .find(|f| f.relative_path.contains("App.tsx"))
        .expect("App.tsx not found").id;

    let button_id = indexer.index.files.values()
        .find(|f| f.relative_path.contains("Button.ts"))
        .expect("Button.ts not found").id;

    let deps = indexer.index.file_dependencies.get(&app_id).expect("App should have dependencies");
    
    assert!(deps.contains(&button_id), 
        "App.tsx should depend on Button.ts via '@my-org/ui' package resolution");
}

#[test]
fn test_monorepo_package_root_resolution() {
    let workspace = TestWorkspace::new();

    // 1. Define 'core' package
    workspace.create_file("libs/core/package.json", r#"{ "name": "@my-org/core" }"#);
    
    // 2. Define index.ts in that package
    workspace.create_file("libs/core/index.ts", "export const VERSION = '1.0';");

    // 3. Import exactly the package name
    workspace.create_file("apps/api/server.ts", r#"
        import { VERSION } from "@my-org/core";
    "#);

    let mut indexer = Indexer::new();
    common::run_pipeline(&mut indexer, &workspace.path);

    let server_id = indexer.index.files.values().find(|f| f.relative_path.contains("server.ts")).unwrap().id;
    let index_id = indexer.index.files.values().find(|f| f.relative_path.contains("index.ts")).unwrap().id;

    let deps = indexer.index.file_dependencies.get(&server_id).unwrap();
    assert!(deps.contains(&index_id), "Importing '@my-org/core' should resolve to 'libs/core/index.ts'");
}

#[test]
fn test_cargo_workspace_mapping() {
    let workspace = TestWorkspace::new();

    // 1. Define a Rust crate in a subfolder
    workspace.create_file("crates/logic/Cargo.toml", r#"
        [package]
        name = "my-logic-crate"
        version = "0.1.0"
    "#);

    workspace.create_file("crates/logic/src/lib.rs", "pub fn do_logic() {}");

    // 2. Run scan
    let mut indexer = Indexer::new();
    let pipeline = Pipeline::new();
    pipeline.scan(&mut indexer, &workspace.path, Some("root_1"));
    
    // 3. Verify Mapping
    // We mainly want to ensure the logic in manifest.rs correctly extracted the name
    // from [package] table.
    assert!(indexer.lookup.package_path_map.contains_key("my-logic-crate"), 
        "Should have indexed crate name 'my-logic-crate' from Cargo.toml");
}