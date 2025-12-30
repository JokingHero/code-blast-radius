use crate::language::{LanguageConfig, SupportedLanguage};

pub const R_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::R,
    file_extensions: &["R", "r"],
    query_defs: r#"(function_definition name: (identifier) @function.name) @function.definition"#,
    query_calls: r#"(call_expression function: (identifier) @call.name)"#,
    query_docs: r#"((comment)+ @function.docs . (function_definition) @function.definition)"#,
    query_imports: "",
    query_exports: "",
    query_literals: r#"(string) @string"#,
    query_implements: "",
    query_config: "",
    query_vals: "",
    query_types: "",
    query_decorators: "",
    query_actions: "",
    query_middleware: "",
    query_route_defs: "",
    di_decorators: &[],
    magic_methods: &[]
};