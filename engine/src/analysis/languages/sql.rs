use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::Sql,
        &["sql"]
    )
    .defs(r#"
        (statement
            (create_table
                (object_reference
                    name: (identifier) @function.name
                )
                (column_definitions) @function.body
            )
        ) @function.definition
    "#)
    .docs(r#"
        (
            (comment)+ @function.docs 
            . 
            (create_table) @function.definition
        )
    "#)
    // Capture string literals (e.g. 'active', 'user')
    .literals(r#"
        (literal) @string
    "#)
    .types(r#"
        ;; Capture column names as type refs.
        ;; This allows heuristics to link code variables (e.g. user_id) to SQL columns.
        (column_definition
            name: (identifier) @type.ref
        )
    "#)
    .build()
}