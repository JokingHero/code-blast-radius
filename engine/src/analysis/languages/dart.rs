use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(SupportedLanguage::Dart, &["dart"])
        .skeleton("{ /* ... {} ... */ }")
        .defs(r#"
        ;; Functions (Top-level)
        (lambda_expression
            parameters: (function_signature
                name: (identifier) @function.name
            )
            body: (function_body) @function.body
        ) @function.definition

        ;; Classes
        (class_definition
            name: (identifier) @function.name
            body: (class_body) @function.body
        ) @function.definition

        ;; Methods
        (class_member_definition
            (method_signature
                (function_signature
                    name: (identifier) @function.name
                )
            )
            (function_body) @function.body
        ) @function.definition

        ;; Enums
        (enum_declaration
            name: (identifier) @function.name
            body: (enum_body) @function.body
        ) @function.definition
        "#)
        .frameworks(r#"
        (class_definition
            name: (identifier) @framework.value
            superclass: (superclass (type_identifier) @framework.key)
            (#match? @framework.key "^(StatelessWidget|StatefulWidget|ConsumerWidget|Riverpod)$")
        )
        "#)
        .calls(r#"
        (call_expression
            function: [
                (identifier) @call.name
                (selector (identifier) @call.name)
            ]
        )
        "#)
        .imports(r#"
        (library_import
            (import_specification
                (configurable_uri 
                    (uri (string_literal) @import.source)
                )
            )
        )
        "#)
        .literals(r#"(string_literal) @string"#)
        .build()
}