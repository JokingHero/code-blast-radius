use blast_radius_engine::analysis::boundary::extract_boundary;
use blast_radius_engine::analysis::language::{ get_config_by_language, ALL_LANGUAGES, SupportedLanguage };
struct MalformedCase {
    lang: SupportedLanguage,
    name: &'static str,
    code: &'static str,
}

#[test]
fn test_specific_malformed_syntax_resilience() {
    let cases = vec![
        MalformedCase {
            lang: SupportedLanguage::Rust,
            name: "Rust: Unclosed Function",
            code: "fn main() { println!(",
        },
        MalformedCase {
            lang: SupportedLanguage::Rust,
            name: "Rust: Missing Identifier",
            code: "fn () {}", // Anonymous function at top level is illegal
        },
        MalformedCase {
            lang: SupportedLanguage::Rust,
            name: "Rust: Partial Macro",
            code: "macro_rules! {",
        },
        MalformedCase {
            lang: SupportedLanguage::Python,
            name: "Python: Bad Indentation",
            code: "def foo():\nprint('bad')", // Missing indentation
        },
        MalformedCase {
            lang: SupportedLanguage::Python,
            name: "Python: Trailing Def",
            code: "def ", // Trailing keyword
        },
        MalformedCase {
            lang: SupportedLanguage::Python,
            name: "Python: Unclosed Decorator",
            code: "@app.route(\n def index(): pass",
        },

        // --- JAVASCRIPT / TYPESCRIPT ---
        MalformedCase {
            lang: SupportedLanguage::TypeScript,
            name: "TS: Unclosed Template",
            code: "const x = ` unclosed template",
        },
        MalformedCase {
            lang: SupportedLanguage::JavaScript,
            name: "JS: Partial Arrow",
            code: "const foo = (a, b => ",
        },
        MalformedCase {
            lang: SupportedLanguage::TypeScript,
            name: "TS: Bad Generics",
            code: "function foo<T extends >() {}",
        },

        // --- JAVA / C# ---
        MalformedCase {
            lang: SupportedLanguage::Java,
            name: "Java: Partial Class",
            code: "public class ",
        },
        MalformedCase {
            lang: SupportedLanguage::CSharp,
            name: "C#: Unclosed Attribute",
            code: "[HttpGet\npublic void Foo() {}",
        },

        // --- HTML / JSX ---
        MalformedCase {
            lang: SupportedLanguage::Html,
            name: "HTML: Unclosed Tags",
            code: "<div><span>oops</div>",
        },

        // --- SQL ---
        MalformedCase {
            lang: SupportedLanguage::Sql,
            name: "SQL: Half Query",
            code: "CREATE TABLE ",
        },

        // --- JSON / YAML ---
        MalformedCase {
            lang: SupportedLanguage::Json,
            name: "JSON: Trailing Comma/Bad Syntax",
            code: "{ \"key\": \"value\", }",
        },
        MalformedCase {
            lang: SupportedLanguage::Yaml,
            name: "YAML: Bad Tab/Space Mix",
            code: "key:\n\tvalue", // YAML forbids tabs
        }
    ];

    let mut failures = Vec::new();

    for case in cases {
        let config = get_config_by_language(case.lang)
            .expect(&format!("Config not found for {:?}", case.lang));

        let dummy_hash = [0u8; 32];

        // The goal here is: DO NOT PANIC.
        // It is acceptable to return an Error, or to return an Ok() with partial/empty results.
        // Tree-sitter is robust, so it usually returns a tree with ERROR nodes, resulting in Ok().
        let result = std::panic::catch_unwind(|| {
            extract_boundary("bad_code", case.code, config, dummy_hash)
        });

        match result {
            Ok(inner_result) => {
                // If the engine decided to error gracefully, that's fine.
                // If it extracted partial data, that's also fine.
                if let Err(e) = inner_result {
                    // Just log, don't fail, unless we want to enforce success on error recovery
                    println!("[{}] Gracefully returned error: {}", case.name, e);
                } else {
                    let boundary = inner_result.unwrap();
                    println!("[{}] Survived. Found {} defs.", case.name, boundary.defs.len());
                }
            }
            Err(_) => {
                failures.push(format!("[{}] PANICKED during extraction!", case.name));
            }
        }
    }

    if !failures.is_empty() {
        panic!("Resilience tests failed (Panics detected):\n{}", failures.join("\n"));
    }
}

#[test]
fn test_universal_fuzzing() {
    // This test feeds pure garbage into EVERY supported language parser to ensure
    // no language configuration causes a crash on arbitrary input.
    let universal_garbage = vec![
        "",
        "   ",
        "undefined",
        "null",
        "12345",
        "!@#$%^&*()_+",
        "{{{{{{{{{{",
        "}}}}}}}}}}",
        "\"unclosed string",
        "'unclosed char",
        "// unclosed comment",
        "/* unclosed block",
        "русский текст", // UTF-8 check
        "😊 🚀" // Emoji check
    ];

    let mut panics = Vec::new();

    for &lang in ALL_LANGUAGES {
        let config = get_config_by_language(lang).unwrap();
        let lang_name = format!("{:?}", config.lang);

        for (i, garbage) in universal_garbage.iter().enumerate() {
            let result = std::panic::catch_unwind(|| {
                extract_boundary("fuzz.test", garbage, &config, [0u8; 32])
            });

            if result.is_err() {
                panics.push(format!("Language: {}, Input Case: #{} ({})", lang_name, i, garbage));
            }
        }
    }

    if !panics.is_empty() {
        panic!(
            "Universal Fuzzing Failed. The engine panicked on the following inputs:\n{}",
            panics.join("\n")
        );
    }
}

#[test]
fn test_stack_overflow_resilience() {
    // Tree-sitter can handle deep nesting, but recursive query matching could explode
    // if not careful.
    let nesting_level = 1000;
    // Create deeply nested structure: {{{{{ ... }}}}}
    let deep_code = "{".repeat(nesting_level) + &"}".repeat(nesting_level);

    // Test against a C-style language (Rust) which uses braces extensively
    let config = get_config_by_language(SupportedLanguage::Rust).unwrap();

    let result = std::panic::catch_unwind(|| {
        extract_boundary("deep.rs", &deep_code, config, [0u8; 32])
    });

    assert!(result.is_ok(), "Engine panicked on deeply nested code (Stack Overflow?)");
}