use rfc_engine::language::{get_language, SupportedLanguage};
use tree_sitter::Parser;

#[test]
fn debug_redux_switch_variable() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::TypeScript);
    parser.set_language(&language).expect("Error loading TS grammar");

    let code = r#"
        const LOGIN = "AUTH/LOGIN";
        switch (action.type) {
            case LOGIN: return {};
        }
    "#;
    
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- TS SWITCH VARIABLE ---");
    println!("{}", tree.root_node().to_sexp());
}

#[test]
fn debug_dispatch_variable() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::TypeScript);
    parser.set_language(&language).expect("Error loading TS grammar");

    let code = r#"
        const EVT = "CLICK";
        emitter.emit(EVT);
    "#;
    
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- TS DISPATCH VARIABLE ---");
    println!("{}", tree.root_node().to_sexp());
}