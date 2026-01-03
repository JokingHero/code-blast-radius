use blast_radius_engine::analysis::analyze_source;
use blast_radius_engine::analysis::language::{get_language_configs, SupportedLanguage};
use std::path::Path;

struct TestCase {
    lang: SupportedLanguage,
    name: &'static str,
    code: &'static str,
    symbol_name: &'static str,
    // If None, we expect body_start to be None.
    // If Some, we expect the code at body_start to begin with this string.
    expected_body_start: Option<&'static str>,
}

#[test]
fn test_body_capture_across_languages() {
    let cases = vec![
        // --- RUST ---
        TestCase {
            lang: SupportedLanguage::Rust,
            name: "Rust Function",
            code: "fn main() { println!(\"hello\"); }",
            symbol_name: "main",
            expected_body_start: Some("{"),
        },
        TestCase {
            lang: SupportedLanguage::Rust,
            name: "Rust Impl",
            code: "impl Foo { fn bar() {} }",
            symbol_name: "anonymous", 
            expected_body_start: Some("{"), 
        },
        // --- TYPESCRIPT ---
        TestCase {
            lang: SupportedLanguage::TypeScript,
            name: "TS Function",
            code: "function add(a: number) { return a + 1; }",
            symbol_name: "add",
            expected_body_start: Some("{"),
        },
        TestCase {
            lang: SupportedLanguage::TypeScript,
            name: "TS Arrow Function",
            code: "const add = (a) => a + 1;",
            symbol_name: "add",
            expected_body_start: Some("a + 1"), // The expression body
        },
        TestCase {
            lang: SupportedLanguage::TypeScript,
            name: "TS Arrow Block",
            code: "const add = (a) => { return a + 1; };",
            symbol_name: "add",
            expected_body_start: Some("{"),
        },
        TestCase {
            lang: SupportedLanguage::TypeScript,
            name: "TS Class",
            code: "class User { id: number; }",
            symbol_name: "User",
            expected_body_start: Some("{"),
        },
        TestCase {
            lang: SupportedLanguage::TypeScript,
            name: "TS Factory (Zustand)",
            code: "const useStore = create((set) => ({ bears: 0 }));",
            symbol_name: "useStore",
            expected_body_start: Some("((set) => ({ bears: 0 }))"), // Captures arguments
        },

        // --- JAVASCRIPT ---
        TestCase {
            lang: SupportedLanguage::JavaScript,
            name: "JS Generator",
            code: "function* gen() { yield 1; }",
            symbol_name: "gen",
            expected_body_start: Some("{"),
        },
        TestCase {
            lang: SupportedLanguage::JavaScript,
            name: "JS Class",
            code: "class App { constructor() {} }",
            symbol_name: "App",
            expected_body_start: Some("{"),
        },

        // --- PYTHON ---
        TestCase {
            lang: SupportedLanguage::Python,
            name: "Python Function",
            code: "def foo():\n    pass",
            symbol_name: "foo",
            expected_body_start: Some("pass"), 
        },
        TestCase {
            lang: SupportedLanguage::Python,
            name: "Python Class",
            code: "class User:\n    def __init__(self): pass",
            symbol_name: "User",
            expected_body_start: Some("def __init__"),
        },

        // --- GO ---
        TestCase {
            lang: SupportedLanguage::Go,
            name: "Go Function",
            code: "package main\nfunc main() { fmt.Println(\"hi\") }",
            symbol_name: "main",
            expected_body_start: Some("{"),
        },
        TestCase {
            lang: SupportedLanguage::Go,
            name: "Go Struct",
            code: "package main\ntype User struct { ID int }",
            symbol_name: "User",
            expected_body_start: Some("{"),
        },

        // --- JAVA ---
        TestCase {
            lang: SupportedLanguage::Java,
            name: "Java Class",
            code: "public class User { int id; }",
            symbol_name: "User",
            expected_body_start: Some("{"),
        },
        TestCase {
            lang: SupportedLanguage::Java,
            name: "Java Method",
            code: "class A { void foo() { return; } }",
            symbol_name: "foo",
            expected_body_start: Some("{"),
        },

        // --- C ---
        TestCase {
            lang: SupportedLanguage::C,
            name: "C Function",
            code: "int main() { return 0; }",
            symbol_name: "main",
            expected_body_start: Some("{"),
        },
        TestCase {
            lang: SupportedLanguage::C,
            name: "C Struct",
            code: "struct Point { int x; int y; };",
            symbol_name: "Point",
            expected_body_start: Some("{"),
        },

        // --- C++ ---
        TestCase {
            lang: SupportedLanguage::Cpp,
            name: "Cpp Class",
            code: "class MyClass { public: int x; };",
            symbol_name: "MyClass",
            expected_body_start: Some("{"),
        },

        // --- PHP ---
        TestCase {
            lang: SupportedLanguage::Php,
            name: "PHP Function",
            code: "<?php function foo() { echo 'hi'; }",
            symbol_name: "foo",
            expected_body_start: Some("{"),
        },

        // --- RUBY ---
        TestCase {
            lang: SupportedLanguage::Ruby,
            name: "Ruby Method",
            code: "def foo\n  puts 'hi'\nend",
            symbol_name: "foo",
            // In Ruby tree-sitter, the body node for 'method' is often the list of statements 
            // inside, or sometimes null if empty. 
            // Note: Ruby definition usually covers the whole 'def...end'.
            // The logic we added captures `(_)?` as body. 
            // It might capture the first statement.
            expected_body_start: Some("puts"), 
        },

        // --- HCL (Terraform) ---
        TestCase {
            lang: SupportedLanguage::Hcl,
            name: "Terraform Resource",
            code: "resource \"aws_s3_bucket\" \"b\" { bucket = \"my-bucket\" }",
            symbol_name: "aws_s3_bucket", 
            expected_body_start: Some("bucket"),
        },

        // --- PRISMA ---
        TestCase {
            lang: SupportedLanguage::Prisma,
            name: "Prisma Model",
            code: "model User { id Int @id }",
            symbol_name: "User",
            expected_body_start: Some("{"),
        },

        // --- SQL ---
        TestCase {
            lang: SupportedLanguage::Sql,
            name: "SQL Create Table",
            code: "CREATE TABLE users ( id INT );",
            symbol_name: "users",
            expected_body_start: Some("("), // Column defs start with (
        },

        // --- DOTENV ---
        TestCase {
            lang: SupportedLanguage::Dotenv,
            name: "Dotenv Var",
            code: "API_KEY=12345",
            symbol_name: "API_KEY",
            expected_body_start: Some("12345"),
        },
        
        // --- JSON ---
        TestCase {
            lang: SupportedLanguage::Json,
            name: "JSON Key",
            code: "{ \"myKey\": { \"nested\": true } }",
            symbol_name: "myKey",
            expected_body_start: Some("{"),
        },

        // --- TOML ---
        TestCase {
            lang: SupportedLanguage::Toml,
            name: "TOML Pair",
            code: "key = \"value\"",
            symbol_name: "key",
            expected_body_start: Some("\"value\""),
        },
        
        // --- YAML ---
        TestCase {
            lang: SupportedLanguage::Yaml,
            name: "YAML Pair",
            code: "key: value",
            symbol_name: "key",
            expected_body_start: Some("value"),
        },
    ];

    let configs = get_language_configs();
    println!("DEBUG: Loaded Language Configs: {:?}", 
        configs.iter().map(|c| c.lang).collect::<Vec<_>>()
    );
    let mut failures = Vec::new();

    for case in cases {
        println!("Testing case: {} ({:?})", case.name, case.lang);

        // Find config
        let config = configs.iter().find(|c| c.lang == case.lang)
            .expect(&format!("Config not found for {:?}", case.lang));

        // Analyze
        let result = analyze_source(Path::new("test"), case.code, config);
        
        if let Err(e) = result {
            failures.push(format!("[{}] Analysis failed: {}", case.name, e));
            continue;
        }

        let analysis = result.unwrap();

        // Find Symbol
        let symbol = analysis.functions.iter().find(|f| f.name == case.symbol_name);

        if symbol.is_none() {
            failures.push(format!("[{}] Symbol '{}' not found", case.name, case.symbol_name));
            // Debug print found symbols
            let found_names: Vec<_> = analysis.functions.iter().map(|f| f.name.clone()).collect();
            println!("   > Found: {:?}", found_names);
            continue;
        }

        let sym = symbol.unwrap();

        // Verify Range
        if sym.range_start >= sym.range_end {
            failures.push(format!("[{}] Invalid range: {}..{}", case.name, sym.range_start, sym.range_end));
        }

        // Verify Body Start
        match (sym.body_start, case.expected_body_start) {
            (Some(actual_idx), Some(expected_str)) => {
                // Check bounds
                if actual_idx < sym.range_start || actual_idx >= sym.range_end {
                     failures.push(format!("[{}] body_start {} is outside range {}..{}", 
                        case.name, actual_idx, sym.range_start, sym.range_end));
                } else {
                    // Check content
                    let actual_slice = &case.code[actual_idx..];
                    if !actual_slice.trim().starts_with(expected_str) {
                         failures.push(format!("[{}] body content mismatch.\n   Expected start: '{}'\n   Actual start:   '{}...'", 
                            case.name, expected_str, &actual_slice.chars().take(20).collect::<String>()));
                    }
                }
            },
            (None, Some(_)) => {
                failures.push(format!("[{}] Expected body_start, but got None", case.name));
            },
            (Some(_), None) => {
                failures.push(format!("[{}] Expected None, but got Some", case.name));
            },
            (None, None) => {
                // OK
            }
        }
    }

    if !failures.is_empty() {
        panic!("Body capture test failures:\n\n{}", failures.join("\n"));
    }
}