mod common;
use common::TestWorkspace;
use rfc_engine::resolution::{Indexer, pipeline::Pipeline};

#[test]
fn test_strategy_1_and_3_cli_path_linking() {
    let workspace = TestWorkspace::new();

    // 1. A Bash script calling a Python script via CLI argument
    // Used .json instead of .csv because .csv is not a supported language 
    // in the indexer configuration yet, so it wouldn't be indexed.
    workspace.create_file("pipeline.sh", r#"
        #!/bin/bash
        echo "Starting pipeline..."
        python3 ./scripts/data_processor.py --input raw_data.json
    "#);

    // 2. The Python script being called
    workspace.create_file("scripts/data_processor.py", "print('Processing...')");
    
    // 3. The data file being passed as an argument
    workspace.create_file("raw_data.json", r#"{ "id": 100 }"#);

    let mut indexer = Indexer::new();
    let mut pipeline = Pipeline::new();
    pipeline.run(&mut indexer, &workspace.path);

    // Helpers to get IDs
    let sh_id = indexer.index.files.values().find(|f| f.path.contains("pipeline.sh")).unwrap().id;
    let py_id = indexer.index.files.values().find(|f| f.path.contains("data_processor.py")).unwrap().id;
    let json_id = indexer.index.files.values().find(|f| f.path.contains("raw_data.json")).unwrap().id;

    // Assertions
    let deps = indexer.index.file_dependencies.get(&sh_id).expect("Pipeline.sh should have dependencies");

    assert!(deps.contains(&py_id), "Bash script should depend on Python script via literal path");
    assert!(deps.contains(&json_id), "Bash script should depend on JSON file via CLI argument literal");
}

#[test]
fn test_strategy_1_config_file_loading() {
    let workspace = TestWorkspace::new();

    // Node.js loading a JSON config
    workspace.create_file("server.js", r#"
        const fs = require('fs');
        const config = JSON.parse(fs.readFileSync('config/settings.json'));
    "#);

    workspace.create_file("config/settings.json", r#"{ "port": 8080 }"#);

    let mut indexer = Indexer::new();
    let mut pipeline = Pipeline::new();
    pipeline.run(&mut indexer, &workspace.path);

    let js_id = indexer.index.files.values().find(|f| f.path.contains("server.js")).unwrap().id;
    let json_id = indexer.index.files.values().find(|f| f.path.contains("settings.json")).unwrap().id;

    let deps = indexer.index.file_dependencies.get(&js_id).expect("Server.js should have dependencies");
    
    assert!(deps.contains(&json_id), "JavaScript file should be linked to the JSON config it reads");
}

#[test]
fn test_strategy_2_shared_routes() {
    let workspace = TestWorkspace::new();

    // 1. HTTP Route Linking (TypeScript Frontend <-> Python Backend)
    // The link is created because they both share the exact string literal "/api/v1/users/create"
    workspace.create_file("frontend/api.ts", r#"
        fetch("/api/v1/users/create", { method: "POST" });
    "#);

    workspace.create_file("backend/routes.py", r#"
        @app.route("/api/v1/users/create", methods=["POST"])
        def create_user():
            pass
    "#);

    let mut indexer = Indexer::new();
    let mut pipeline = Pipeline::new();
    pipeline.run(&mut indexer, &workspace.path);

    // --- Assertions for HTTP Route ---
    let ts_id = indexer.index.files.values().find(|f| f.path.contains("api.ts")).unwrap().id;
    let py_id = indexer.index.files.values().find(|f| f.path.contains("routes.py")).unwrap().id;
    
    let ts_deps = indexer.index.file_dependencies.get(&ts_id).expect("Frontend should link to backend");
    assert!(ts_deps.contains(&py_id), "Files sharing HTTP route literal should be linked");
}

#[test]
fn test_strategy_2_ipc_supported_langs() {
    // Re-implementation of the IPC test using JS and Rust (which are definitely supported)
    // We removed the Go test case because Go is not currently in languages/mod.rs
    let workspace = TestWorkspace::new();

    workspace.create_file("producer.js", r#"
        bus.emit("events.user.signup.completed");
    "#);

    workspace.create_file("consumer.rs", r#"
        fn main() {
            bus.subscribe("events.user.signup.completed");
        }
    "#);

    let mut indexer = Indexer::new();
    let mut pipeline = Pipeline::new();
    pipeline.run(&mut indexer, &workspace.path);

    let js_id = indexer.index.files.values().find(|f| f.path.contains("producer.js")).unwrap().id;
    let rs_id = indexer.index.files.values().find(|f| f.path.contains("consumer.rs")).unwrap().id;

    let deps = indexer.index.file_dependencies.get(&js_id).expect("Producer should link to consumer");
    assert!(deps.contains(&rs_id), "Files sharing distinct IPC literal should be linked");
}