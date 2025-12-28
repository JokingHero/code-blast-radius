use crate::{language::{LanguageConfig, SupportedLanguage}, languages::typescript::TYPESCRIPT_CONFIG};

pub const JAVASCRIPT_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::JavaScript,
    file_extensions: &["js", "jsx", "mjs", "cjs", "vue"],
    query_defs: r#"
        [
          (function_declaration name: (identifier) @function.name) @function.definition
          (generator_function_declaration name: (identifier) @function.name) @function.definition
          (method_definition name: (property_identifier) @function.name) @function.definition
          (class_declaration name: (identifier) @function.name) @function.definition
          ((variable_declarator name: (identifier) @function.name value: [(arrow_function) (function_expression)]) @function.definition)
        ]
    "#,
    query_calls: TYPESCRIPT_CONFIG.query_calls,
    query_docs: r#"
      (
        (comment)+ @function.docs
        .
        [ 
            (function_declaration) 
            (generator_function_declaration)
            (method_definition) 
            (export_statement (variable_declaration)) 
            (class_declaration) 
        ] @function.definition
      )
    "#,
    query_imports: TYPESCRIPT_CONFIG.query_imports,
    query_exports: "",
    query_literals: r#"[ (string) (template_string) ] @string"#,
    query_implements: r#"
        (class_declaration
            name: (identifier) @impl.child
            (class_heritage (identifier) @impl.parent)
        )
    "#,
    query_config: "",
    query_vals: r#"
        (variable_declarator
            name: (identifier) @val.name
            value: [(string) (template_string)] @val.value
        )
    "#,
    query_types: "",
    query_decorators: "",
    query_actions: TYPESCRIPT_CONFIG.query_actions,
    di_decorators: &[],
    magic_methods: &[]
};