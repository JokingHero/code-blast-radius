use crate::language::{LanguageConfig, SupportedLanguage};

pub const RUST_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::Rust,
    file_extensions: &["rs"],
    query_defs: r#"
        ;; 1. Standard Function Definitions
        (function_item name: (identifier) @function.name) @function.definition
        
        ;; 2. Macro Definitions (macro_rules! foo {})
        (macro_definition name: (identifier) @function.name) @function.definition

        ;; 3. Common Library Patterns
        
        ;; thread_local! { static FOO: ... }
        (macro_invocation
            macro: (identifier) @m (#eq? @m "thread_local")
            (token_tree
                (identifier) @kw_static (#eq? @kw_static "static")
                .
                (identifier) @function.name
            )
        ) @function.definition

        ;; lazy_static! { static ref FOO: ... }
        (macro_invocation
            macro: (identifier) @m (#eq? @m "lazy_static")
            (token_tree
                (identifier) @kw_ref (#eq? @kw_ref "ref")
                .
                (identifier) @function.name
            )
        ) @function.definition

        ;; 4. Improved Heuristic: "Definers"
        ;; We use #not-match? to skip keywords like 'pub', 'async', 'crate'
        
        ;; Case A: Direct Identifier (e.g., create_struct!(MyStruct))
        (macro_invocation
            macro: (identifier) @macro_type
            (token_tree 
                (identifier) @function.name
            )
            (#match? @macro_type "^(define_|create_|decl_|impl_|make_)")
            (#not-match? @function.name "^(pub|async|unsafe|extern|crate|use)$")
        ) @function.definition

        ;; Case B: Visibility Modifier (e.g., create_struct!(pub MyStruct))
        (macro_invocation
            macro: (identifier) @macro_type
            (token_tree 
                (identifier) @_vis 
                . 
                (identifier) @function.name
            )
            (#match? @macro_type "^(define_|create_|decl_|impl_|make_)")
            (#match? @_vis "^(pub)$")
        ) @function.definition

        ;; 5. Heuristic: Route Definitions (Web Frameworks)
        ;; Matches: route!(GET, "/path", MyHandler) -> Captures MyHandler
        (macro_invocation
            macro: (identifier) @m (#match? @m "(route|endpoint)")
            (token_tree
                (identifier) ;; Method (GET)
                (string_literal) ;; Path
                (identifier) @function.name ;; Handler
            )
        ) @function.definition

        ;; 6. Heuristic: "Test modules"
        (macro_invocation
            macro: (identifier) @macro_type
            (token_tree . (identifier) @function.name)
            (#match? @macro_type "_suite$")
        ) @function.definition
    "#,
    query_calls: r#"(call_expression function: [(identifier) @call.name (field_expression field: (field_identifier) @call.name)])"#,
    query_docs: r#"
        (
            (line_comment)+ @function.docs 
            . 
            [
                (function_item)
                (macro_definition)
                (macro_invocation)
            ] @function.definition
        )
    "#,
    query_imports: r#"
        (use_declaration
            argument: [
                (scoped_identifier)
                (identifier)
            ] @import.source
        )
    "#,
    query_exports: "",
    query_literals: r#"(string_literal) @string"#,
    query_implements: r#"
        (impl_item
            trait: (type_identifier) @impl.parent
            type: (type_identifier) @impl.child
        )
    "#,
    query_config: "",
    query_vals: r#"
        [
            (const_item
                name: (identifier) @val.name
                value: (string_literal) @val.value
            )
            (let_declaration
                pattern: (identifier) @val.name
                value: (string_literal) @val.value
            )
        ]
    "#,
    query_types: r#"
        [
            (parameter type: (type_identifier) @type.ref)
            (function_item return_type: (type_identifier) @type.ref)
            (field_declaration type: (type_identifier) @type.ref)
            (let_declaration type: (type_identifier) @type.ref)
            (struct_expression name: (type_identifier) @type.ref)
            (type_arguments (type_identifier) @type.ref)
        ]
    "#,
    query_decorators: r#"
        (attribute_item) @decorator.name
    "#,
    query_actions: r#"
        (call_expression
            function: (field_expression field: (field_identifier) @fn (#match? @fn "^(emit|dispatch|publish|send)$"))
            arguments: (arguments 
                (string_literal) @action.dispatch
            )
        )
        (match_arm
            pattern: (match_pattern
                (string_literal) @action.handle
            )
        )
        (match_arm
            pattern: (match_pattern
                (or_pattern
                    (string_literal) @action.handle
                )
            )
        )
    "#,
    query_middleware: "",
    query_route_defs: "",
    di_decorators: &[],
    magic_methods: &[]
};