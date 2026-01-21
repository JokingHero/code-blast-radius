use blast_radius_engine::analysis::language::{get_language, SupportedLanguage};
use tree_sitter::Parser;

#[test]
fn inspect_c_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::C);
    parser
        .set_language(&language)
        .expect("Error loading C grammar");

    // A snippet containing types, structs, and fields
    let code = r#"
        struct Point { int x; };
        void main() {
            struct Point p;
            p.x = 10;
        }
    "#;

    let tree = parser.parse(code, None).unwrap();
    let root = tree.root_node();

    println!("\n--- C S-EXPRESSION ---");
    println!("{}", root.to_sexp());
    println!("----------------------\n");
}