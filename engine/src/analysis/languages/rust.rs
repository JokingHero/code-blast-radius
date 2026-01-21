use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::Rust,
        &["rs"]
    )
    .skeleton("{ /* ... {} ... */ }")
    .defs(r#"
        ;; 1. Standard Function Definitions
        (function_item
            name: (identifier) @function.name
            body: (block) @function.body
        ) @function.definition

        ;; 2. Struct Definitions
        (struct_item
            name: (type_identifier) @function.name
            body: (field_declaration_list) @function.body
        ) @function.definition

        ;; 3. Enum Definitions
        (enum_item
            name: (type_identifier) @function.name
            body: (enum_variant_list) @function.body
        ) @function.definition

        ;; 3b. Trait Definitions
        (trait_item
            name: (type_identifier) @function.name
            body: (declaration_list) @function.body
        ) @function.definition

        ;; 4. Macro Definitions (macro_rules! foo {})
        (macro_definition
            name: (identifier) @function.name
        ) @function.definition

        ;; 5. Impl blocks - anonymous container
        (impl_item
            type: (type_identifier)
            body: (declaration_list) @function.body
        ) @function.definition

        ;; 4. Common Library Patterns
        (macro_invocation
            macro: (identifier) @m (#eq? @m "thread_local")
            (token_tree
                (identifier) @function.name
                (#not-match? @function.name "^(static|ref)$")
            )
        ) @function.definition

        (macro_invocation
            macro: (identifier) @m (#eq? @m "lazy_static")
            (token_tree
                (identifier) @function.name
                (#not-match? @function.name "^(static|ref)$")
            )
        ) @function.definition

        ;; 4. Improved Heuristic: "Definers"
        (macro_invocation
            macro: (identifier) @macro_type
            (token_tree 
                (identifier) @function.name
            )
            (#match? @macro_type "^(define_|create_|decl_|impl_|make_)")
            (#not-match? @function.name "^(pub|async|unsafe|extern|crate|use)$")
        ) @function.definition

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
    "#)
    // --- NEW: Inference Engine Hooks ---
    .frameworks(r#"
        ;; --- ACTIX WEB / ROCKET ATTRIBUTES ---
        ;; Captures: #[get("/api")] -> key="actix.get", value="/api"
        (attribute_item
            (attribute
                (identifier) @framework.key
                (token_tree
                    (string_literal) @framework.value
                )
            )
            (#match? @framework.key "^(get|post|put|delete|patch)$")
        )

        ;; --- AXUM ROUTER ---
        ;; Captures: .route("/users", get(users_handler)) -> key="axum.route", value="/users"
        (call_expression
            function: (field_expression
                field: (field_identifier) @framework.key
            )
            arguments: (arguments
                (string_literal) @framework.value
            )
            (#eq? @framework.key "route")
        )
    "#)
    // --- EXISTING LOGIC ---
    .calls(r#"(call_expression function: [(identifier) @call.name (field_expression field: (field_identifier) @call.name)])"#)
    .docs(r#"
        (
            (line_comment)+ @function.docs 
            . 
            [
                (function_item)
                (macro_definition)
                (macro_invocation)
            ] @function.definition
        )
    "#)
    .imports(r#"
        (use_declaration
            argument: [
                (scoped_identifier)
                (identifier)
            ] @import.source
        )
    "#)
    .literals(r#"(string_literal) @string"#)
    .implements(r#"
        (impl_item
            trait: (type_identifier) @impl.parent
            type: (type_identifier) @impl.child
        )
    "#)
    .config_keys(r#"
        (call_expression
            function: (scoped_identifier
                path: (identifier) @mod
                name: (identifier) @fn
            )
            arguments: (arguments (string_literal) @config.key)
            (#eq? @mod "env")
            (#eq? @fn "var")
        )
    "#)
    .vals(r#"
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
    "#)
    .types(r#"
        [
            (parameter type: (type_identifier) @type.ref)
            (function_item return_type: (type_identifier) @type.ref)
            (field_declaration type: (type_identifier) @type.ref)
            (let_declaration type: (type_identifier) @type.ref)
            (struct_expression name: (type_identifier) @type.ref)
            (type_arguments (type_identifier) @type.ref)
        ]
    "#)
    .decorators(r#"
        (attribute_item) @decorator.name
    "#)
    .actions(r#"
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
    "#)
    .constructor_names(&["new", "with_capacity", "default"])
    .project_config_files(&["Cargo.toml"])
    .build()
}