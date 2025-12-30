use crate::language::{LanguageConfig, SupportedLanguage};

pub const BASH_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::Bash,
    file_extensions: &["sh", "bash"],
    query_defs: r#"(function_definition name: (word) @function.name) @function.definition"#,
    query_calls: r#"(command name: (command_name (word) @call.name))"#,
    query_docs: r#"((comment)+ @function.docs . (function_definition) @function.definition)"#,
    query_imports: "",
    query_exports: "",
    query_literals: r#"(word) @string"#,
    query_implements: "",
    query_config: "",
    // Capture: MY_VAR="value"
    query_vals: r#"
        (variable_assignment
            name: (variable_name) @val.name
            value: (word) @val.value
        )
    "#,
    query_types: "",
    query_decorators: "",
    query_actions: "",
    query_middleware: "",
    query_route_defs: "",
    di_decorators: &[],
    magic_methods: &[]
};