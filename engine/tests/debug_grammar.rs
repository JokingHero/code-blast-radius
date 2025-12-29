use rfc_engine::language::{get_language, SupportedLanguage};
use tree_sitter::Parser;

#[test]
fn debug_rust_match_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Rust);
    parser.set_language(&language).expect("Error loading Rust grammar");

    let code = r#"
        fn main() {
            match event {
                "SYSTEM_BOOT" => { init(); },
                "A" | "B" => { handle(); }
            }
        }
    "#;

    let tree = parser.parse(code, None).unwrap();
    println!("\n--- RUST MATCH STRUCTURE ---");
    println!("{}", tree.root_node().to_sexp());
    println!("----------------------------\n");
}

#[test]
fn debug_python_signal_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Python);
    parser.set_language(&language).expect("Error loading Python grammar");

    let code = r#"
        user_signal.send("USER_REGISTERED", user=u)

        @receiver("USER_REGISTERED")
        def handler(): pass

        if action == "LOGIN": pass
    "#;

    let tree = parser.parse(code, None).unwrap();
    println!("\n--- PYTHON SIGNAL STRUCTURE ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-------------------------------\n");
}


#[test]
fn debug_rust_lazy_static() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Rust);
    parser.set_language(&language).expect("Error loading Rust grammar");

    let code = r#"
        lazy_static! {
            pub static ref GLOBAL_CONFIG: HashMap<u32, String> = HashMap::new();
        }
    "#;

    let tree = parser.parse(code, None).unwrap();
    println!("\n--- RUST LAZY_STATIC STRUCTURE ---");
    println!("{}", tree.root_node().to_sexp());
    println!("----------------------------------\n");
}

#[test]
fn debug_rust_macro_rules() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Rust);
    parser.set_language(&language).expect("Error loading Rust grammar");

    let code = r#"
        macro_rules! create_handler {
            ($name:ident) => {
                fn $name() { println!("handled"); }
            }
        }
    "#;

    let tree = parser.parse(code, None).unwrap();
    println!("\n--- RUST MACRO_RULES STRUCTURE ---");
    println!("{}", tree.root_node().to_sexp());
    println!("----------------------------------\n");
}

#[test]
fn debug_rust_heuristic_macro() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Rust);
    parser.set_language(&language).expect("Error loading Rust grammar");

    let code = r#"
        create_handler!(LoginHandler);
    "#;

    let tree = parser.parse(code, None).unwrap();
    println!("\n--- RUST HEURISTIC MACRO STRUCTURE ---");
    println!("{}", tree.root_node().to_sexp());
    println!("--------------------------------------\n");
}

#[test]
fn inspect_ts_mongoose_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::TypeScript);
    parser.set_language(&language).expect("Error loading TS grammar");
    
    // The exact line failing in factory_test.rs
    let code = r#"
        import mongoose from 'mongoose';
        const User = mongoose.model('User', { name: String });
    "#;

    let tree = parser.parse(code, None).unwrap();
    println!("\n--- TS MONGOOSE STRUCTURE ---");
    println!("{}", tree.root_node().to_sexp());
    println!("-----------------------------\n");
}