use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::Go,
        &["go"]
    )
    .skeleton("{ /* ... {} ... */ }")
    .defs(r#"
        (function_declaration name: (identifier) @function.name body: (block) @function.body) @function.definition
        (method_declaration name: (field_identifier) @function.name body: (block) @function.body) @function.definition
        (type_spec name: (type_identifier) @function.name type: (struct_type (field_declaration_list) @function.body)) @function.definition
    "#)
    // --- NEW: Inference Engine Hooks ---
    .frameworks(r#"
        ;; --- GIN / ECHO / FIBER ROUTES ---
        ;; Captures: router.GET("/api/ping", ...) -> key="GET", value="/api/ping"
        (call_expression
            function: (selector_expression
                field: (field_identifier) @framework.key
            )
            arguments: (argument_list
                (interpreted_string_literal) @framework.value
            )
            (#match? @framework.key "^(GET|POST|PUT|DELETE|PATCH|Group)$")
        )
    "#)
    // --- EXISTING LOGIC ---
    .calls(r#"
        (call_expression function: [
            (identifier) @call.name
            (selector_expression field: (field_identifier) @call.name)
        ])
    "#)
    .imports(r#"
        (import_spec path: (interpreted_string_literal) @import.source)
    "#)
    .literals(r#"
        [(interpreted_string_literal) (raw_string_literal)] @string
    "#)
    .project_config_files(&["go.mod", "go.sum"])
    .build()
}