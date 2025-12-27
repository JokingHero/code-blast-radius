use crate::language::{LanguageConfig, SupportedLanguage};

pub const TOML_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::Toml,
    file_extensions: &["toml"],
    // Matches key = value or [table_name]
    query_defs: r#"
        [
            (pair key: (bare_key) @function.name) 
            (table (bare_key) @function.name)
        ] @function.definition
    "#,
    query_calls: "",
    query_docs: "",
    query_imports: "",
    query_exports: "",
    query_literals: r#"(string) @string"#,
    query_implements: "",
    query_config: "",
    query_vals: "",
    query_types: "",
    query_decorators: "",
    di_decorators: &[]
};