use blast_radius_engine::analysis::boundary::extract_boundary;
use blast_radius_engine::analysis::language::{get_config_by_language, SupportedLanguage};
use std::collections::HashSet;
struct RefTestCase {
    lang: SupportedLanguage,
    name: &'static str,
    code: &'static str,
    /// Symbols that MUST appear in the references list (usages)
    expected_refs: Vec<&'static str>,
    /// Symbols that MUST NOT appear (definitions, too short, or keywords)
    should_not_contain: Vec<&'static str>,
}
#[test]
fn test_reference_extraction_and_filtering() {
    let cases = vec![
        RefTestCase {
            lang: SupportedLanguage::TypeScript,
            name: "TS: Function Calls and Types",
            code: r#"
function processUser(user: UserDTO) {
const dbResult = database.save(user);
logger.info("Saved");
return dbResult;
}
"#,
            expected_refs: vec![
                "UserDTO",  // Type reference
                "database", // Object access
                "save",     // Method call
                "logger",   // Object access
                "info",     // Method call
                "dbResult", // Variable usage
            ],
            should_not_contain: vec![
                "processUser", // This is the DEFINITION, so it should be removed from refs
                "u",           // Too short (if it existed)
            ],
        },
        RefTestCase {
            lang: SupportedLanguage::JavaScript,
            name: "JS: Instantiation and React Patterns",
            code: r#"
const App = () => {
const [state, setState] = useState(null);
return <UserProfile user={currentUser} />;
}
"#,
            expected_refs: vec![
                "useState",    // Hook call
                "UserProfile", // JSX Component usage
                "currentUser", // Prop usage
                "setState",    // Destructured usage
            ],
            should_not_contain: vec![
                "App", // Definition
            ],
        },
        RefTestCase {
            lang: SupportedLanguage::Rust,
            name: "Rust: Structs, Traits, and Macros",
            code: r#"
            fn handle_request(req: Request) -> Result<Response> {
                let mut db = Database::connect()?;
                println!("Handling...");
                let val = vec![1, 2, 3];
                db.query(val)
            }
        "#,
            expected_refs: vec![
                "Request",  // Type
                "Result",   // Type
                "Response", // Type
                "Database", // Struct usage
                "connect",  // Static method
                "println",  // Macro
                "vec",      // Macro
                "query",    // Method
            ],
            should_not_contain: vec![
                "handle_request", // Definition
                "fn",             // Keyword (should be ignored by grammar queries)
                "let",            // Keyword
                "mut",            // Keyword
            ],
        },
        RefTestCase {
            lang: SupportedLanguage::Python,
            name: "Python: Decorators and Calls",
            code: r#"
            @app.route("/index")
            def index():
                data = service.get_items()
                return jsonify(data)
        "#,
            expected_refs: vec![
                "app",       // Decorator object
                "route",     // Decorator method
                "service",   // Global/Import usage
                "get_items", // Method call
                "jsonify",   // Function call
            ],
            should_not_contain: vec![
                "index", // Definition
                "def",   // Keyword
            ],
        },
        RefTestCase {
            lang: SupportedLanguage::Java,
            name: "Java: Generic Types and Instantiation",
            code: r#"
            public class Manager {
                public List<String> getNames() {
                    ArrayList<String> list = new ArrayList<>();
                    list.add("test");
                    return list;
                }
            }
        "#,
            expected_refs: vec![
                "List",      // Type
                "String",    // Type
                "ArrayList", // Type/Constructor
                "add",       // Method
            ],
            should_not_contain: vec![
                "Manager",  // Definition (Class)
                "getNames", // Definition (Method)
            ],
        },
        RefTestCase {
            lang: SupportedLanguage::TypeScript,
            name: "Filter: Short Variables",
            code: r#"
            function loop() {
                let i = 0;
                let x = 10;
                let y = 20;
                return x + y;
            }
        "#,
            // "x" and "y" and "i" are all < 3 chars, so they should be filtered out
            // by the heuristic in boundary.rs (extract_boundary)
            expected_refs: vec![],
            should_not_contain: vec!["i", "x", "y", "loop"],
        },
        // 6. GO
        RefTestCase {
            lang: SupportedLanguage::Go,
            name: "Go: Packages and Methods",
            code: r#"
                func handleRequest(w http.ResponseWriter, r *http.Request) {
                    fmt.Println("Received");
                    user := db.GetUser(r.Context())
                    json.NewEncoder(w).Encode(user)
                }
            "#,
            expected_refs: vec![
                "http",           // Package usage
                "ResponseWriter", // Type usage
                "Request",        // Type usage
                "fmt",            // Package usage
                "Println",        // Function call
                "db",             // Variable/Package usage
                "GetUser",        // Method call
                "Context",        // Method call
                "json",           // Package usage
                "NewEncoder",     // Function call
                "Encode",         // Method call
            ],
            should_not_contain: vec![
                "handleRequest", // Definition
                "func",          // Keyword
                "nil",           // Keyword (if present)
            ],
        },
        RefTestCase {
            lang: SupportedLanguage::CSharp,
            name: "C#: Attributes and LINQ",
            code: r#"
                [HttpGet]
                public IEnumerable<WeatherForecast> Get() {
                    return Enumerable.Range(1, 5).Select(index => new WeatherForecast
                    {
                        Date = DateTime.Now.AddDays(index),
                        TemperatureC = Random.Shared.Next(-20, 55)
                    });
                }
            "#,
            expected_refs: vec![
                "HttpGet",         // Attribute usage
                "IEnumerable",     // Generic Type usage
                "WeatherForecast", // Type usage
                "Enumerable",      // Class usage
                "Range",           // Static method
                "Select",          // LINQ method
                "DateTime",        // Struct usage
                "Now",             // Property usage
                "AddDays",         // Method usage
                "Random",          // Class usage
                "Shared",          // Static property
                "Next",            // Method
            ],
            should_not_contain: vec![
                "Get",    // Definition
                "public", // Keyword
                "return", // Keyword
            ],
        },
        RefTestCase {
            lang: SupportedLanguage::Ruby,
            name: "Ruby: Modules and Symbols",
            code: r#"
                class UserController < ApplicationController
                    def show
                        @user = User.find(params[:id])
                        render json: @user
                    end
                end
            "#,
            expected_refs: vec![
                "ApplicationController", // Inheritance usage
                "User",                  // Class usage
                "find",                  // Method call
                "params",                // Method/Hash usage
                "render",                // Method call
                "json",                  // Symbol key (often captured as identifier in Ruby TS)
            ],
            should_not_contain: vec![
                "UserController", // Definition
                "show",           // Definition
                "def",            // Keyword
                "end",            // Keyword
            ],
        },
        RefTestCase {
            lang: SupportedLanguage::Php,
            name: "PHP: Static Calls and Properties",
            code: r#"
                <?php
                class AuthService {
                    public function login($credentials) {
                        $user = UserRepository::findByEmail($credentials['email']);
                        if (!$user) {
                            throw new Exception("Not found");
                        }
                        return $this->tokenGenerator->create($user);
                    }
                }
            "#,
            expected_refs: vec![
                "UserRepository", // Static class usage
                "findByEmail",    // Static method
                "Exception",      // Class instantiation
                "tokenGenerator", // Property access
                "create",         // Method call
            ],
            should_not_contain: vec![
                "AuthService", // Definition
                "login",       // Definition
            ],
        },
        RefTestCase {
            lang: SupportedLanguage::Cpp,
            name: "C++: Namespace and Std",
            code: r#"
                void process_vector(std::vector<int>& vec) {
                    vec.push_back(42);
                    std::cout << "Done" << std::endl;
                }
            "#,
            expected_refs: vec![
                "std",       // Namespace
                "vector",    // Type
                "push_back", // Method
                "cout",      // Object
                "endl",      // Manipulator
            ],
            should_not_contain: vec![
                "process_vector", // Definition
                "int",            // Primitive (usually ignored or filtered)
                "void",           // Keyword
            ],
        },
        RefTestCase {
            lang: SupportedLanguage::Hcl,
            name: "HCL: Resource References",
            code: r#"
                resource "aws_instance" "web" {
                    ami           = data.aws_ami.ubuntu.id
                    instance_type = var.instance_type
                    subnet_id     = aws_subnet.main.id
                }
            "#,
            // HCL analysis is tricky; usually identifiers on the RHS are refs
            expected_refs: vec![
                "data",
                "aws_ami",
                "ubuntu",
                "id",
                "var",
                "instance_type",
                "aws_subnet",
                "main",
            ],
            should_not_contain: vec![
                "aws_instance", // Resource Type (Definition-side)
                "web",          // Resource Name (Definition-side)
            ],
        },
        RefTestCase {
            lang: SupportedLanguage::Sql,
            name: "SQL: Table and Column Refs",
            code: r#"
                SELECT username, email 
                FROM users 
                WHERE active = 1 AND role_id = 5
            "#,
            expected_refs: vec!["username", "email", "users", "active", "role_id"],
            should_not_contain: vec![
                "SELECT", "FROM", "WHERE", // Keywords
            ],
        },
        RefTestCase {
            lang: SupportedLanguage::Bash,
            name: "Bash: Command and Variable Usage",
            code: r#"
                #!/bin/bash
                echo "Deploying to $ENV"
                docker build -t my-app .
                deploy_script --token $AUTH_TOKEN
            "#,
            expected_refs: vec![
                "echo",          // Command
                "docker",        // Command
                "build",         // Argument/Subcommand
                "deploy_script", // Command
                "ENV",           // Variable usage
                "AUTH_TOKEN",    // Variable usage
            ],
            should_not_contain: vec![
                "bin", "bash", // Shebang usually ignored
            ],
        },
        RefTestCase {
            lang: SupportedLanguage::Rust,
            name: "Filter: Self-Reference / Recursion",
            code: r#"
            fn recursive_algo() {
                recursive_algo();
            }
        "#,
            expected_refs: vec![],
            should_not_contain: vec![
                "recursive_algo", // Should be removed because it is in 'defs'
            ],
        },
    ];

    let mut failures = Vec::new();

    for case in cases {
        let config = get_config_by_language(case.lang)
            .expect(&format!("Config not found for {:?}", case.lang));

        let dummy_hash = [0u8; 32];
        let result = extract_boundary("test_file", case.code, config, dummy_hash);

        if let Err(e) = result {
            failures.push(format!("[{}] Analysis Failed: {}", case.name, e));
            continue;
        }

        let boundary = result.unwrap();

        // Convert references to a Set for easy checking
        let found_refs: HashSet<String> = boundary.symbol_refs.into_iter().collect();

        // 1. Check Expected Refs (Must exist)
        for expected in &case.expected_refs {
            if !found_refs.contains(*expected) {
                failures.push(format!(
                    "[{}] Missing expected usage reference: '{}'.\n   Found: {:?}",
                    case.name, expected, found_refs
                ));
            }
        }

        // 2. Check Negative Constraints (Must NOT exist)
        for negative in &case.should_not_contain {
            if found_refs.contains(*negative) {
                failures.push(format!(
                    "[{}] Found reference that should have been ignored/removed: '{}'",
                    case.name, negative
                ));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "\n\nReference Extraction Test Failures ({}):\n========================================\n{}\n",
            failures.len(),
            failures.join("\n\n")
        );
    }
}
