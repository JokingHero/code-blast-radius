use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(SupportedLanguage::GdScript, &["gd"])
        .skeleton("# ... {} ...")
        .defs(r#"
        (function_definition
            name: (name) @function.name
            body: (body) @function.body
        ) @function.definition

        (class_definition
            name: (name) @function.name
            body: (class_body) @function.body
        ) @function.definition

        (signal_statement
            name: (name) @function.name
        ) @function.definition
        "#)
        .calls(r#"
        (call_expression
            function: (name) @call.name
        )
        (call_expression
            function: (attribute_expression
                attribute: (name) @call.name
            )
        )
        "#)
        .imports(r#"
        (extends_statement
            (string) @import.source
        )
        "#)
        .literals(r#"(string) @string"#)
        .vals(r#"
        (variable_statement
            name: (name) @val.name
        )
        (const_statement
            name: (name) @val.name
        )
        "#)
        .build()
}