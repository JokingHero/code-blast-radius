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