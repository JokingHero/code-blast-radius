mod common;
use common::TestWorkspace;
use rfc_engine::indexer::Indexer;
use rfc_engine::analyzer::find_related_symbols;

#[test]
fn test_java_spring_annotation() {
    let workspace = TestWorkspace::new();

    // 1. Define the Annotation (The "Magic" Provider)
    workspace.create_file("src/annotations.java", r#"
        public @interface Transactional {}
    "#);

    // 2. Use it (The Consumer)
    workspace.create_file("src/Service.java", r#"
        import com.example.annotations.Transactional;

        public class UserService {
            @Transactional
            public void createUser() {
                // logic
            }
        }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    // 3. Search for "Transactional" -> Should find "createUser"
    let related = find_related_symbols(&indexer.index, "Transactional").unwrap();
    let names: Vec<String> = related.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();

    assert!(names.contains(&"createUser".to_string()), "Searching for annotation should find decorated methods");
    
    // 4. Search for "createUser" -> Should find "Transactional"
    let related_func = find_related_symbols(&indexer.index, "createUser").unwrap();
    let func_names: Vec<String> = related_func.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();
        
    assert!(func_names.contains(&"Transactional".to_string()), "Context for method should include its annotations");
}

#[test]
fn test_python_flask_decorator() {
    let workspace = TestWorkspace::new();

    // 1. Setup minimal Flask app structure
    workspace.create_file("app.py", r#"
        from flask import Flask, login_required

        app = Flask(__name__)

        @app.route("/dashboard")
        @login_required
        def dashboard():
            return "Secret"
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    let dashboard_id = indexer.index.symbol_map.get("dashboard").unwrap()[0];
    let sym = indexer.index.symbols.get(&dashboard_id).unwrap();

    // Check if decorators were extracted
    assert!(sym.decorators.iter().any(|d| d.contains("login_required")));
    assert!(sym.decorators.iter().any(|d| d.contains("route")));
}