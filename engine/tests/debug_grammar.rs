use rfc_engine::language::{get_language, SupportedLanguage};
use tree_sitter::Parser;

#[test]
fn inspect_hcl_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Hcl);
    parser.set_language(&language).expect("Error loading HCL grammar");

    let code = r#"
        resource "aws_s3_bucket" "main" {
            bucket = "my-bucket"
        }

        variable "region" {
            default = "us-east-1"
        }
    "#;

    let tree = parser.parse(code, None).unwrap();
    println!("\n--- HCL S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("------------------------\n");
}