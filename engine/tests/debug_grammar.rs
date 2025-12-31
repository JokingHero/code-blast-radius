use rfc_engine::analysis::language::{ get_language, SupportedLanguage };
use tree_sitter::{ Parser, Query, QueryCursor, StreamingIterator }; // Import StreamingIterator

// --- HTML DEBUGGING ---

#[test]
fn debug_html_ast() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Html);
    parser.set_language(&language).expect("Error loading HTML grammar");

    let code = r#"
<div id="main">
    <user-profile [user]="currentUser"></user-profile>
</div>
"#;

    let tree = parser.parse(code, None).unwrap();
    println!("\n=== HTML S-EXPRESSION ===");
    println!("{}", tree.root_node().to_sexp());
    println!("=========================\n");

    // This is the query causing the panic in your main tests
    let bad_query = "(function_declaration)";
    let res = Query::new(&language, bad_query);
    match res {
        Ok(_) => println!("Query '{}' is VALID for HTML", bad_query),
        Err(e) => println!("Query '{}' is INVALID for HTML: {}", bad_query, e),
    }

    // This is the fix we proposed
    let good_query =
        r#"(attribute (attribute_name) @attr (#eq? @attr "id") (attribute_value) @function.name) @function.definition"#;
    let res = Query::new(&language, good_query);
    match res {
        Ok(_) => println!("Proposed Query is VALID for HTML"),
        Err(e) => println!("Proposed Query is INVALID for HTML: {}", e),
    }
}

// --- TYPESCRIPT DECORATOR DEBUGGING ---

#[test]
fn debug_ts_decorator_structure() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::TypeScript);
    parser.set_language(&language).expect("Error loading TS grammar");

    let code =
        r#"
        @Controller('users')
        export class UsersController {
            @Get('profile')
            getProfile() {}
        }
    "#;

    let tree = parser.parse(code, None).unwrap();
    let root = tree.root_node();

    println!("\n=== TYPESCRIPT DECORATOR AST ===");
    println!("{}", root.to_sexp());
    println!("================================\n");

    let query_str =
        r#"
        (decorator
            (call_expression
                function: (identifier) @fn
                arguments: (arguments [(string) (template_string)] @route.path)
            )
            (#match? @fn "^(Controller|Get|Post|Put|Delete|Patch)$")
        )
    "#;

    let query = Query::new(&language, query_str).expect("Query should be valid");
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, root, code.as_bytes());

    println!("\n--- Query Matches ---");
    while let Some(m) = matches.next() {
        for cap in m.captures {
            let capture_name = query.capture_names()[cap.index as usize];
            let text = cap.node.utf8_text(code.as_bytes()).unwrap();
            println!("Capture '{}': {}", capture_name, text);
        }
    }
}

#[test]
fn inspect_ts_types() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::TypeScript);
    parser.set_language(&language).expect("Error loading TS grammar");
    let code =
        r#"
    class A extends B implements C, D<T> {}
    function foo(arr: BaseEntity[]) {}
    const x = new MyClass();
    call<BaseEntity[]>();
"#;

    let tree = parser.parse(code, None).unwrap();
    let root = tree.root_node();

    println!("\n=== TYPESCRIPT TYPES AST ===");
    println!("{}", root.to_sexp());
    println!("============================\n");
}
