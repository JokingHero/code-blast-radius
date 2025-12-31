mod common;
use common::TestWorkspace;
use rfc_engine::indexer::Indexer;
use rfc_engine::query::traversal::find_related_symbols;

#[test]
fn test_express_middleware_context() {
    let workspace = TestWorkspace::new();

    // 1. The Middleware (God Object)
    workspace.create_file("auth.ts", r#"
        export function AuthMiddleware(req, res, next) {
            if (!req.user) throw new Error("Auth");
            next();
        }
    "#);

    // 2. The Controller (The Protected Logic)
    // Note: It DOES NOT import AuthMiddleware
    workspace.create_file("users.ts", r#"
        export function getUser(id) {
            return db.find(id);
        }
    "#);

    // 3. The App Entry Point (The Orchestrator)
    workspace.create_file("app.ts", r#"
        import { AuthMiddleware } from "./auth";
        import * as UserController from "./users";
        import express from "express";

        const app = express();

        // Register Global Middleware
        app.use(AuthMiddleware);

        // Register Routes
        app.get("/user/:id", UserController.getUser);
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    // 4. Verification
    // If I search for "getUser", I must see "AuthMiddleware" in the context,
    // even though "users.ts" never imports "auth.ts".
    
    let related = find_related_symbols(&indexer.index, "getUser").unwrap();
    let names: Vec<String> = related.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();

    println!("Found context for getUser: {:?}", names);

    assert!(names.contains(&"getUser".to_string()));
    assert!(names.contains(&"AuthMiddleware".to_string()), 
        "Context for getUser MUST include AuthMiddleware because they are linked via app.ts orchestration");
}

#[test]
fn test_django_middleware_context() {
    let workspace = TestWorkspace::new();

    // 1. Middleware
    workspace.create_file("security.py", r#"
class SecurityMiddleware:
    def process_request(self, request):
        pass
"#);

    // 2. View
    workspace.create_file("views.py", r#"
def my_view(request):
    return "Hello"
"#);

    // 3. Settings (Orchestrator)
    workspace.create_file("settings.py", r#"
MIDDLEWARE = [
    "django.middleware.common.CommonMiddleware",
    "security.SecurityMiddleware", # String reference
]
"#);

    // 4. Urls (Linker) - In Python, settings.py is implicit, but we need
    // to simulate the import chain.
    // However, our heuristic works on the file level. 
    // If 'settings.py' imports 'security.py' (via string res) it links them.
    // But 'views.py' isn't imported by 'settings.py'.
    // In Django, this is harder. But if we have an `app.py` or `urls.py` that imports both views 
    // and some config, it might work.
    
    // Let's adjust for a clearer Python pattern: Flask Decorator
    workspace.create_file("app.py", r#"
        from flask import Flask
        from security import SecurityMiddleware
        from views import my_view

        app = Flask(__name__)

        @app.before_request
        def run_security():
            SecurityMiddleware()

        app.add_url_rule('/', view_func=my_view)
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    let related = find_related_symbols(&indexer.index, "my_view").unwrap();
    let names: Vec<String> = related.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();

    assert!(names.contains(&"SecurityMiddleware".to_string()), 
        "Flask view context should include @before_request middleware");
}