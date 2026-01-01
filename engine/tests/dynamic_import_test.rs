mod common;
use common::TestWorkspace;
use rfc_engine::{models::StagingArea, resolution::{Indexer, pipeline::Pipeline}};

#[test]
fn test_ts_dynamic_import_with_variable() {
    let workspace = TestWorkspace::new();

    // 1. The Target File
    workspace.create_file("modules/heavy.ts", "export const data = 999;");

    // 2. The Importer (TypeScript) using a variable
    // This tests:
    // a) query_vals captures 'HEAVY_PATH'
    // b) query_imports captures 'import(HEAVY_PATH)' as @import.dynamic
    // c) analyzer resolves HEAVY_PATH to "./modules/heavy"
    workspace.create_file("loader.ts", r#"
        const HEAVY_PATH = "./modules/heavy";
        
        async function load() {
            const mod = await import(HEAVY_PATH);
            console.log(mod.data);
        }
    "#);

    let mut indexer = Indexer::new();
    let mut pipeline = Pipeline::new();
    pipeline.run(&mut indexer, &workspace.path);

    let loader_id = indexer.index.files.values()
        .find(|f| f.path.contains("loader.ts"))
        .expect("loader.ts not found").id;
        
    let heavy_id = indexer.index.files.values()
        .find(|f| f.path.contains("heavy.ts"))
        .expect("heavy.ts not found").id;

    let deps = indexer.index.file_dependencies.get(&loader_id)
        .expect("Loader should have dependencies");

    assert!(deps.contains(&heavy_id), "Variable-based dynamic import should link files");
}

#[test]
fn test_js_require_with_variable() {
    let workspace = TestWorkspace::new();

    workspace.create_file("lib.js", "module.exports = { val: 1 };");

    // Test CommonJS require with variable
    workspace.create_file("app.js", r#"
        const libName = "./lib";
        const myLib = require(libName);
    "#);

    let mut indexer = Indexer::new();
    let mut pipeline = Pipeline::new();
    pipeline.run(&mut indexer, &workspace.path);

    let app_id = indexer.index.files.values().find(|f| f.path.contains("app.js")).unwrap().id;
    let lib_id = indexer.index.files.values().find(|f| f.path.contains("lib.js")).unwrap().id;

    let deps = indexer.index.file_dependencies.get(&app_id).unwrap();
    assert!(deps.contains(&lib_id), "Variable-based require() should link files");
}

#[test]
fn test_python_importlib_variable() {
    let workspace = TestWorkspace::new();

    workspace.create_file("plugins/payment.py", "def pay(): pass");

    // Python test for importlib
    workspace.create_file("main.py", r#"
        import importlib
        
        # We define the module path as a string variable
        PLUGIN_NAME = "plugins.payment"
        
        def load_plugin():
            # This triggers @import.dynamic -> looks up PLUGIN_NAME -> "plugins.payment"
            mod = importlib.import_module(PLUGIN_NAME)
            mod.pay()
    "#);

    let mut indexer = Indexer::new();
    let mut pipeline = Pipeline::new();
    pipeline.run(&mut indexer, &workspace.path);

    let main_id = indexer.index.files.values().find(|f| f.path.contains("main.py")).unwrap().id;
    let plugin_id = indexer.index.files.values().find(|f| f.path.contains("payment.py")).unwrap().id;

    let deps = indexer.index.file_dependencies.get(&main_id)
        .expect("Main.py should have dependencies");

    assert!(deps.contains(&plugin_id), "Python importlib(variable) should link files");
}

#[test]
fn test_ts_dynamic_import_literal() {
    let workspace = TestWorkspace::new();

    // Simple test to ensure we didn't break standard literal dynamic imports
    workspace.create_file("utils.ts", "export const x = 1;");
    workspace.create_file("main.ts", r#"
        import('./utils').then(m => console.log(m));
    "#);

    let mut indexer = Indexer::new();
    let mut pipeline = Pipeline::new();
    pipeline.run(&mut indexer, &workspace.path);

    let main_id = indexer.index.files.values().find(|f| f.path.contains("main.ts")).unwrap().id;
    let utils_id = indexer.index.files.values().find(|f| f.path.contains("utils.ts")).unwrap().id;

    let deps = indexer.index.file_dependencies.get(&main_id).unwrap();
    assert!(deps.contains(&utils_id), "Literal dynamic import should link files");
}