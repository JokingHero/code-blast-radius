mod common;
use common::TestWorkspace;
use rfc_engine::indexer::Indexer;

#[test]
fn test_const_propagation_import() {
    let workspace = TestWorkspace::new();

    // 1. Define a constant for a path
    workspace.create_file("paths.ts", "export const UTILS = './utils';");
    
    // 2. The file being imported
    workspace.create_file("utils.ts", "export function help() {}");

    // 3. Import using the constant (Dynamic Import logic simulation)
    // Note: Standard static imports (import x from Y) don't allow variables in JS,
    // but require() or dynamic import() do.
    workspace.create_file("main.ts", r#"
        const LIB_PATH = "./utils";
        const lib = require(LIB_PATH); 
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    let main_id = indexer.index.files.values().find(|f| f.path.contains("main.ts")).unwrap().id;
    let utils_id = indexer.index.files.values().find(|f| f.path.contains("utils.ts")).unwrap().id;

    // The indexer should have resolved LIB_PATH to "./utils" and linked the files
    let deps = indexer.index.file_dependencies.get(&main_id).expect("Should have dependencies");
    assert!(deps.contains(&utils_id), "Dependency should be resolved via constant propagation");
}

#[test]
fn test_const_propagation_shared_route() {
    let workspace = TestWorkspace::new();

    // 1. Backend defines route as literal
    workspace.create_file("server.py", r#"
        @app.route("/api/v1/login")
        def login(): pass
    "#);

    // 2. Frontend defines route as constant
    workspace.create_file("client.ts", r#"
        const LOGIN_ROUTE = "/api/v1/login";
        fetch(LOGIN_ROUTE);
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    let server_id = indexer.index.files.values().find(|f| f.path.contains("server.py")).unwrap().id;
    let client_id = indexer.index.files.values().find(|f| f.path.contains("client.ts")).unwrap().id;

    let deps = indexer.index.file_dependencies.get(&client_id).expect("Client should link to server");
    assert!(deps.contains(&server_id), "Shared route should link files even when one uses a constant");
}