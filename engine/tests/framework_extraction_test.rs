use blast_radius_engine::analysis::boundary::extract_boundary;
use blast_radius_engine::analysis::language::{get_config_by_language, SupportedLanguage};

struct HintTestCase {
    lang: SupportedLanguage,
    name: &'static str,
    code: &'static str,
    /// Expect (key, value) pairs.
    expected_hints: Vec<(&'static str, &'static str)>,
}

#[test]
fn test_framework_hint_extraction_comprehensive() {
    let cases = vec![
        // ========================================================
        // 1. JAVA (Spring Boot)
        // ========================================================
        HintTestCase {
            lang: SupportedLanguage::Java,
            name: "Spring: Class Annotations",
            code: r#"
                @Service
                @RequestMapping("/api/v1")
                public class UserService {}
            "#,
            expected_hints: vec![
                ("Service", "UserService"),
                ("RequestMapping", "/api/v1"), // Unquoted
            ],
        },
        HintTestCase {
            lang: SupportedLanguage::Java,
            name: "Spring: Method Annotations",
            code: r#"
                class C {
                    @GetMapping("/users")
                    public void getUsers() {}
                }
            "#,
            expected_hints: vec![("GetMapping", "/users")],
        },
        // ========================================================
        // 2. TYPESCRIPT / JAVASCRIPT
        // ========================================================
        HintTestCase {
            lang: SupportedLanguage::TypeScript,
            name: "NestJS: Controller & Methods",
            code: r#"
                @Controller('cats')
                export class CatsController {
                    @Get(':id')
                    findOne() {}
                }
            "#,
            expected_hints: vec![("Controller", "cats"), ("Get", ":id")],
        },
        HintTestCase {
            lang: SupportedLanguage::TypeScript,
            name: "Angular: Component",
            code: r#"
                @Component({
                    selector: 'app-hero',
                    templateUrl: './hero.component.html',
                })
                export class HeroComponent {}
            "#,
            expected_hints: vec![
                // Object literal content is captured as-is (boundary.rs only strips quotes from the string itself, but for an object node it likely returns the full text)
                // However, our query for object captures (object). boundary.rs will see the full text.
                // It attempts to trim quotes from the *start/end* of the string.
                // An object string "{...}" doesn't start with quote, so it remains "{...}".
                (
                    "Component",
                    "{ selector: 'app-hero', templateUrl: './hero.component.html', }",
                ),
            ],
        },
        HintTestCase {
            lang: SupportedLanguage::TypeScript,
            name: "Marker Decorator",
            code: r#"
                @Injectable()
                class Service {}
            "#,
            expected_hints: vec![("Injectable", "")],
        },
        HintTestCase {
            lang: SupportedLanguage::TypeScript,
            name: "Express: Routes",
            code: r#"
                app.get('/api/users', (req, res) => {});
            "#,
            expected_hints: vec![("get", "/api/users")],
        },
        HintTestCase {
            lang: SupportedLanguage::TypeScript,
            name: "Redux: Dispatch",
            code: r#"
                store.dispatch({ type: 'LOGIN_SUCCESS' });
            "#,
            expected_hints: vec![("dispatch", "LOGIN_SUCCESS")],
        },
        // ========================================================
        // 3. PYTHON
        // ========================================================
        HintTestCase {
            lang: SupportedLanguage::Python,
            name: "Flask: Route",
            code: r#"
                @app.route("/login")
                def login(): pass
            "#,
            expected_hints: vec![("route", "/login")],
        },
        HintTestCase {
            lang: SupportedLanguage::Python,
            name: "FastAPI: Methods",
            code: r#"
                @app.get("/items/{item_id}")
                def read_item(): pass
            "#,
            expected_hints: vec![("get", "/items/{item_id}")],
        },
        // ========================================================
        // 4. RUST
        // ========================================================
        HintTestCase {
            lang: SupportedLanguage::Rust,
            name: "Actix: Attributes",
            code: r#"
                #[get("/api/health")]
                async fn health() {}
            "#,
            expected_hints: vec![("get", "/api/health")],
        },
        HintTestCase {
            lang: SupportedLanguage::Rust,
            name: "Axum: Route",
            code: r#"
                let app = Router::new().route("/users", get(users_handler));
            "#,
            expected_hints: vec![("route", "/users")],
        },
        // ========================================================
        // 5. GO
        // ========================================================
        HintTestCase {
            lang: SupportedLanguage::Go,
            name: "Gin: Methods",
            code: r#"
                r.GET("/ping", handler)
            "#,
            expected_hints: vec![("GET", "/ping")],
        },
        // ========================================================
        // 6. PHP
        // ========================================================
        HintTestCase {
            lang: SupportedLanguage::Php,
            name: "Laravel: Route",
            code: r#"
                <?php
                Route::get('/user', 'C@m');
            "#,
            expected_hints: vec![("get", "/user")],
        },
        HintTestCase {
            lang: SupportedLanguage::Php,
            name: "Symfony: Attribute",
            code: r#"
                <?php
                #[Route('/api')]
                class ApiController {}
            "#,
            expected_hints: vec![("Route", "/api")],
        },
        // ========================================================
        // 7. C#
        // ========================================================
        HintTestCase {
            lang: SupportedLanguage::CSharp,
            name: "ASP.NET: Route",
            code: r#"
                [Route("api/[controller]")]
                public class UsersController {}
            "#,
            expected_hints: vec![("Route", "api/[controller]")],
        },
        // ========================================================
        // 8. RUBY
        // ========================================================
        HintTestCase {
            lang: SupportedLanguage::Ruby,
            name: "Rails: Routes",
            code: r#"
                get '/login', to: 'sessions#new'
                resources :photos
            "#,
            expected_hints: vec![("get", "/login"), ("resources", ":photos")],
        },
        // ========================================================
        // 9. C++
        // ========================================================
        HintTestCase {
            lang: SupportedLanguage::Cpp,
            name: "Crow: Route",
            code: r#"
                CROW_ROUTE(app, "/hello")([](){});
            "#,
            expected_hints: vec![("CROW_ROUTE", "/hello")],
        },
    ];

    let mut failures = Vec::new();

    for case in cases {
        let config = get_config_by_language(case.lang)
            .expect(&format!("Config not found for {:?}", case.lang));

        let boundary =
            extract_boundary("test", case.code, config, [0; 32]).expect("Extraction failed");

        let found_hints = &boundary.framework_hints;

        for (exp_key, exp_val_part) in &case.expected_hints {
            let found = found_hints.iter().find(|h| h.key == *exp_key);

            if let Some(h) = found {
                let clean_found: String = h.value.chars().filter(|c| !c.is_whitespace()).collect();
                let clean_exp: String = exp_val_part
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect();

                if !clean_found.contains(&clean_exp) {
                    failures.push(format!(
                        "[{}] Key '{}' found, but value mismatch.\n   Expected part: '{}'\n   Actual full:   '{}'",
                        case.name, exp_key, exp_val_part, h.value
                    ));
                }
            } else {
                failures.push(format!(
                    "[{}] Key '{}' not found in hints.\n   Found keys: {:?}",
                    case.name,
                    exp_key,
                    found_hints.iter().map(|h| &h.key).collect::<Vec<_>>()
                ));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "Framework Extraction Failures ({}):\n\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }
}
