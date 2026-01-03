use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::Dotenv,
        &["env", "env.example", "env.template", "env.local", "env.development", "env.test", "env.production"]
    )
    .defs(r#"
        (variable_assignment
            name: (variable_name) @function.name
            value: (_) @function.body
        ) @function.definition
    "#)
    .vals(r#"
        (variable_assignment
            name: (variable_name) @val.name
            value: [(word) (string) (raw_string)] @val.value
        )
    "#)
    .docs(r#"
        (
            (comment)+ @function.docs 
            . 
            (variable_assignment) @function.definition
        )
    "#)
    .literals(r#"
        [
            (string)
            (raw_string)
        ] @string
    "#)
    .build()
}