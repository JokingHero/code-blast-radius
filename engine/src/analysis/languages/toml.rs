use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::Toml,
        &["toml"]
    )
    .defs(r#"
        [
            (pair key: (bare_key) @function.name) 
            (table (bare_key) @function.name)
        ] @function.definition
    "#)
    .literals(r#"(string) @string"#)
    .vals(r#"
        (pair
            key: (bare_key) @val.name
            value: [
                (string) @val.value
                (integer) @val.value
                (boolean) @val.value
            ]
        )
    "#)
    .build()
}