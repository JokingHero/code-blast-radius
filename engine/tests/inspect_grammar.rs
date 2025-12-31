use rfc_engine::analysis::language::{ get_language, SupportedLanguage };
use tree_sitter::Parser;
#[test]
fn inspect_prisma_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Prisma);
    parser.set_language(&language).expect("Error loading Prisma grammar");
    let code =
        r#"
    model Order {
        id        Int     @id @default(autoincrement())
        product   String
    }
"#;

    let tree = parser.parse(code, None).unwrap();
    let root = tree.root_node();

    println!("\n--- PRISMA S-EXPRESSION ---");
    println!("{}", root.to_sexp());
    println!("---------------------------\n");
}
#[test]
fn inspect_sql_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Sql);
    parser.set_language(&language).expect("Error loading SQL grammar");
    let code =
        r#"
    CREATE TABLE users (
        id SERIAL PRIMARY KEY,
        email VARCHAR(255)
    );
"#;

    let tree = parser.parse(code, None).unwrap();
    let root = tree.root_node();

    println!("\n--- SQL S-EXPRESSION ---");
    println!("{}", root.to_sexp());
    println!("------------------------\n");
}
