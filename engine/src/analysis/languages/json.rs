use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::Json,
        &["json"]
    )
    .skeleton("")
    // Matches keys in "key": value pairs
    .defs(r#"(pair key: (string (string_content) @function.name) value: (_) @function.body) @function.definition"#)
    .literals(r#"(string_content) @string"#)
    .build()
}
