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
    .frameworks(r#"
        ;; --- LARAVEL ROUTES ---
        ;; Route::get('/user', ...)
        (scoped_call_expression
            scope: (name) @scope
            name: (name) @framework.key
            arguments: (arguments (argument (string) @framework.value))
            (#eq? @scope "Route")
            (#match? @framework.key "^(get|post|put|delete|patch|group)$")
        )

        ;; --- SYMFONY ATTRIBUTES (PHP 8+) ---
        ;; #[Route('/api')]
        (attribute
            (name) @framework.key
            parameters: (arguments (argument (string) @framework.value))
            (#eq? @framework.key "Route")
        )
        
        ;; #[Route(path: '/api')]
        (attribute
            (name) @framework.key
            parameters: (arguments 
                (argument 
                    name: (name) @arg_name
                    (string) @framework.value
                )
            )
            (#eq? @framework.key "Route")
            (#eq? @arg_name "path")
        )
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
