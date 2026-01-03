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