use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(SupportedLanguage::FirebaseRules, &["rules"])
        .skeleton("{ /* ... {} ... */ }")
        .defs(r#"
        ;; Function Declaration
        ;; Matches: function name() { ... }
        (function_declaration
            name: (identifier) @function.name
            body: (function_body) @function.body
        ) @function.definition

        ;; Service Declaration
        ;; Matches: service cloud.firestore { ... }
        ;; We only capture the name, as the body is flattened in the AST.
        (service_declaration
            name: (service_name_identifier) @function.name
        ) @function.definition

        ;; Match Declaration
        ;; Matches: match /path/to/doc { ... }
        ;; We capture the path as the name. The body is flattened in the AST (no single block node).
        (match_declaration
            path: (_) @function.name
        ) @function.definition
        "#)
        .calls(r#"
        (call_expression
            function: [
                (identifier) @call.name
                (member_expression member: (identifier) @call.name)
            ]
        )
        "#)
        .imports(r#""#)
        .literals(r#"(string) @string"#)
        .project_config_files(&["firestore.rules", "storage.rules"])
        .build()
}