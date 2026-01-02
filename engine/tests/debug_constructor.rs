use blast_radius_engine::analysis::language::{get_language, SupportedLanguage};
use tree_sitter::Parser;

#[test]
fn inspect_constructor_params() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::TypeScript);
    parser.set_language(&language).expect("Error loading TS grammar");

    let code = r#"
export class UserProfileComponent {
    constructor(private api: UserService) {}
    load() {
        this.api.list();
    }
}
"#;

    let tree = parser.parse(code, None).unwrap();
    let root = tree.root_node();

    println!("\n=== TYPESCRIPT CONSTRUCTOR AST ===");
    println!("{}", root.to_sexp());
    println!("==================================\n");
}
