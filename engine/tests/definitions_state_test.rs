use blast_radius_engine::analysis::boundary::extract_boundary;
use blast_radius_engine::analysis::language::{get_config_by_language, SupportedLanguage};
use blast_radius_engine::models::SymbolKind;

struct DefTestCase {
    lang: SupportedLanguage,
    name: &'static str,
    code: &'static str,
    expected: Vec<(&'static str, SymbolKind)>,
    // Optional: Ensure these specific names are NOT found
    should_not_contain: Vec<&'static str>,
}

#[test]
fn test_comprehensive_definitions_extraction() {
    let cases = vec![
        // ========================================================
        // RUST
        // ========================================================
        DefTestCase {
            lang: SupportedLanguage::Rust,
            name: "Rust: Standard Items",
            code: r#"
                fn calculate() {}
                pub struct User { id: u32 }
                pub enum Status { Active, Inactive }
                trait IService {}
            "#,
            expected: vec![
                ("calculate", SymbolKind::Function),
                ("User", SymbolKind::Class), // Structs often map to Class or Unknown depending on boundary.rs heuristic
                ("Status", SymbolKind::Class), // Enums often map to Class
                ("IService", SymbolKind::Interface), // Traits map to Interface (heuristic) or Unknown
            ],
            should_not_contain: vec![],
        },
        DefTestCase {
            lang: SupportedLanguage::Rust,
            name: "Rust: Macros & Heuristics",
            code: r#"
                macro_rules! my_macro { () => {} }
                thread_local! { static KEY: u32 = 0; }
                lazy_static! { static ref CACHE: Map = Map::new(); }
                create_endpoint!(GetUsers);
            "#,
            expected: vec![
                ("my_macro", SymbolKind::Function), // Macros often treat as Function or Macro
                ("KEY", SymbolKind::Function),      // Thread locals captured as defs
                ("CACHE", SymbolKind::Function),
                ("GetUsers", SymbolKind::Function), // Heuristic capture
            ],
            should_not_contain: vec![],
        },
        // ========================================================
        // TYPESCRIPT / JAVASCRIPT
        // ========================================================
        DefTestCase {
            lang: SupportedLanguage::TypeScript,
            name: "TS: Classes and Interfaces",
            code: r#"
                class UserManager implements IUser {
                    find() {}
                }
                interface IUser { id: number; }
                type UserID = string;
            "#,
            expected: vec![
                ("UserManager", SymbolKind::Class),
                ("find", SymbolKind::Function),
                ("IUser", SymbolKind::Interface),
                // "type" alias usually maps to Class or Unknown in current heuristics
                ("UserID", SymbolKind::Class),
            ],
            should_not_contain: vec![],
        },
        DefTestCase {
            lang: SupportedLanguage::TypeScript,
            name: "TS: Variable Patterns & Factories",
            code: r#"
                const sum = (a, b) => a + b;
                const logger = function() {};
                const useStore = create((set) => ({ count: 0 }));
                const User = mongoose.model('User', schema);
                const Title = styled.h1`color: red`;
            "#,
            expected: vec![
                ("sum", SymbolKind::Function),      // Variable with arrow fn
                ("logger", SymbolKind::Function),   // Variable with fn expr
                ("useStore", SymbolKind::Function), // Factory pattern
                ("User", SymbolKind::Function),     // Mongoose pattern
                ("Title", SymbolKind::Function),    // Styled components pattern
            ],
            should_not_contain: vec![],
        },
        // ========================================================
        // PYTHON
        // ========================================================
        DefTestCase {
            lang: SupportedLanguage::Python,
            name: "Python: Standard",
            code: r#"
                def my_func(): pass
                class MyClass:
                    def method(self): pass
                async def async_worker(): pass
            "#,
            expected: vec![
                ("my_func", SymbolKind::Function),
                ("MyClass", SymbolKind::Class),
                ("method", SymbolKind::Function),
                ("async_worker", SymbolKind::Function),
            ],
            should_not_contain: vec![],
        },
        // ========================================================
        // JAVA & C#
        // ========================================================
        DefTestCase {
            lang: SupportedLanguage::Java,
            name: "Java: Classes and Records",
            code: r#"
                public class AuthService {
                    public void login() {}
                }
                public interface IAuth {}
                public record UserDTO(String name) {}
            "#,
            expected: vec![
                ("AuthService", SymbolKind::Class),
                ("login", SymbolKind::Function),
                ("IAuth", SymbolKind::Interface),
                ("UserDTO", SymbolKind::Class), // Records are classes
            ],
            should_not_contain: vec![],
        },
        DefTestCase {
            lang: SupportedLanguage::CSharp,
            name: "C#: Classes and Structs",
            code: r#"
                public class PaymentProcessor {
                    public void Process() {}
                }
                public struct Money {}
                interface IPayable {}
            "#,
            expected: vec![
                ("PaymentProcessor", SymbolKind::Class),
                ("Process", SymbolKind::Function),
                ("Money", SymbolKind::Class),
                ("IPayable", SymbolKind::Interface),
            ],
            should_not_contain: vec![],
        },
        // ========================================================
        // GO
        // ========================================================
        DefTestCase {
            lang: SupportedLanguage::Go,
            name: "Go: Structs and Methods",
            code: r#"
                type Server struct {}
                func NewServer() *Server { return &Server{} }
                func (s *Server) Start() {}
            "#,
            expected: vec![
                ("Server", SymbolKind::Class),
                ("NewServer", SymbolKind::Function),
                ("Start", SymbolKind::Function),
            ],
            should_not_contain: vec![],
        },
        // ========================================================
        // RUBY & PHP
        // ========================================================
        DefTestCase {
            lang: SupportedLanguage::Ruby,
            name: "Ruby: Modules and Classes",
            code: r#"
                module Utils
                    class Calculator
                        def add(a,b); end
                        def self.info; end
                    end
                end
            "#,
            expected: vec![
                ("Utils", SymbolKind::Module),
                ("Calculator", SymbolKind::Class),
                ("add", SymbolKind::Function),
                ("info", SymbolKind::Function),
            ],
            should_not_contain: vec![],
        },
        DefTestCase {
            lang: SupportedLanguage::Php,
            name: "PHP: Classes and Functions",
            code: r#"
                <?php
                function globalFunc() {}
                class User {
                    public function getName() {}
                }
            "#,
            expected: vec![
                ("globalFunc", SymbolKind::Function),
                ("User", SymbolKind::Class),
                ("getName", SymbolKind::Function),
            ],
            should_not_contain: vec![],
        },
        // ========================================================
        // C & C++
        // ========================================================
        DefTestCase {
            lang: SupportedLanguage::Cpp,
            name: "C++: Classes and Functions",
            code: r#"
                class Manager {
                    void manage() {}
                };
                int main() { return 0; }
                void* allocate(size_t s) {}
            "#,
            expected: vec![
                ("Manager", SymbolKind::Class),
                ("manage", SymbolKind::Function),
                ("main", SymbolKind::Function),
                ("allocate", SymbolKind::Function),
            ],
            should_not_contain: vec![],
        },
        // ========================================================
        // CONFIG & INFRASTRUCTURE (HCL, SQL, PRISMA, BASH)
        // ========================================================
        DefTestCase {
            lang: SupportedLanguage::Hcl,
            name: "Terraform (HCL)",
            code: r#"
                resource "aws_s3_bucket" "my_bucket" {}
                variable "region" {}
                module "vpc" {}
            "#,
            expected: vec![
                ("my_bucket", SymbolKind::Class), // Resources map to class usually
                ("region", SymbolKind::Variable),
                ("vpc", SymbolKind::Module),
            ],
            should_not_contain: vec!["aws_s3_bucket"],
        },
        DefTestCase {
            lang: SupportedLanguage::Sql,
            name: "SQL",
            code: r#"
                CREATE TABLE users (id int);
                create table orders (id int);
            "#,
            expected: vec![("users", SymbolKind::Class), ("orders", SymbolKind::Class)],
            should_not_contain: vec![],
        },
        DefTestCase {
            lang: SupportedLanguage::Prisma,
            name: "Prisma",
            code: r#"
                model User {
                    id Int @id
                }
            "#,
            expected: vec![("User", SymbolKind::Class)],
            should_not_contain: vec![],
        },
        DefTestCase {
            lang: SupportedLanguage::Bash,
            name: "Bash",
            code: r#"
                install_deps() { echo "hi"; }
                function cleanup { rm -rf ./tmp; }
            "#,
            expected: vec![
                ("install_deps", SymbolKind::Function),
                ("cleanup", SymbolKind::Function),
            ],
            should_not_contain: vec![],
        },
        // ========================================================
        // DATA LANGUAGES (JSON, YAML, TOML)
        // ========================================================
        // Note: Data languages often map SymbolKind to Unknown because
        // the node kinds (pair, entry) don't match Class/Function heuristics.
        DefTestCase {
            lang: SupportedLanguage::Json,
            name: "JSON",
            code: r#"{ "name": "project", "version": "1.0" }"#,
            expected: vec![
                ("name", SymbolKind::Unknown),
                ("version", SymbolKind::Unknown),
            ],
            should_not_contain: vec![],
        },
        DefTestCase {
            lang: SupportedLanguage::Yaml,
            name: "YAML",
            code: r#"
                services:
                    web:
                        image: nginx
            "#,
            expected: vec![
                ("services", SymbolKind::Unknown),
                ("web", SymbolKind::Unknown),
                // "image" is nested deeply, depending on query it might not be a top-level def
            ],
            should_not_contain: vec![],
        },
        // ========================================================
        // SCIENTIFIC (R, JULIA)
        // ========================================================
        DefTestCase {
            lang: SupportedLanguage::R,
            name: "R",
            code: r#"
                calculate_mean <- function(x) { mean(x) }
            "#,
            expected: vec![("calculate_mean", SymbolKind::Function)],
            should_not_contain: vec![],
        },
        DefTestCase {
            lang: SupportedLanguage::Julia,
            name: "Julia",
            code: r#"
                function solve() end
                macro sayhello() end
            "#,
            expected: vec![
                ("solve", SymbolKind::Function),
                ("sayhello", SymbolKind::Function),
            ],
            should_not_contain: vec![],
        },
        // ========================================================
        // DART
        // ========================================================
        DefTestCase {
            lang: SupportedLanguage::Dart,
            name: "Dart: Classes and Functions",
            code: r#"
                class MyClass {
                    void myMethod() {}
                }
                void main() {}
            "#,
            expected: vec![
                ("MyClass", SymbolKind::Class),
                ("myMethod", SymbolKind::Function),
                ("main", SymbolKind::Function),
            ],
            should_not_contain: vec![],
        },
        // ========================================================
        // GDScript
        // ========================================================
        DefTestCase {
            lang: SupportedLanguage::GdScript,
            name: "GDScript: Classes and Functions",
            code: r#"
                class Player:
                    func _ready():
                        pass
            "#,
            expected: vec![
                ("Player", SymbolKind::Class),
                ("_ready", SymbolKind::Function),
            ],
            should_not_contain: vec![],
        },
        // ========================================================
        // FirebaseRules
        // ========================================================
        DefTestCase {
            lang: SupportedLanguage::FirebaseRules,
            name: "FirebaseRules: Services, Matches, and Functions",
            code: r#"
                service cloud.firestore {
                  match /users/{userId} {
                    allow read: if true;
                  }
                  
                  function isLoggedIn() {
                    return request.auth != null;
                  }
                }
            "#,
            expected: vec![
                ("cloud.firestore", SymbolKind::Class),
                ("/users/{userId}", SymbolKind::Class),
                ("isLoggedIn", SymbolKind::Function),
            ],
            should_not_contain: vec![],
        },
    ];

    let mut failures = Vec::new();

    for case in cases {
        let config = get_config_by_language(case.lang)
            .expect(&format!("Config not found for {:?}", case.lang));

        let hash = [0u8; 32];

        // We handle parse errors gracefully to allow test suite to continue
        let result = extract_boundary("test_file", case.code, config, hash);

        if let Err(e) = result {
            failures.push(format!("[{}] Analysis Failed: {}", case.name, e));
            continue;
        }

        let boundary = result.unwrap();

        // Convert found definitions to a format easy to check
        // Note: boundary.rs logic might produce Unknown for things that aren't strictly class/interface/function keywords
        let found_symbols: Vec<(String, SymbolKind)> = boundary
            .defs
            .iter()
            .map(|f| (f.name.clone(), f.kind))
            .collect();

        // 1. Check Expected
        for (exp_name, exp_kind) in &case.expected {
            // Flexible kind check: Some heuristics in boundary.rs are broad (e.g. Unknown vs Class).
            // We strictly check name, and loosely check kind if it matches strict expectation.

            let match_found = found_symbols.iter().any(|(n, k)| {
                n == exp_name && (*k == *exp_kind || *exp_kind == SymbolKind::Unknown)
            });

            if !match_found {
                // Try to find if the name exists with a different kind, for better error message
                let name_exists = found_symbols.iter().find(|(n, _)| n == exp_name);

                if let Some((_, actual_kind)) = name_exists {
                    failures.push(format!(
                        "[{}] Symbol '{}' found but kind mismatch.\n   Expected: {:?}\n   Found:    {:?}", 
                        case.name, exp_name, exp_kind, actual_kind
                    ));
                } else {
                    failures.push(format!(
                        "[{}] Missing expected definition: '{}'.\n   All Found: {:?}",
                        case.name,
                        exp_name,
                        found_symbols.iter().map(|s| &s.0).collect::<Vec<_>>()
                    ));
                }
            }
        }

        // 2. Check Negative Constraints
        for not_exp in &case.should_not_contain {
            if let Some((name, kind)) = found_symbols.iter().find(|(n, _)| n == not_exp) {
                failures.push(format!(
                    "[{}] Found symbol that should be ignored: '{}' ({:?})",
                    case.name, name, kind
                ));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "\n\nDefinition Scope Test Failures ({}):\n==================================\n{}\n",
            failures.len(),
            failures.join("\n\n")
        );
    }
}
