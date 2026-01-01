use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::Php,
        &["php"]
    )
    .defs(r#"
        (function_definition name: (identifier) @function.name) @function.definition
        (method_declaration name: (identifier) @function.name) @function.definition
        (class_declaration name: (identifier) @function.name) @function.definition
    "#)
    .calls(r#"
        (function_call_expression
            function: [
                (qualified_name (identifier) @call.name)
                (member_call_expression name: (field_identifier) @call.name)
            ]
        )
    "#)
    .imports(r#"
        (namespace_use_clause (qualified_name) @import.source)
        (include_expression (string) @import.source)
        (require_expression (string) @import.source)
    "#)
    .literals(r#"
        [(string) (heredoc)] @string
    "#)
    .project_config_files(&["composer.json"])
    .build()
}
