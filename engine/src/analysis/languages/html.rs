use crate::analysis::language::{LanguageConfig, SupportedLanguage};

pub const HTML_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::Html,
    file_extensions: &["html", "htm"],
    query_defs: r#"(attribute (attribute_name) @attr (#eq? @attr "id") (attribute_value) @function.name) @function.definition"#,
    query_calls: "",
    query_docs: "",
    query_imports: "",
    query_exports: "",
    query_literals: r#"(attribute_value) @string"#,
    query_implements: "",
    query_config: "",
    query_vals: "",
    query_types: "",
    query_decorators: "",
    query_actions: "",
    query_middleware: "",
    di_decorators: &[],
    magic_methods: &[],
    query_route_defs: "",
};