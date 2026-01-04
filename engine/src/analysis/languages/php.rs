use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::Php,
        &["php"]
    )
    .skeleton("{ /* ... {} ... */ }")
    .defs(r#"
        (function_definition name: (name) @function.name body: (compound_statement) @function.body) @function.definition
        (method_declaration name: (name) @function.name body: (compound_statement) @function.body) @function.definition
        (class_declaration name: (name) @function.name body: (declaration_list) @function.body) @function.definition
    "#)
    .calls(r#"
        (function_call_expression
            function: [
                (qualified_name (name) @call.name)
                (member_call_expression name: (name) @call.name)
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
