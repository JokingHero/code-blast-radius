mod common;
use std::path::Path;
use blast_radius_engine::analysis::analyze_source;
use blast_radius_engine::analysis::language::{get_language_configs, SupportedLanguage};
use blast_radius_engine::models::SymbolKind;

struct DefTestCase {
    lang: SupportedLanguage,
    name: &'static str,
    code: &'static str,
    // A list of expected (Symbol Name, Symbol Kind)
    // Order doesn't strictly matter for the test logic, but helps readability
    expected: Vec<(&'static str, SymbolKind)>,
}

#[test]
fn test_comprehensive_definitions_extraction() {
    let cases = vec![
        // ========================================================
        // RUST
        // ========================================================
        DefTestCase {
            lang: SupportedLanguage::Rust,
            name: "Rust: Functions and Structs",
            code: r#"
                fn calculate() {}
                pub struct User { id: u32 }
                impl User {
                    fn new() -> Self { User { id: 0 } }
                }
            "#,
            expected: vec![
                ("calculate", SymbolKind::Function),
                // struct_item is captured and mapped to Container via node_kind check
                ("User", SymbolKind::Container),
                // impl_item is anonymous (no name captured) - it appears as "anonymous"
                ("anonymous", SymbolKind::Container),
                ("new", SymbolKind::Function),
            ],
        },
        DefTestCase {
            lang: SupportedLanguage::Rust,
            name: "Rust: Macros",
            code: r#"
                macro_rules! my_macro { () => {} }
                lazy_static! { static ref CONFIG: i32 = 0; }
                thread_local! { static CONTEXT: i32 = 0; }
            "#,
            expected: vec![
                ("my_macro", SymbolKind::Macro),        // macro_rules! definitions are Macro
                ("CONFIG", SymbolKind::MacroGenerated), // lazy_static! macro invocations are MacroGenerated
                ("CONTEXT", SymbolKind::MacroGenerated), // thread_local! macro invocations are MacroGenerated
            ],
        },

        // ========================================================
        // TYPESCRIPT
        // ========================================================
        DefTestCase {
            lang: SupportedLanguage::TypeScript,
            name: "TS: Standard Definitions",
            code: r#"
                function add(a, b) { return a+b; }
                class UserService {
                    findAll() {}
                }
                interface IUser {}
                const helpers = {
                    format: () => {}
                };
            "#,
            expected: vec![
                ("add", SymbolKind::Function),
                ("UserService", SymbolKind::Container),
                ("findAll", SymbolKind::Function),
                ("IUser", SymbolKind::Container),
            ],
        },
        DefTestCase {
            lang: SupportedLanguage::TypeScript,
            name: "TS: Modern Framework Patterns",
            code: r#"
                // Store Factory
                const useStore = create((set) => ({}));
                // Styled Component
                const Title = styled.h1`color: red`;
                // Mongoose Model
                const User = mongoose.model('User', schema);
            "#,
            expected: vec![
                ("useStore", SymbolKind::Function), // Variable factories captured as functions
                ("Title", SymbolKind::Function),
                ("User", SymbolKind::Function),
            ],
        },

        // ========================================================
        // JAVASCRIPT
        // ========================================================
        DefTestCase {
            lang: SupportedLanguage::JavaScript,
            name: "JS: Functions and Classes",
            code: r#"
                export default function main() {}
                class App extends Component {
                    render() {}
                }
                const util = function() {};
            "#,
            expected: vec![
                ("main", SymbolKind::Function),
                ("App", SymbolKind::Container),
                ("render", SymbolKind::Function),
                ("util", SymbolKind::Function),
            ],
        },

        // ========================================================
        // PYTHON
        // ========================================================
        DefTestCase {
            lang: SupportedLanguage::Python,
            name: "Python: Defs and Classes",
            code: r#"
                def process_data(x):
                    pass
                
                class DataProcessor:
                    def __init__(self):
                        pass
            "#,
            expected: vec![
                ("process_data", SymbolKind::Function),
                ("DataProcessor", SymbolKind::Container),
                ("__init__", SymbolKind::Function),
            ],
        },
        DefTestCase {
            lang: SupportedLanguage::Python,
            name: "Python: Async",
            code: r#"
                async def fetch_api():
                    pass
            "#,
            expected: vec![
                ("fetch_api", SymbolKind::Function),
            ],
        },

        // ========================================================
        // GO
        // ========================================================
        DefTestCase {
            lang: SupportedLanguage::Go,
            name: "Go: Funcs, Methods, Structs",
            code: r#"
                package main
                
                type Config struct { Port int }
                
                func Start() {}
                
                func (c *Config) Load() {}
            "#,
            expected: vec![
                ("Config", SymbolKind::Function), // tree-sitter-go mapping currently treats type_spec as def
                ("Start", SymbolKind::Function),
                ("Load", SymbolKind::Function), // Method receiver
            ],
        },

        // ========================================================
        // JAVA
        // ========================================================
        DefTestCase {
            lang: SupportedLanguage::Java,
            name: "Java: OOP Structure",
            code: r#"
                public class MainController {
                    public void handleRequest() {}
                }
                interface Handler {}
                enum State { ON, OFF }
            "#,
            expected: vec![
                ("MainController", SymbolKind::Container),
                ("handleRequest", SymbolKind::Function),
                ("Handler", SymbolKind::Container),
                ("State", SymbolKind::Function), // Enums often map to definition or container depending on query
            ],
        },

        // ========================================================
        // C#
        // ========================================================
        DefTestCase {
            lang: SupportedLanguage::CSharp,
            name: "C#: Class and Methods",
            code: r#"
                public class Startup {
                    public void Configure() {}
                }
                interface IService {}
            "#,
            expected: vec![
                ("Startup", SymbolKind::Container),
                ("Configure", SymbolKind::Function),
                ("IService", SymbolKind::Container),
            ],
        },

        // ========================================================
        // PHP
        // ========================================================
        DefTestCase {
            lang: SupportedLanguage::Php,
            name: "PHP: Mixed",
            code: r#"
                <?php
                function globalFunc() {}
                class User {
                    public function getName() {}
                }
            "#,
            expected: vec![
                ("globalFunc", SymbolKind::Function),
                ("User", SymbolKind::Container),
                ("getName", SymbolKind::Function),
            ],
        },

        // ========================================================
        // RUBY
        // ========================================================
        DefTestCase {
            lang: SupportedLanguage::Ruby,
            name: "Ruby: Modules and Methods",
            code: r#"
                module Utils
                  class Parser
                    def parse; end
                  end
                end
            "#,
            expected: vec![
                ("Utils", SymbolKind::Container),
                ("Parser", SymbolKind::Container),
                ("parse", SymbolKind::Function),
            ],
        },

        // ========================================================
        // C / C++
        // ========================================================
        DefTestCase {
            lang: SupportedLanguage::C,
            name: "C: Structs and Funcs",
            code: r#"
                struct Point { int x; };
                int main() { return 0; }
            "#,
            expected: vec![
                ("Point", SymbolKind::Container), // struct_specifier contains "struct" -> Container
                ("main", SymbolKind::Function),
            ],
        },
        DefTestCase {
            lang: SupportedLanguage::Cpp,
            name: "C++: Classes",
            code: r#"
                class Engine {
                    public:
                        void start() {}
                };
            "#,
            expected: vec![
                ("Engine", SymbolKind::Container), // class_specifier contains "class" -> Container
                ("start", SymbolKind::Function),
            ],
        },

        // ========================================================
        // CONFIGURATION & DATA (HCL, YAML, JSON, SQL)
        // ========================================================
        DefTestCase {
            lang: SupportedLanguage::Hcl,
            name: "HCL: Terraform Resources",
            code: r#"
                resource "aws_s3_bucket" "b" { bucket = "b" }
                variable "region" {}
                data "aws_ami" "ubuntu" {}
            "#,
            expected: vec![
                ("aws_s3_bucket", SymbolKind::Resource), // Specific kind for HCL
                ("region", SymbolKind::Variable),        // Specific kind for HCL
                ("aws_ami", SymbolKind::Resource),
            ],
        },
        DefTestCase {
            lang: SupportedLanguage::Sql,
            name: "SQL: DDL",
            code: r#"
                CREATE TABLE users ( id int );
            "#,
            expected: vec![
                ("users", SymbolKind::Function), // DDL often treated as definition
            ],
        },
        DefTestCase {
            lang: SupportedLanguage::Prisma,
            name: "Prisma: Models",
            code: r#"
                model Order { id Int @id }
            "#,
            expected: vec![
                ("Order", SymbolKind::Function),
            ],
        },
        DefTestCase {
            lang: SupportedLanguage::Yaml,
            name: "YAML: Keys",
            code: r#"
                services:
                  web:
                    image: nginx
            "#,
            expected: vec![
                ("services", SymbolKind::Function),
                ("web", SymbolKind::Function),
                ("image", SymbolKind::Function),
            ],
        },
        DefTestCase {
            lang: SupportedLanguage::Json,
            name: "JSON: Keys",
            code: r#"{ "scripts": { "test": "echo 1" } }"#,
            expected: vec![
                ("scripts", SymbolKind::Function),
                ("test", SymbolKind::Function),
            ],
        },
        DefTestCase {
            lang: SupportedLanguage::Dotenv,
            name: "Dotenv: Vars",
            code: r#"API_KEY=123"#,
            expected: vec![
                ("API_KEY", SymbolKind::Function),
            ],
        },
        DefTestCase {
            lang: SupportedLanguage::Toml,
            name: "TOML: Keys",
            code: r#"
                [package]
                name = "test"
            "#,
            expected: vec![
                ("package", SymbolKind::Function),
                ("name", SymbolKind::Function),
            ],
        },

        // ========================================================
        // SCRIPTING (Bash)
        // ========================================================
        DefTestCase {
            lang: SupportedLanguage::Bash,
            name: "Bash: Functions",
            code: r#"
                function deploy() { echo "go"; }
                cleanup() { rm -rf ./dist; }
            "#,
            expected: vec![
                ("deploy", SymbolKind::Function),
                ("cleanup", SymbolKind::Function),
            ],
        },
    ];

    let configs = get_language_configs();
    let mut failures = Vec::new();

    for case in cases {
        // 1. Get Config
        let config = configs.iter().find(|c| c.lang == case.lang)
            .expect(&format!("Configuration for {:?} not found", case.lang));

        // 2. Run Analysis
        // We use a dummy path "test"
        let result = analyze_source(Path::new("test"), case.code, config);

        if let Err(e) = result {
            failures.push(format!("[{}] Analysis Failed: {}", case.name, e));
            continue;
        }

        let analysis = result.unwrap();

        // 3. Verify Expectations
        let found_symbols: Vec<(&str, SymbolKind)> = analysis.functions.iter()
            // Filter out the implicit Module definition that matches the file name/path
            // defined in definitions.rs. Usually name is "(module) test"
            .filter(|f| f.kind != SymbolKind::Module)
            .map(|f| (f.name.as_str(), f.kind))
            .collect();

        // Check if all expected symbols are present
        for (exp_name, exp_kind) in &case.expected {
            let found = found_symbols.iter().any(|(n, k)| n == exp_name && k == exp_kind);
            
            if !found {
                failures.push(format!(
                    "[{}] Missing expected definition: '{}' ({:?}).\n   Found: {:?}", 
                    case.name, exp_name, exp_kind, found_symbols
                ));
            }
        }

        // Optional: Check against extra noise?
        // For now, we only ensure what we WANT is there. 
        // We generally allow extra definitions if the parser is aggressive, 
        // unless it's completely wrong.
    }

    if !failures.is_empty() {
        panic!(
            "Definition Scope Test Failures ({}):\n\n{}", 
            failures.len(), 
            failures.join("\n\n")
        );
    }
}