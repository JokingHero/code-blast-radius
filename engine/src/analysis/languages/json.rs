use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::Json,
        &["json"]
    )
    // Matches keys in "key": value pairs
    .defs(r#"(pair key: (string (string_content) @function.name)) @function.definition"#)
    .literals(r#"(string_content) @string"#)
    .build()
}