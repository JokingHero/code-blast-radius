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
    // --- NEW: Inference Engine Hooks ---
    .frameworks(r#"
        ;; --- LARAVEL ROUTES ---
        ;; Captures: Route::get('/api/user', ...) -> key="get", value="'/api/user'"
        (scoped_call_expression
            scope: (name) @scope
            name: (name) @framework.key
            arguments: (arguments (argument (string) @framework.value))
            (#eq? @scope "Route")
            (#match? @framework.key "^(get|post|put|delete|patch|group)$")
        )

        ;; --- SYMFONY ATTRIBUTES (PHP 8+) ---
        ;; Captures: #[Route('/api/user')] -> key="Route", value="'/api/user'"
        (attribute
            name: (name) @framework.key
            arguments: (arguments (argument (string) @framework.value))
            (#eq? @framework.key "Route")
        )
    "#)
    // --- EXISTING LOGIC ---
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