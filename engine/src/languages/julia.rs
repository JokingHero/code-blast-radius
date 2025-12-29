use crate::language::{LanguageConfig, SupportedLanguage};

pub const JULIA_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::Julia,
    file_extensions: &["jl"],
    query_defs: r#"(function_definition name: (identifier) @function.name) @function.definition"#,
    query_calls: r#"(call_expression function: (identifier) @call.name)"#,
    query_docs: r#"((block_comment) @function.docs . (function_definition) @function.definition)"#,
    query_imports: "",
    query_exports: "",
    query_literals: r#"(string_literal) @string"#,
    query_implements: "",
    query_config: "",
    query_vals: "",
    query_types: "",
    query_decorators: "",
    query_actions: "",
    query_middleware: "",
    di_decorators: &[],
    magic_methods: &[]
};