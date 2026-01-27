use crate::analysis::language::SupportedLanguage;
use crate::inference::frameworks::{ConceptRule, ConceptType, FrameworkManager, FrameworkSpec};
use regex::Regex;

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
    register_flutter(manager);
    register_godot(manager);
}

fn register_go_gin(manager: &mut FrameworkManager) {
    manager.register(FrameworkSpec {
        name: "Gin/Echo".to_string(),
        language: SupportedLanguage::Go,
        detection_import: None,
        detection_suffix: Some(".go".to_string()),
        rules: vec![
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "GET".to_string(),
                extraction_regex: None,
                output_template: "route:GET:{}".to_string(),
                parent_context_key: None,
            },
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "POST".to_string(),
                extraction_regex: None,
                output_template: "route:POST:{}".to_string(),
                parent_context_key: None,
            },
        ],
    });
}

fn register_angular(manager: &mut FrameworkManager) {
    manager.register(FrameworkSpec {
        name: "Angular".to_string(),
        language: SupportedLanguage::TypeScript,
        // Removed import check to support partial files/tests
        detection_import: None, 
        detection_suffix: Some(".ts".to_string()),
        rules: vec![
            // Trigger: "Component" (was "@Component")
            ConceptRule {
                concept: ConceptType::View,
                trigger_key: "Component".to_string(),
                extraction_regex: Some(
                    Regex::new(r"selector\s*:\s*['`\x22]([\w-]+)['`\x22]").expect("Invalid Regex"),
                ),
                output_template: "html:tag:{}".to_string(),
                parent_context_key: None,
            },
            // Trigger: "Injectable" (was "@Injectable")
            ConceptRule {
                concept: ConceptType::DependencyProvider,
                trigger_key: "Injectable".to_string(),
                extraction_regex: None,
                output_template: "di:{}".to_string(),
                parent_context_key: None,
            },
        ],
    });
}

fn register_nestjs(manager: &mut FrameworkManager) {
    manager.register(FrameworkSpec {
        name: "NestJS".to_string(),
        language: SupportedLanguage::TypeScript,
        // Removed import check to support partial files/tests
        detection_import: None,
        detection_suffix: Some(".ts".to_string()),
        rules: vec![
            // Trigger: "Controller" (was "@Controller")
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "Controller".to_string(),
                extraction_regex: None,
                output_template: "route:root:{}".to_string(),
                parent_context_key: None,
            },
            // Trigger: "Get" (was "@Get")
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "Get".to_string(),
                extraction_regex: None,
                output_template: "route:GET:{parent}/{}".to_string(),
                parent_context_key: Some("Controller".to_string()),
            },
            // Trigger: "Post" (was "@Post")
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "Post".to_string(),
                extraction_regex: None,
                output_template: "route:POST:{parent}/{}".to_string(),
                parent_context_key: Some("Controller".to_string()),
            },
            ConceptRule {
                concept: ConceptType::DependencyProvider,
                trigger_key: "Injectable".to_string(),
                extraction_regex: None,
                output_template: "di:{}".to_string(),
                parent_context_key: None,
            },
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
            // Trigger: "route" (was "@app.route")
            // The python grammar captures the attribute name 'route'
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "route".to_string(),
                extraction_regex: None,
                output_template: "route:GET:{}".to_string(),
                parent_context_key: None,
            },
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
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "get".to_string(),
                extraction_regex: None,
                output_template: "route:GET:{}".to_string(),
                parent_context_key: None,
            },
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "post".to_string(),
                extraction_regex: None,
                output_template: "route:POST:{}".to_string(),
                parent_context_key: None,
            },
        ],
    });
}

fn register_spring_boot(manager: &mut FrameworkManager) {
    manager.register(FrameworkSpec {
        name: "Spring Boot".to_string(),
        language: SupportedLanguage::Java,
        // Removed import check to support partial files/tests
        detection_import: None,
        detection_suffix: Some(".java".to_string()),
        rules: vec![
            // Trigger: "RequestMapping" (was "@RequestMapping")
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "RequestMapping".to_string(),
                extraction_regex: None,
                output_template: "route:root:{}".to_string(),
                parent_context_key: None,
            },
            // Trigger: "GetMapping" (was "@GetMapping")
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "GetMapping".to_string(),
                extraction_regex: None,
                output_template: "route:GET:{parent}/{}".to_string(),
                parent_context_key: Some("RequestMapping".to_string()),
            },
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "GetMapping".to_string(),
                extraction_regex: None,
                output_template: "route:GET:{}".to_string(),
                parent_context_key: None,
            },
            // Trigger: "Service" (was "@Service")
            ConceptRule {
                concept: ConceptType::DependencyProvider,
                trigger_key: "Service".to_string(),
                extraction_regex: None,
                output_template: "di:{}".to_string(),
                parent_context_key: None,
            },
            // Trigger: "Component"
            ConceptRule {
                concept: ConceptType::DependencyProvider,
                trigger_key: "Component".to_string(),
                extraction_regex: None,
                output_template: "di:{}".to_string(),
                parent_context_key: None,
            },
        ],
    });
}

fn register_rust_actix(manager: &mut FrameworkManager) {
    manager.register(FrameworkSpec {
        name: "Actix/Rocket".to_string(),
        language: SupportedLanguage::Rust,
        detection_import: None,
        detection_suffix: Some(".rs".to_string()),
        rules: vec![ConceptRule {
            concept: ConceptType::Route,
            trigger_key: "get".to_string(),
            extraction_regex: None,
            output_template: "route:GET:{}".to_string(),
            parent_context_key: None,
        }],
    });

    manager.register(FrameworkSpec {
        name: "Axum".to_string(),
        language: SupportedLanguage::Rust,
        detection_import: None,
        detection_suffix: Some(".rs".to_string()),
        rules: vec![ConceptRule {
            concept: ConceptType::Route,
            trigger_key: "route".to_string(),
            extraction_regex: None,
            output_template: "route:GET:{}".to_string(),
            parent_context_key: None,
        }],
    })
}

fn register_react_redux(manager: &mut FrameworkManager) {
    manager.register(FrameworkSpec {
        name: "Redux".to_string(),
        language: SupportedLanguage::TypeScript,
        detection_import: Some("redux".to_string()),
        detection_suffix: None,
        rules: vec![
            ConceptRule {
                concept: ConceptType::DependencyConsumer,
                trigger_key: "dispatch".to_string(),
                extraction_regex: None,
                output_template: "redux:action:{}".to_string(),
                parent_context_key: None,
            },
            ConceptRule {
                concept: ConceptType::DependencyProvider,
                // Note: The js/ts parser captures this as "action.handle" but boundary.rs puts raw text
                // We need to match what boundary.rs extracts, which is the key/function name.
                // For simplicity assuming the parser output handling logic aligns.
                trigger_key: "handle".to_string(),
                extraction_regex: None,
                output_template: "redux:action:{}".to_string(),
                parent_context_key: None,
            },
        ],
    });
}

fn register_php_frameworks(manager: &mut FrameworkManager) {
    manager.register(FrameworkSpec {
        name: "Laravel".to_string(),
        language: SupportedLanguage::Php,
        detection_import: Some("Illuminate".to_string()),
        detection_suffix: Some(".php".to_string()),
        rules: vec![ConceptRule {
            concept: ConceptType::Route,
            trigger_key: "get".to_string(),
            extraction_regex: None,
            output_template: "route:GET:{}".to_string(),
            parent_context_key: None,
        }],
    });

    manager.register(FrameworkSpec {
        name: "Symfony".to_string(),
        language: SupportedLanguage::Php,
        detection_import: Some("Symfony".to_string()),
        detection_suffix: Some(".php".to_string()),
        rules: vec![ConceptRule {
            concept: ConceptType::Route,
            trigger_key: "Route".to_string(),
            extraction_regex: None,
            output_template: "route:GET:{}".to_string(),
            parent_context_key: None,
        }],
    });
}

fn register_ruby_rails(manager: &mut FrameworkManager) {
    manager.register(FrameworkSpec {
        name: "Rails".to_string(),
        language: SupportedLanguage::Ruby,
        detection_import: Some("rails".to_string()),
        detection_suffix: Some(".rb".to_string()),
        rules: vec![
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "get".to_string(),
                extraction_regex: None,
                output_template: "route:GET:{}".to_string(),
                parent_context_key: None,
            },
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "resources".to_string(),
                extraction_regex: Some(Regex::new(r":(\w+)").unwrap()),
                output_template: "route:GET:/{}".to_string(),
                parent_context_key: None,
            },
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
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "Route".to_string(),
                extraction_regex: None,
                output_template: "route:root:{}".to_string(),
                parent_context_key: None,
            },
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "HttpGet".to_string(),
                extraction_regex: None,
                output_template: "route:GET:{parent}/{}".to_string(),
                parent_context_key: Some("Route".to_string()),
            },
            ConceptRule {
                concept: ConceptType::Route,
                trigger_key: "HttpGet".to_string(),
                extraction_regex: None,
                output_template: "route:GET:{}".to_string(),
                parent_context_key: None,
            },
        ],
    });
}

fn register_cpp_frameworks(manager: &mut FrameworkManager) {
    manager.register(FrameworkSpec {
        name: "Crow".to_string(),
        language: SupportedLanguage::Cpp,
        detection_import: Some("crow.h".to_string()),
        detection_suffix: None,
        rules: vec![ConceptRule {
            concept: ConceptType::Route,
            trigger_key: "CROW_ROUTE".to_string(),
            extraction_regex: None,
            output_template: "route:GET:{}".to_string(),
            parent_context_key: None,
        }],
    });
}

fn register_flutter(manager: &mut FrameworkManager) {
    manager.register(FrameworkSpec {
        name: "Flutter".to_string(),
        language: SupportedLanguage::Dart,
        detection_import: Some("flutter".to_string()),
        detection_suffix: Some(".dart".to_string()),
        rules: vec![
            // Detect View Components
            ConceptRule {
                concept: ConceptType::View,
                trigger_key: "StatelessWidget".to_string(),
                extraction_regex: None,
                output_template: "view:{}".to_string(),
                parent_context_key: None,
            },
            ConceptRule {
                concept: ConceptType::View,
                trigger_key: "StatefulWidget".to_string(),
                extraction_regex: None,
                output_template: "view:{}".to_string(),
                parent_context_key: None,
            },
        ],
    });
}

fn register_godot(manager: &mut FrameworkManager) {
    manager.register(FrameworkSpec {
        name: "Godot".to_string(),
        language: SupportedLanguage::GdScript,
        detection_import: None, // GDScript doesn't always import, it extends
        detection_suffix: Some(".gd".to_string()),
        rules: vec![
            // Heuristic: If we see "_ready", it's likely a node script
            ConceptRule {
                concept: ConceptType::DependencyProvider,
                trigger_key: "_ready".to_string(),
                extraction_regex: None,
                output_template: "godot:node".to_string(),
                parent_context_key: None,
            },
        ],
    });
}