use crate::language::{LanguageConfig, SupportedLanguage};

pub const RUST_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::Rust,
    file_extensions: &["rs"],
    query_defs: r#"(function_item name: (identifier) @function.name) @function.definition"#,
    query_calls: r#"(call_expression function: [(identifier) @call.name (field_expression field: (field_identifier) @call.name)])"#,
    query_docs: r#"((line_comment)+ @function.docs . (function_item) @function.definition)"#,
    query_imports: "",
    query_exports: "",
    query_literals: r#"(string_literal) @string"#,
    query_implements: r#"
        (impl_item
            trait: (type_identifier) @impl.parent
            type: (type_identifier) @impl.child
        )
    "#,
    query_config: "",
    // Matches: const X: &str = "val" or let x = "val"
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
    // Matches types in args, return, fields, and generics
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
};