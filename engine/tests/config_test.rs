mod common;
use common::TestWorkspace;
use rfc_engine::resolution::Indexer;
use rfc_engine::query::traversal::find_related_symbols;

#[test]
fn test_runtime_config_linkage() {
    let workspace = TestWorkspace::new();

    // 1. Define a configuration in a YAML file
    workspace.create_file("config.yaml", r#"
services:
  openai:
    api_key: "sk-proj-..."
    model: "gpt-4o"
db_url: "postgres://localhost:5432"
"#);

    // 2. Define code that uses one of these keys via process.env
    workspace.create_file("client.ts", r#"
        function getAiClient() {
            const key = process.env.api_key;
            return new AI(key);
        }
    "#);

    // 3. Define code that uses config.get()
    workspace.create_file("db.ts", r#"
        function connect() {
            const url = config.get("db_url");
            return database.connect(url);
        }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    // Verification A: Check if 'api_key' in YAML is a recognized config definition
    assert!(indexer.lookup.config_definitions.contains_key("api_key"), "api_key should be indexed from YAML");
    assert!(indexer.lookup.config_definitions.contains_key("db_url"), "db_url should be indexed from YAML");

    // Verification B: Semantic Cluster (The "Golden Path")
    // When searching for 'getAiClient', it should pull in the 'api_key' definition from config.yaml
    let related_ai = find_related_symbols(&indexer.index, &indexer.lookup, &indexer.reverse_graph, "getAiClient").unwrap();
    let names_ai: Vec<String> = related_ai.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();

    assert!(names_ai.contains(&"getAiClient".to_string()));
    assert!(names_ai.contains(&"api_key".to_string()), "Context for getAiClient must include its config definition");

    // Verification C: config.get() link
    let related_db = find_related_symbols(&indexer.index, &indexer.lookup, &indexer.reverse_graph, "connect").unwrap();
    let names_db: Vec<String> = related_db.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();

    assert!(names_db.contains(&"connect".to_string()));
    assert!(names_db.contains(&"db_url".to_string()), "Context for connect() must include the db_url key from YAML");
}

#[test]
fn test_config_linkage_json_and_dotenv() {
    let workspace = TestWorkspace::new();

    // Mix JSON config and Dotenv
    workspace.create_file("settings.json", r#"{ "PORT": 8080 }"#);
    workspace.create_file(".env", r#"APP_NAME=MyApp"#);

    workspace.create_file("app.ts", r#"
        function start() {
            const port = process.env.PORT;
            const name = process.env.APP_NAME;
            console.log(name, port);
        }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    let related = find_related_symbols(&indexer.index, &indexer.lookup, &indexer.reverse_graph, "start").unwrap();
    let names: Vec<String> = related.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();

    assert!(names.contains(&"PORT".to_string()), "Should link to JSON key");
    assert!(names.contains(&"APP_NAME".to_string()), "Should link to Dotenv key");
}