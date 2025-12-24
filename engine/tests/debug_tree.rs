use rfc_engine::language::{get_language, SupportedLanguage};
use tree_sitter::Parser;

#[test]
fn dump_ts_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::TypeScript);
    parser.set_language(&language).expect("Error loading TypeScript grammar");

    // We specifically want to see 'interface' inheritance structure now
    let code = "interface MyInterface extends BaseInterface, OtherInterface {}";
    
    let tree = parser.parse(code, None).unwrap();
    let root = tree.root_node();
    
    println!("--- TREE S-EXPRESSION ---");
    println!("{}", root.to_sexp());
    println!("-------------------------");
}