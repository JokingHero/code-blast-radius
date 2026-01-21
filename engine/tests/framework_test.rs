mod common;
use common::TestWorkspace;

#[test]
fn test_angular_component_connection() {
    let mut ws = TestWorkspace::new();

    // 1. Define the Component (The Source)
    ws.add_file(
        "src/app/user.component.ts",
        r#"
        import { Component } from '@angular/core';
        @Component({
            selector: 'app-user-list',
            template: '...'
        })
        export class UserListComponent {}
    "#,
    );

    // 2. Define the HTML Consumer (The Target)
    ws.add_file(
        "src/app/app.component.html",
        r#"
        <div>
            <h1>Users</h1>
            <app-user-list></app-user-list>
        </div>
    "#,
    );

    ws.rebuild_index();

    // Assert that changing the TS file affects the HTML file
    ws.assert_connected("src/app/user.component.ts", "src/app/app.component.html");
}

#[test]
fn test_nestjs_controller_connection() {
    let mut ws = TestWorkspace::new();

    // 1. Define Controller (Source)
    ws.add_file(
        "src/users.controller.ts",
        r#"
        @Controller('api/v1')
        export class UserController {
            @Get('users')
            findAll() {}
        }
    "#,
    );

    // 2. Define Client (Target)
    ws.add_file(
        "src/frontend/api.ts",
        r#"
        const getUsers = () => fetch('/api/v1/users');
    "#,
    );

    ws.rebuild_index();
    ws.assert_connected("src/users.controller.ts", "src/frontend/api.ts");
}

#[test]
fn test_flask_route_connection() {
    let mut ws = TestWorkspace::new();

    ws.add_file(
        "app.py",
        r#"
        from flask import Flask
        app = Flask(__name__)

        @app.route("/login", methods=["POST"])
        def login():
            pass
    "#,
    );

    ws.add_file(
        "frontend.js",
        r#"
        axios.post('/login', { ... });
    "#,
    );

    ws.rebuild_index();
    ws.assert_connected("app.py", "frontend.js");
}

#[test]
fn test_rust_actix_connection() {
    let mut ws = TestWorkspace::new();

    ws.add_file(
        "src/main.rs",
        r#"
        #[get("/api/health")]
        async fn health_check() -> impl Responder {
            HttpResponse::Ok().body("OK")
        }
    "#,
    );

    ws.add_file(
        "scripts/check_health.sh",
        r#"
        curl http://localhost:8080/api/health
    "#,
    );

    ws.rebuild_index();
    ws.assert_connected("src/main.rs", "scripts/check_health.sh");
}

#[test]
fn test_java_spring_di() {
    let mut ws = TestWorkspace::new();

    // 1. Service Definition
    ws.add_file(
        "src/main/java/com/example/UserService.java",
        r#"
        @Service
        public class UserService {
            public void save() {}
        }
    "#,
    );

    // 2. Controller Usage (Implicit DI via field name/type matching)
    // Note: The simple walker checks literals. "UserService" literal exists here.
    ws.add_file(
        "src/main/java/com/example/UserController.java",
        r#"
        @RestController
        public class UserController {
            @Autowired
            private UserService userService;
        }
    "#,
    );

    ws.rebuild_index();
    ws.assert_connected(
        "src/main/java/com/example/UserService.java",
        "src/main/java/com/example/UserController.java",
    );
}
