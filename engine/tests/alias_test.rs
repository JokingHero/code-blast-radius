mod common;
use common::TestWorkspace;
use rfc_engine::indexer::Indexer;

#[test]
fn test_tsconfig_path_alias() {
    let workspace = TestWorkspace::new();

    // 1. Define tsconfig with alias "@/*" mapping to "src/*"
    workspace.create_file("tsconfig.json", r#"
    {
        "compilerOptions": {
            "paths": {
                "@/*": ["src/*"]
            }
        }
    }
    "#);

    // 2. The Target
    workspace.create_file("src/utils/math.ts", "export function add(a, b) { return a + b; }");

    // 3. The Consumer using the Alias
    workspace.create_file("src/main.ts", r#"
        import { add } from "@/utils/math";
        function calc() { add(1, 1); }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    let main_id = indexer.index.files.values().find(|f| f.path.contains("main.ts")).unwrap().id;
    let math_id = indexer.index.files.values().find(|f| f.path.contains("math.ts")).unwrap().id;

    let deps = indexer.index.file_dependencies.get(&main_id).expect("Dependencies expected");
    
    assert!(deps.contains(&math_id), "Should resolve @/utils/math to src/utils/math.ts using tsconfig.json");
}

#[test]
fn test_rust_crate_alias() {
    let workspace = TestWorkspace::new();

    // 1. Target (in src/utils/helper.rs)
    workspace.create_file("src/utils/helper.rs", r#"
        pub fn help() {}
    "#);

    // 2. Consumer using "crate::" (in src/main.rs)
    workspace.create_file("src/main.rs", r#"
        use crate::utils::helper;
        fn main() {
            helper::help();
        }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    let main_id = indexer.index.files.values().find(|f| f.path.contains("main.rs")).unwrap().id;
    let helper_id = indexer.index.files.values().find(|f| f.path.contains("helper.rs")).unwrap().id;

    let deps = indexer.index.file_dependencies.get(&main_id).expect("Dependencies expected");

    assert!(deps.contains(&helper_id), "Should resolve crate::utils::helper to src/utils/helper.rs automatically");
}

#[test]
fn test_fuzzy_fallback_resolution() {
    let workspace = TestWorkspace::new();

    // 1. Target (nested deep)
    workspace.create_file("src/nested/deep/logic.py", "def compute(): pass");

    // 2. Consumer (sloppy import)
    // Python allows imports relative to PYTHONPATH. 
    // If the tool doesn't know PYTHONPATH, it usually fails.
    // Our fuzzy matcher should see "nested.deep.logic" and find "src/nested/deep/logic.py"
    workspace.create_file("app.py", r#"
        import nested.deep.logic
        nested.deep.logic.compute()
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    let app_id = indexer.index.files.values().find(|f| f.path.contains("app.py")).unwrap().id;
    let logic_id = indexer.index.files.values().find(|f| f.path.contains("logic.py")).unwrap().id;

    let deps = indexer.index.file_dependencies.get(&app_id).expect("Dependencies expected");

    assert!(deps.contains(&logic_id), "Should resolve 'nested.deep.logic' to 'src/nested/deep/logic.py' via fuzzy matching");
}