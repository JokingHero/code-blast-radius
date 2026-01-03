use blast_radius_engine::analysis::language::{get_language, SupportedLanguage};
use blast_radius_engine::analysis::languages;
use tree_sitter::{Parser, Query};

fn check_query_compilation(lang: SupportedLanguage, query_source: &str, name: &str) {
    let language = get_language(lang);
    match Query::new(&language, query_source) {
        Ok(_) => println!("✅ {} Query compiled successfully", name),
        Err(e) => {
            println!("❌ {} Query FAILED: {:?}", name, e);
            panic!("Query compilation failed");
        }
    }
}

#[test]
fn debug_rust_query_errors() {
    let config = languages::rust::config();
    if let Some(defs) = config.queries.defs {
        check_query_compilation(SupportedLanguage::Rust, defs, "Rust Defs");
    }
}

#[test]
fn debug_typescript_query_errors() {
    let config = languages::typescript::config();
    if let Some(defs) = config.queries.defs {
        check_query_compilation(SupportedLanguage::TypeScript, defs, "TypeScript Defs");
    }
}

#[test]
fn debug_javascript_query_errors() {
    let config = languages::javascript::config();
    if let Some(defs) = config.queries.defs {
        check_query_compilation(SupportedLanguage::JavaScript, defs, "JavaScript Defs");
    }
}

#[test]
fn debug_go_query_errors() {
    let config = languages::go::config();
    if let Some(defs) = config.queries.defs {
        check_query_compilation(SupportedLanguage::Go, defs, "Go Defs");
    }
}

#[test]
fn debug_cpp_query_errors() {
    let config = languages::cpp::config();
    if let Some(defs) = config.queries.defs {
        check_query_compilation(SupportedLanguage::Cpp, defs, "C++ Defs");
    }
}

// Keep your existing AST inspection tests below...

#[test]
fn inspect_rust_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Rust);
    parser.set_language(&language).expect("Error loading Rust grammar");
    let code = "fn main() { println!(\"hello\"); }";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- RUST FUNCTION S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_rust_impl_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Rust);
    parser.set_language(&language).expect("Error loading Rust grammar");
    let code = "impl Foo { fn bar() {} }";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- RUST IMPL S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_ts_function_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::TypeScript);
    parser.set_language(&language).expect("Error loading TypeScript grammar");
    let code = "function add(a: number) { return a + 1; }";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- TS FUNCTION S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_ts_arrow_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::TypeScript);
    parser.set_language(&language).expect("Error loading TypeScript grammar");
    let code = "const add = (a) => a + 1;";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- TS ARROW S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_ts_class_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::TypeScript);
    parser.set_language(&language).expect("Error loading TypeScript grammar");
    let code = "class User { id: number; }";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- TS CLASS S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_js_generator_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::JavaScript);
    parser.set_language(&language).expect("Error loading JavaScript grammar");
    let code = "function* gen() { yield 1; }";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- JS GENERATOR S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_js_class_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::JavaScript);
    parser.set_language(&language).expect("Error loading JavaScript grammar");
    let code = "class App { constructor() {} }";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- JS CLASS S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_python_function_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Python);
    parser.set_language(&language).expect("Error loading Python grammar");
    let code = "def foo():\n    pass";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- PYTHON FUNCTION S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_python_class_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Python);
    parser.set_language(&language).expect("Error loading Python grammar");
    let code = "class User:\n    def __init__(self): pass";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- PYTHON CLASS S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_go_function_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Go);
    parser.set_language(&language).expect("Error loading Go grammar");
    let code = "package main\nfunc main() { fmt.Println(\"hi\") }";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- GO FUNCTION S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_go_struct_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Go);
    parser.set_language(&language).expect("Error loading Go grammar");
    let code = "package main\ntype User struct { ID int }";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- GO STRUCT S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_java_class_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Java);
    parser.set_language(&language).expect("Error loading Java grammar");
    let code = "public class User { int id; }";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- JAVA CLASS S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_java_method_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Java);
    parser.set_language(&language).expect("Error loading Java grammar");
    let code = "class A { void foo() { return; } }";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- JAVA METHOD S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_c_function_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::C);
    parser.set_language(&language).expect("Error loading C grammar");
    let code = "int main() { return 0; }";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- C FUNCTION S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_c_struct_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::C);
    parser.set_language(&language).expect("Error loading C grammar");
    let code = "struct Point { int x; int y; };";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- C STRUCT S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_cpp_class_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Cpp);
    parser.set_language(&language).expect("Error loading C++ grammar");
    let code = "class MyClass { public: int x; };";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- C++ CLASS S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_ruby_method_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Ruby);
    parser.set_language(&language).expect("Error loading Ruby grammar");
    let code = "def foo\n  puts 'hi'\nend";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- RUBY METHOD S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_hcl_resource_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Hcl);
    parser.set_language(&language).expect("Error loading HCL grammar");
    let code = "resource \"aws_s3_bucket\" \"b\" { bucket = \"my-bucket\" }";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- HCL RESOURCE S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_prisma_model_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Prisma);
    parser.set_language(&language).expect("Error loading Prisma grammar");
    let code = "model User { id Int @id }";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- PRISMA MODEL S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_sql_table_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Sql);
    parser.set_language(&language).expect("Error loading SQL grammar");
    let code = "CREATE TABLE users ( id INT );";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- SQL TABLE S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_sql_with_string() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Sql);
    parser.set_language(&language).expect("Error loading SQL grammar");
    let code = "SELECT * FROM users WHERE name = 'test';";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- SQL WITH STRING S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_sql_create_statement() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Sql);
    parser.set_language(&language).expect("Error loading SQL grammar");
    let code = "CREATE TABLE users ( id INT );";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- SQL CREATE DETAILED ---");
    for child in tree.root_node().children(&mut tree.walk()) {
        println!("Child kind: {}", child.kind());
        if child.kind() == "statement" {
            for grandchild in child.children(&mut tree.walk()) {
                println!("  Grandchild kind: {}", grandchild.kind());
                for ggchild in grandchild.children(&mut tree.walk()) {
                    println!("    GGGrandchild kind: {}", ggchild.kind());
                }
            }
        }
    }
    println!("-----------------------\n");
}

#[test]
fn inspect_dotenv_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Dotenv);
    parser.set_language(&language).expect("Error loading Dotenv grammar");
    let code = "API_KEY=12345";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- DOTENV S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_json_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Json);
    parser.set_language(&language).expect("Error loading JSON grammar");
    let code = "{ \"myKey\": { \"nested\": true } }";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- JSON S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_yaml_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Yaml);
    parser.set_language(&language).expect("Error loading YAML grammar");
    let code = "key: value";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- YAML S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_php_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Php);
    parser.set_language(&language).expect("Error loading PHP grammar");
    let code = "<?php function foo() { echo 'hi'; }";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- PHP S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_toml_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Toml);
    parser.set_language(&language).expect("Error loading TOML grammar");
    let code = "key = \"value\"";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- TOML S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_ts_export_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::TypeScript);
    parser.set_language(&language).expect("Error loading TypeScript grammar");
    let code = "export * from 'module'";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- TS EXPORT WILDCARD S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_js_export_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::JavaScript);
    parser.set_language(&language).expect("Error loading JavaScript grammar");
    let code = "export * from 'module'";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- JS EXPORT WILDCARD S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_java_implements_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Java);
    parser.set_language(&language).expect("Error loading Java grammar");
    let code = "class Foo implements Bar {}";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- JAVA IMPLEMENTS S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_bash_export_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Bash);
    parser.set_language(&language).expect("Error loading Bash grammar");
    let code = "export FOO=bar";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- BASH EXPORT S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_julia_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Julia);
    parser.set_language(&language).expect("Error loading Julia grammar");
    let code = "function foo(x) return x end";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- JULIA FUNCTION S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_julia_assignment_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Julia);
    parser.set_language(&language).expect("Error loading Julia grammar");
    let code = "x = 1";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- JULIA ASSIGNMENT S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_r_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::R);
    parser.set_language(&language).expect("Error loading R grammar");
    let code = "f <- function() {}";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- R FUNCTION S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_rust_macro_definition_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Rust);
    parser.set_language(&language).expect("Error loading Rust grammar");
    let code = "macro_rules! foo { }";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- RUST MACRO_DEFINITION S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_rust_macro_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Rust);
    parser.set_language(&language).expect("Error loading Rust grammar");
    let code = "lazy_static! { static ref FOO: i32 = 42; }";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- RUST MACRO S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_go_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Go);
    parser.set_language(&language).expect("Error loading Go grammar");
    let code = "package main\ntype User struct { ID int }";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- GO S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_cpp_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Cpp);
    parser.set_language(&language).expect("Error loading Cpp grammar");
    let code = "class MyClass { public: int x; };";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- CPP S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("------------------------\n");
}

#[test]
fn inspect_rust_struct_and_impl() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Rust);
    parser.set_language(&language).expect("Error loading Rust grammar");
    let code = r#"
fn calculate() {}
pub struct User { id: u32 }
impl User {
    fn new() -> Self { User { id: 0 } }
}
"#;
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- RUST STRUCT AND IMPL ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_ruby_module_and_class() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Ruby);
    parser.set_language(&language).expect("Error loading Ruby grammar");
    let code = r#"
module Utils
  class Parser
    def parse; end
  end
end
"#;
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- RUBY MODULE AND CLASS ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_cpp_class_with_method() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Cpp);
    parser.set_language(&language).expect("Error loading Cpp grammar");
    let code = r#"
class Engine {
    public:
        void start() {}
};
"#;
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- CPP CLASS WITH METHOD ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_hcl_variable_and_data() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Hcl);
    parser.set_language(&language).expect("Error loading Hcl grammar");
    let code = r#"
resource "aws_s3_bucket" "b" { bucket = "b" }
variable "region" {}
data "aws_ami" "ubuntu" {}
"#;
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- HCL VARIABLE AND DATA ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_rust_thread_local_macro() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Rust);
    parser.set_language(&language).expect("Error loading Rust grammar");
    let code = "thread_local! { static CONTEXT: i32 = 0; }";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- RUST THREAD_LOCAL MACRO ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}

#[test]
fn inspect_rust_lazy_static_macro() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Rust);
    parser.set_language(&language).expect("Error loading Rust grammar");
    let code = "lazy_static! { static ref CONFIG: i32 = 0; }";
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- RUST LAZY_STATIC MACRO ---");
    println!("{}", tree.root_node().to_sexp());

    // Inspect token_tree children
    let root = tree.root_node();
    let code_bytes = code.as_bytes();
    if let Some(macro_invoc) = root.child(0) {
        println!("\nMacro invocation kind: {}", macro_invoc.kind());
        for i in 0..macro_invoc.child_count() {
            let child = macro_invoc.child(i).unwrap();
            println!("  Child {}: {} = '{}'", i, child.kind(), child.utf8_text(code_bytes).unwrap_or(""));
            if child.kind() == "token_tree" {
                for j in 0..child.child_count() {
                    let tc = child.child(j).unwrap();
                    println!("    TokenTree child {}: {} = '{}'", j, tc.kind(), tc.utf8_text(code_bytes).unwrap_or(""));
                }
            }
        }
    }
    println!("-----------------------\n");
}

#[test]
fn inspect_rust_macro_test_code() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Rust);
    parser.set_language(&language).expect("Error loading Rust grammar");
    let code = r#"
                macro_rules! my_macro { () => {} }
                lazy_static! { static ref CONFIG: i32 = 0; }
                thread_local! { static CONTEXT: i32 = 0; }
            "#;
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- RUST MACRO TEST CODE ---");
    println!("{}", tree.root_node().to_sexp());

    let root = tree.root_node();
    let code_bytes = code.as_bytes();
    for i in 0..root.child_count() {
        let child = root.child(i).unwrap();
        if child.kind() == "macro_invocation" {
            println!("\nMacro invocation {}: {}", i, child.utf8_text(code_bytes).unwrap_or(""));
            for j in 0..child.child_count() {
                let grandchild = child.child(j).unwrap();
                if grandchild.kind() == "token_tree" {
                    println!("  TokenTree: {}", grandchild.utf8_text(code_bytes).unwrap_or(""));
                    for k in 0..grandchild.child_count() {
                        let tc = grandchild.child(k).unwrap();
                        println!("    {}: {}", tc.kind(), tc.utf8_text(code_bytes).unwrap_or(""));
                    }
                }
            }
        }
    }
    println!("-----------------------\n");
}

#[test]
fn test_rust_macro_query() {
    use blast_radius_engine::analysis::language::{get_language, SupportedLanguage};
    use tree_sitter::{Query, StreamingIterator};

    let mut parser = tree_sitter::Parser::new();
    let language = get_language(SupportedLanguage::Rust);
    parser.set_language(&language).expect("Error loading Rust grammar");

    // Test with pattern for lazy_static - capture token_tree and inspect it
    let query_str = r#"
        (macro_invocation
            macro: (identifier) @m (#eq? @m "lazy_static")
            (token_tree) @tt
        )
    "#;

    let code = b"lazy_static! { static ref CONFIG: i32 = 0; }";
    let tree = parser.parse(code, None).unwrap();

    match Query::new(&language, query_str) {
        Ok(query) => {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, tree.root_node(), &code[..]);
            println!("\n--- LAZY_STATIC MACRO TOKEN_TREE ---");
            while let Some(mat) = matches.next() {
                for cap in mat.captures {
                    let cap_name = &query.capture_names()[cap.index as usize];
                    let text = cap.node.utf8_text(&code[..]).unwrap_or("");
                    println!("  Capture '{}' = '{}'", cap_name, text);
                    if cap_name == &"tt" && cap.node.kind() == "token_tree" {
                        println!("    TokenTree has {} children:", cap.node.child_count());
                        for j in 0..cap.node.child_count() {
                            let tc = cap.node.child(j).unwrap();
                            println!("      {}: {} = '{}'", j, tc.kind(), tc.utf8_text(&code[..]).unwrap_or(""));
                        }
                    }
                }
            }
        }
        Err(e) => println!("Query error: {:?}", e),
    }

    // Test thread_local
    let query_str2 = r#"
        (macro_invocation
            macro: (identifier) @m (#eq? @m "thread_local")
            (token_tree) @tt
        )
    "#;

    let code2 = b"thread_local! { static CONTEXT: i32 = 0; }";
    let tree2 = parser.parse(code2, None).unwrap();

    match Query::new(&language, query_str2) {
        Ok(query) => {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, tree2.root_node(), &code2[..]);
            println!("\n--- THREAD_LOCAL MACRO TOKEN_TREE ---");
            while let Some(mat) = matches.next() {
                for cap in mat.captures {
                    let cap_name = &query.capture_names()[cap.index as usize];
                    let text = cap.node.utf8_text(&code2[..]).unwrap_or("");
                    println!("  Capture '{}' = '{}'", cap_name, text);
                    if cap_name == &"tt" && cap.node.kind() == "token_tree" {
                        println!("    TokenTree has {} children:", cap.node.child_count());
                        for j in 0..cap.node.child_count() {
                            let tc = cap.node.child(j).unwrap();
                            println!("      {}: {} = '{}'", j, tc.kind(), tc.utf8_text(&code2[..]).unwrap_or(""));
                        }
                    }
                }
            }
        }
        Err(e) => println!("Query error: {:?}", e),
    }

    // Now try to match the name - third identifier for lazy_static
    let query_str3 = r#"
        (macro_invocation
            macro: (identifier) @m (#eq? @m "lazy_static")
            (token_tree
                (_)* @skip
                (identifier) @name
            )
        )
    "#;

    match Query::new(&language, query_str3) {
        Ok(query) => {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, tree.root_node(), &code[..]);
            println!("\n--- LAZY_STATIC IDENTIFIER MATCHES ---");
            while let Some(mat) = matches.next() {
                for cap in mat.captures {
                    let cap_name = &query.capture_names()[cap.index as usize];
                    let text = cap.node.utf8_text(&code[..]).unwrap_or("");
                    if cap_name == &"name" {
                        println!("  Name = '{}'", text);
                    }
                }
            }
        }
        Err(e) => println!("Query error: {:?}", e),
    }
    println!("-----------------------\n");
}

#[test]
fn inspect_cpp_method_in_class() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Cpp);
    parser.set_language(&language).expect("Error loading Cpp grammar");
    let code = r#"
class Engine {
    public:
        void start() {}
};
"#;
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- CPP METHOD IN CLASS ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------\n");
}