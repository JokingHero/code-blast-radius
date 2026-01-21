use regex::Regex;
use crate::analysis::language::SupportedLanguage;
use crate::inference::frameworks::{FrameworkManager, FrameworkSpec, ConceptRule, ConceptType};

/// Populates the FrameworkManager with all known framework definitions.
pub fn register_all(manager: &mut FrameworkManager) {
    register_angular(manager);
    register_nestjs(manager);
    register_flask(manager);
    register_fastapi(manager);
    register_spring_boot(manager);
    register_rust_actix(manager);
    register_react_redux(manager);
    register_go_gin(manager);
    register_php_frameworks(manager);
    register_ruby_rails(manager);
    register_aspnet_core(manager);
    register_cpp_frameworks(manager);
}

fn register_go_gin(manager: &mut FrameworkManager) {
    manager.register(FrameworkSpec {
        name: "Gin/Echo".to_string(),
        language: SupportedLanguage::Go,
        detection_import: None, // Common names like "gin" or "echo" might appear in imports but Go imports are URLs.
        detection_suffix: Some(".go".to_string()),
        rules: vec![
            // 1. HTTP Methods
            // Hint: key="GET", value="/ping"
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "GET".to_string(),
                extraction_regex: None,
                output_template: "route:GET:{}".to_string(),
                parent_context_key: None, // Context tracking for Go is harder (variable tracing), MVP is stateless
            },
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "POST".to_string(),
                extraction_regex: None,
                output_template: "route:POST:{}".to_string(),
                parent_context_key: None,
            },
             // Add PUT, DELETE, etc.
        ],
    });
}

fn register_angular(manager: &mut FrameworkManager) {
    manager.register(FrameworkSpec {
        name: "Angular".to_string(),
        language: SupportedLanguage::TypeScript,
        detection_import: Some("@angular/core".to_string()),
        detection_suffix: Some(".ts".to_string()),
        rules: vec![
            // 1. Component Selectors -> "html:tag:app-root"
            // Hint: key="@Component", value="{ selector: 'app-root', ... }"
            ConceptRule {
                concept: ConceptType::View,
                trigger_key: "@Component".to_string(),
                // Regex: find the selector property
                extraction_regex: Some(Regex::new(r"selector\s*:\s*['`\x22]([\w-]+)['`\x22]").expect("Invalid Regex")),
                output_template: "html:tag:{}".to_string(),
                parent_context_key: None,
            },
            // 2. Injectable Services -> "di:UserService"
            // Hint: key="@Injectable", value="" (on class UserService)
            // Note: This relies on the Parser extracting the Class Name as the value if the decorator is empty
            ConceptRule {
                concept: ConceptType::DependencyProvider,
                trigger_key: "@Injectable".to_string(),
                extraction_regex: None, // Take the class name directly
                output_template: "di:{}".to_string(), 
                parent_context_key: None,
            }
        ],
    });
}

fn register_nestjs(manager: &mut FrameworkManager) {
    manager.register(FrameworkSpec {
        name: "NestJS".to_string(),
        language: SupportedLanguage::TypeScript,
        detection_import: Some("@nestjs/common".to_string()),
        detection_suffix: Some(".ts".to_string()),
        rules: vec![
            // 1. Controller Base Path -> Context
            // Hint: key="@Controller", value="api/v1"
            ConceptRule {
                concept: ConceptType::Route, // Just context, but required struct field
                trigger_key: "@Controller".to_string(),
                extraction_regex: None, // The parser gives us the string inside the parens
                output_template: "route:root:{}".to_string(), // Intermediate key
                parent_context_key: None,
            },
            // 2. GET Methods -> "route:GET:/api/v1/users"
            // Hint: key="@Get", value="users"
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "@Get".to_string(),
                extraction_regex: None,
                // {parent} comes from @Controller, {} is the @Get value
                // Note: We need to handle slash joining in the engine (format_output helper)
                output_template: "route:GET:{parent}/{}".to_string(),
                parent_context_key: Some("@Controller".to_string()),
            },
            // 3. POST Methods
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "@Post".to_string(),
                extraction_regex: None,
                output_template: "route:POST:{parent}/{}".to_string(),
                parent_context_key: Some("@Controller".to_string()),
            },
            // 4. Dependency Injection
            ConceptRule {
                concept: ConceptType::DependencyProvider,
                trigger_key: "@Injectable".to_string(),
                extraction_regex: None, 
                output_template: "di:{}".to_string(),
                parent_context_key: None,
            }
        ],
    });
}

fn register_flask(manager: &mut FrameworkManager) {
    manager.register(FrameworkSpec {
        name: "Flask".to_string(),
        language: SupportedLanguage::Python,
        detection_import: Some("flask".to_string()),
        detection_suffix: Some(".py".to_string()),
        rules: vec![
            // Hint: key="@app.route", value="/api/users"
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "@app.route".to_string(),
                extraction_regex: None, 
                // Default to GET for basic blasting, Flask allows methods=["POST"] args 
                // which requires more complex parsing later.
                output_template: "route:GET:{}".to_string(), 
                parent_context_key: None,
            }
        ],
    });
}

fn register_fastapi(manager: &mut FrameworkManager) {
    manager.register(FrameworkSpec {
        name: "FastAPI".to_string(),
        language: SupportedLanguage::Python,
        detection_import: Some("fastapi".to_string()),
        detection_suffix: Some(".py".to_string()),
        rules: vec![
            // Hint: key="@app.get", value="/items/{id}"
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "@app.get".to_string(),
                extraction_regex: None,
                output_template: "route:GET:{}".to_string(),
                parent_context_key: None,
            },
            // Hint: key="@app.post", value="/items"
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "@app.post".to_string(),
                extraction_regex: None,
                output_template: "route:POST:{}".to_string(),
                parent_context_key: None,
            }
        ],
    });
}

fn register_spring_boot(manager: &mut FrameworkManager) {
    manager.register(FrameworkSpec {
        name: "Spring Boot".to_string(),
        language: SupportedLanguage::Java,
        detection_import: Some("org.springframework".to_string()),
        detection_suffix: Some(".java".to_string()),
        rules: vec![
            // 1. Context: RequestMapping on Class
            // Hint: key="@RequestMapping", value="/api"
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "@RequestMapping".to_string(),
                extraction_regex: None,
                output_template: "route:root:{}".to_string(),
                parent_context_key: None,
            },
            // 2. GetMapping with Context
            // Hint: key="@GetMapping", value="/users"
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "@GetMapping".to_string(),
                extraction_regex: None,
                output_template: "route:GET:{parent}/{}".to_string(),
                parent_context_key: Some("@RequestMapping".to_string()),
            },
            // 3. GetMapping Standalone (if no class mapping)
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "@GetMapping".to_string(),
                extraction_regex: None,
                output_template: "route:GET:{}".to_string(),
                parent_context_key: None,
            },
            // 4. Service Beans
            // Hint: key="@Service", value="UserService" (Parser extracts class name)
            ConceptRule {
                concept: ConceptType::DependencyProvider,
                trigger_key: "@Service".to_string(),
                extraction_regex: None,
                output_template: "di:{}".to_string(),
                parent_context_key: None,
            },
             // 5. Component Beans
            ConceptRule {
                concept: ConceptType::DependencyProvider,
                trigger_key: "@Component".to_string(),
                extraction_regex: None,
                output_template: "di:{}".to_string(),
                parent_context_key: None,
            }
        ],
    });
}

// configs.rs
fn register_rust_actix(manager: &mut FrameworkManager) {
    manager.register(FrameworkSpec {
        name: "Actix/Rocket".to_string(),
        language: SupportedLanguage::Rust,
        detection_import: None, // Rust imports are scoped
        detection_suffix: Some(".rs".to_string()),
        rules: vec![
            // Matches: #[get("/path")] -> key="get", value="/path"
            // Note: Parser uses "get", not "actix.get" because capture is on identifier
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "get".to_string(),
                extraction_regex: None,
                output_template: "route:GET:{}".to_string(),
                parent_context_key: None,
            },
             // Add post, put, delete
        ],
    });
    
    // Axum
    manager.register(FrameworkSpec {
        name: "Axum".to_string(),
        language: SupportedLanguage::Rust,
        detection_import: None,
        detection_suffix: Some(".rs".to_string()),
        rules: vec![
             // Matches: .route("/path", ...)
             ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "route".to_string(),
                extraction_regex: None,
                output_template: "route:GET:{}".to_string(), // Defaulting to GET
                parent_context_key: None,
            },
        ]
    })
}

fn register_react_redux(manager: &mut FrameworkManager) {
    manager.register(FrameworkSpec {
        name: "Redux".to_string(),
        language: SupportedLanguage::TypeScript, // And JS
        detection_import: Some("redux".to_string()), // or react-redux or @reduxjs/toolkit
        detection_suffix: None,
        rules: vec![
            // 1. Dispatching Actions
            // Hint: key="redux.dispatch", value="USER_LOGIN"
            ConceptRule {
                concept: ConceptType::DependencyConsumer, // It relies on a reducer
                trigger_key: "redux.dispatch".to_string(),
                extraction_regex: None,
                output_template: "redux:action:{}".to_string(),
                parent_context_key: None,
            },
            // 2. Handling Actions (Reducers)
            // Hint: key="redux.handle", value="USER_LOGIN"
            ConceptRule {
                concept: ConceptType::DependencyProvider, // It provides the logic
                trigger_key: "redux.handle".to_string(),
                extraction_regex: None,
                output_template: "redux:action:{}".to_string(),
                parent_context_key: None,
            }
        ],
    });
}

fn register_php_frameworks(manager: &mut FrameworkManager) {
    // Laravel
    manager.register(FrameworkSpec {
        name: "Laravel".to_string(),
        language: SupportedLanguage::Php,
        detection_import: Some("Illuminate".to_string()),
        detection_suffix: Some(".php".to_string()),
        rules: vec![
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "get".to_string(), // Matches Route::get
                extraction_regex: None,
                output_template: "route:GET:{}".to_string(),
                parent_context_key: None,
            },
            // Add post, etc.
        ],
    });

    // Symfony
    manager.register(FrameworkSpec {
        name: "Symfony".to_string(),
        language: SupportedLanguage::Php,
        detection_import: Some("Symfony".to_string()),
        detection_suffix: Some(".php".to_string()),
        rules: vec![
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "Route".to_string(), // Matches #[Route]
                extraction_regex: None,
                output_template: "route:GET:{}".to_string(), // Defaulting to GET
                parent_context_key: None,
            }
        ],
    });
}

fn register_ruby_rails(manager: &mut FrameworkManager) {
    manager.register(FrameworkSpec {
        name: "Rails".to_string(),
        language: SupportedLanguage::Ruby,
        detection_import: Some("Rails".to_string()), 
        detection_suffix: Some(".rb".to_string()),
        rules: vec![
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "get".to_string(),
                extraction_regex: None,
                output_template: "route:GET:{}".to_string(),
                parent_context_key: None,
            },
             // Add post, etc.
             
            // 'resources' generates multiple routes (index, show, new, edit, etc.)
            // For MVP, we can treat it as a "root" route for that resource.
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "resources".to_string(),
                // Strip the colon from ":photos"
                extraction_regex: Some(Regex::new(r":(\w+)").unwrap()),
                output_template: "route:GET:/{}".to_string(), 
                parent_context_key: None,
            }
        ],
    });
}

fn register_aspnet_core(manager: &mut FrameworkManager) {
    manager.register(FrameworkSpec {
        name: "ASP.NET Core".to_string(),
        language: SupportedLanguage::CSharp,
        detection_import: Some("Microsoft.AspNetCore".to_string()),
        detection_suffix: Some(".cs".to_string()),
        rules: vec![
            // 1. Controller Base Path
            // Hint: key="Route", value="api/[controller]"
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "Route".to_string(),
                extraction_regex: None,
                output_template: "route:root:{}".to_string(),
                parent_context_key: None,
            },
            // 2. HTTP Methods
            // Hint: key="HttpGet", value="{id}"
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "HttpGet".to_string(),
                extraction_regex: None,
                // ASP.NET combines parent + child path automatically
                output_template: "route:GET:{parent}/{}".to_string(),
                parent_context_key: Some("Route".to_string()),
            },
            // Standalone HttpGet (if no parent Route)
             ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "HttpGet".to_string(),
                extraction_regex: None,
                output_template: "route:GET:{}".to_string(),
                parent_context_key: None,
            }
        ],
    });
}

fn register_cpp_frameworks(manager: &mut FrameworkManager) {
    // Crow
    manager.register(FrameworkSpec {
        name: "Crow".to_string(),
        language: SupportedLanguage::Cpp,
        detection_import: Some("crow.h".to_string()),
        detection_suffix: None,
        rules: vec![
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "CROW_ROUTE".to_string(),
                extraction_regex: None,
                output_template: "route:GET:{}".to_string(), // Crow defaults to GET
                parent_context_key: None,
            },
        ]
    });
}