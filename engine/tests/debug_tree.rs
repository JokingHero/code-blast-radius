use rfc_engine::{language::{SupportedLanguage, get_language}};
use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};

#[test]
fn debug_ts_zustand_and_styled() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::TypeScript);
    parser.set_language(&language).expect("Error loading TS grammar");

    let code = r#"
        // 1. Zustand (Direct Call)
        export const useStore = create((set) => ({ bears: 0 }));

        // 2. Styled (Tagged Template)
        const Title = styled.h1`color: red`;
    "#;
    
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- TS FACTORY STRUCTURES ---");
    println!("{}", tree.root_node().to_sexp());
}

#[test]
fn debug_python_flask_decorators() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Python);
    parser.set_language(&language).expect("Error loading Python grammar");

    let code = r#"
        @app.before_request
        def run_security(): pass
        
        @app.route("/")
        def index(): pass
    "#;
    
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- PYTHON DECORATOR STRUCTURES ---");
    println!("{}", tree.root_node().to_sexp());
}

#[test]
fn debug_ts_query_execution_factories() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::TypeScript);
    parser.set_language(&language).expect("Error loading TS grammar");

    let code = r#"
        const Title = styled.h1`color: red`;
    "#;
    let tree = parser.parse(code, None).unwrap();
    
    // UPDATED: Use call_expression with template_string arguments
    // matches: styled.h1`...`
    let query_str = r#"
        (variable_declarator
            name: (identifier) @function.name
            value: (call_expression
                function: (member_expression
                    object: (identifier) @obj
                    property: (property_identifier)
                )
                arguments: (template_string)
            )
            (#eq? @obj "styled")
        ) @function.definition
    "#;

    let query = Query::new(&language, query_str).expect("Query compilation failed");
    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(&query, tree.root_node(), code.as_bytes());
    
    let count = matches.count();
    println!("Tagged Template Matches: {}", count);
    assert!(count > 0, "Should match styled components using call_expression pattern");
}