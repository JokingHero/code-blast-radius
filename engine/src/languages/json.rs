use crate::language::{LanguageConfig, SupportedLanguage};

pub const JSON_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::Json,
    file_extensions: &["json"],
    // Matches keys in "key": value pairs
    query_defs: r#"(pair key: (string (string_content) @function.name)) @function.definition"#,
    query_calls: "",
    query_docs: "",
    query_imports: "",
    query_exports: "",
    query_literals: r#"(string_content) @string"#,
    query_implements: "",
    query_config: "",
    query_vals: "",
    query_types: "",
    query_decorators: "",
    di_decorators: &[]
};