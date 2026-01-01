mod common;
use common::TestWorkspace;
use rfc_engine::resolution::Indexer;
use rfc_engine::query::traversal::find_related_symbols;

use crate::common::get_calls;

#[test]
fn test_return_type_bridge_sniffing() {
    let workspace = TestWorkspace::new();

    // 1. Define a class with a method and a factory function that returns that class type
    workspace.create_file("api.ts", r#"
        class Database {
            connect() { return true; }
            query(sql: string) { return []; }
        }

        export function getDb(): Database {
            return new Database();
        }
    "#);

    // 2. Use the factory and call a method on the resulting variable
    workspace.create_file("app.ts", r#"
        import { getDb } from "./api";

        function startApp() {
            const db = getDb();
            db.connect();
        }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    // Verification Logic:
    // Searching for "connect" should ideally find "startApp" as a related symbol 
    // because "startApp" calls "db.connect()" and we know "db" is a "Database" 
    // because "getDb()" returns "Database".

    let connect_ids = indexer.lookup.symbol_map.get("connect").expect("Should find 'connect' symbol");
    let _connect_id = connect_ids[0];

    let related = find_related_symbols(&indexer.index, &indexer.lookup, &indexer.reverse_graph, "connect").unwrap();
    let names: Vec<String> = related.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();

    assert!(names.contains(&"startApp".to_string()), "startApp should be linked to connect() via return type sniffing");
    assert!(names.contains(&"Database".to_string()), "Database class should be in context for connect()");
}

#[test]
fn test_explicit_type_sniffing() {
    let workspace = TestWorkspace::new();

    // 1. Define a complex type
    workspace.create_file("types.ts", r#"
        interface FileSystem {
            readFile(path: string): string;
            writeFile(path: string, data: string): void;
        }
    "#);

    // 2. Use explicit type annotation on a variable
    workspace.create_file("logic.ts", r#"
        function sync(fs: any) {
            const disk: FileSystem = fs;
            disk.writeFile("test.txt", "hello");
        }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    let write_ids = indexer.lookup.symbol_map.get("writeFile").expect("Should find 'writeFile'");
    let write_id = write_ids[0];

    // Check if "sync" is a caller of "writeFile"
    let sync_id = indexer.lookup.symbol_map.get("sync").unwrap()[0];
    let resolutions = get_calls(&indexer.index, sync_id);
    // Note: Implicit type sniffing now adds Call edges if it finds methods, 
    // or TypeReference edges for the variable itself. 
    // Check resolve_type_sniffing logic: it adds EdgeKind::Calls for methods.
    assert!(resolutions.contains(&write_id));
}

#[test]
fn test_chained_inference_no_bloat() {
    let workspace = TestWorkspace::new();

    workspace.create_file("lib.ts", r#"
        class AuthService {
            login() { return "token"; }
        }
        export const auth = new AuthService();
    "#);

    workspace.create_file("main.ts", r#"
        import { auth } from "./lib";
        function run() {
            const service = auth;
            service.login();
        }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    // Ensure 'service' is NOT a symbol (no bloat)
    if let Some(ids) = indexer.lookup.symbol_map.get("service") {
        // It's okay if 'service' is found in other files, but not in main.ts as a top-level symbol
        let in_main = ids.iter().any(|id| indexer.index.symbols[id].name == "service");
        assert!(!in_main, "Local variable 'service' should not be a top-level symbol");
    }

    // Ensure the connection still works
    let login_id = indexer.lookup.symbol_map.get("login").unwrap()[0];
    let run_id = indexer.lookup.symbol_map.get("run").unwrap()[0];
    
    let calls = get_calls(&indexer.index, run_id);
    assert!(calls.contains(&login_id), "Should resolve call via variable assignment 'service = auth'");
}