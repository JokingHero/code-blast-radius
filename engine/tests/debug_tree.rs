use rfc_engine::{language::{SupportedLanguage, get_language}, languages::typescript::TYPESCRIPT_CONFIG};
use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};

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

#[test]
fn debug_rust_thread_local() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::Rust);
    parser.set_language(&language).expect("Error loading Rust grammar");

    let code = r#"
        thread_local! {
            pub static FOO: RefCell<u32> = RefCell::new(1);
        }
    "#;
    
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- RUST THREAD_LOCAL STRUCTURE ---");
    println!("{}", tree.root_node().to_sexp());
}

#[test]
fn debug_ts_factories() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::TypeScript);
    parser.set_language(&language).expect("Error loading TS grammar");

    let code = r#"
        // 1. Mongoose
        const User = mongoose.model('User', { name: String });

        // 2. Styled Components (Tagged Template)
        const Title = styled.h1`color: red`;
        
        // 3. Styled Components (Method call)
        const Box = styled('div')({});
    "#;
    
    let tree = parser.parse(code, None).unwrap();
    println!("\n--- TS FACTORY STRUCTURES ---");
    println!("{}", tree.root_node().to_sexp());
}

// --- NEW DEBUG TEST ---
#[test]
fn test_ts_query_execution() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::TypeScript);
    parser.set_language(&language).expect("Error loading TS grammar");

    let code = r#"
        const User = mongoose.model('User', { name: String });
    "#;
    
    let tree = parser.parse(code, None).unwrap();
    
    // This is the snippet from typescript.rs we want to verify
    let query_str = r#"
        (variable_declarator
            name: (identifier) @function.name
            value: (call_expression
                function: (member_expression
                    object: (_)
                    property: [(property_identifier) (identifier)] @fn_name
                    (#match? @fn_name "^(create|make|define|model|component|router|styled)$")
                )
            )
        ) @function.definition
    "#;

    let query = Query::new(&language, query_str).expect("Query compilation failed");
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), code.as_bytes());
    
    println!("\n--- QUERY MATCHES ---");
    let mut count = 0;
    while let Some(m) = matches.next() {
        count += 1;
        println!("Match found!");
        for cap in m.captures {
            let name = query.capture_names()[cap.index as usize];
            let text = cap.node.utf8_text(code.as_bytes()).unwrap();
            println!("  Capture: {} = '{}'", name, text);
        }
    }
    println!("Total matches: {}", count);
    assert!(count > 0, "Query failed to match Mongoose pattern");
}

#[test]
fn debug_compare_strict_vs_loose_query() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::TypeScript);
    parser.set_language(&language).expect("Error loading TS grammar");

    let code = r#"const User = mongoose.model('User', {});"#;
    let tree = parser.parse(code, None).unwrap();
    let root = tree.root_node();

    // 1. Strict Query (Current implementation in typescript.rs)
    let strict_q_str = r#"
        (variable_declarator
            name: (identifier)
            value: (call_expression
                function: (member_expression
                    object: [(identifier) (this) (member_expression)]
                    property: (property_identifier) @fn_name
                    (#match? @fn_name "^(model)$")
                )
            )
        )
    "#;

    // 2. Loose Query (Proposed Fix)
    // Allows object to be anything (_), and property to be identifier OR property_identifier
    let loose_q_str = r#"
        (variable_declarator
            name: (identifier)
            value: (call_expression
                function: (member_expression
                    object: (_)
                    property: [(property_identifier) (identifier)] @fn_name
                    (#match? @fn_name "^(model)$")
                )
            )
        )
    "#;

    let strict_query = Query::new(&language, strict_q_str).expect("Strict query compile");
    let loose_query = Query::new(&language, loose_q_str).expect("Loose query compile");
    
    let mut cursor = QueryCursor::new();

    let strict_count = cursor.matches(&strict_query, root, code.as_bytes()).count();
    let loose_count = cursor.matches(&loose_query, root, code.as_bytes()).count();

    println!("\n--- QUERY COMPARISON ---");
    println!("Strict Matches: {}", strict_count);
    println!("Loose Matches:  {}", loose_count);
    println!("------------------------\n");

    // If this fails, it proves the strict query is missing the node
    assert!(loose_count > 0, "Loose query should definitely match");
}

#[test]
fn debug_full_config_query_on_mongoose() {
    let mut parser = Parser::new();
    let language = get_language(SupportedLanguage::TypeScript);
    parser.set_language(&language).expect("Error loading TS grammar");

    // The code that is failing in factory_test
    let code = r#"const User = mongoose.model('User', { name: String });"#;
    let tree = parser.parse(code, None).unwrap();
    
    // Compile the ACTUAL full query from the config
    let query = Query::new(&language, TYPESCRIPT_CONFIG.query_defs).expect("Full query compilation failed");
    
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), code.as_bytes());

    println!("\n--- FULL CONFIG MATCHES ---");
    let mut found_factory = false;
    let mut match_count = 0;

    while let Some(m) = matches.next() {
        match_count += 1;
        println!("Match #{}: pattern_index={}", match_count, m.pattern_index);
        for cap in m.captures {
            let name = query.capture_names()[cap.index as usize];
            let text = cap.node.utf8_text(code.as_bytes()).unwrap();
            
            println!("  Capture: {} -> '{}'", name, text);

            if name == "function.definition" {
                found_factory = true;
            }
        }
    }
    println!("---------------------------\n");

    assert!(found_factory, "The full TYPESCRIPT_CONFIG query failed to match the Mongoose factory pattern");
}