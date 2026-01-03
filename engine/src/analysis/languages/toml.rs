use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::Toml,
        &["toml"]
    )
    .defs(r#"
        [
            (pair (bare_key) @function.name (string) @function.body)
            (table (bare_key) @function.name)
        ] @function.definition
    "#)
    .literals(r#"(string) @string"#)
    .vals(r#"
        (pair
            (bare_key) @val.name
            (string) @val.value
        )
        (pair
            (bare_key) @val.name
            (integer) @val.value
        )
        (pair
            (bare_key) @val.name
            (boolean) @val.value
        )
    "#)
    .build()
}