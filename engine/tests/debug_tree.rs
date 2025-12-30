use rfc_engine::{language::{SupportedLanguage, get_language}};
use tree_sitter::{Parser};

#[test]
fn debug_hcl_complex_env() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Hcl);
    parser.set_language(&language).expect("Error loading HCL grammar");

    // This is the tricky ECS pattern
    let code = r#"
        resource "aws_ecs_task_definition" "app" {
            container_definitions = <<DEFINITION
            [
              {
                "environment": [
                  {"name": "MY_BUCKET_NAME", "value": "some-value"}
                ]
              }
            ]
            DEFINITION
        }
    "#;
    
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- HCL COMPLEX S-EXPRESSION ---");
    println!("{}", tree.root_node().to_sexp());
    println!("--------------------------------\n");
}