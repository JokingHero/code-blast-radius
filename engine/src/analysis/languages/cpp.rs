use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::Cpp,
        &["cpp", "hpp", "cc", "cxx", "hh"]
    )
    .skeleton("{ /* ... {} ... */ }")
    .defs(r#"
        (function_definition
            declarator: [
                (identifier) @function.name
                (function_declarator declarator: (identifier) @function.name)
                (function_declarator declarator: (field_identifier) @function.name)
                (pointer_declarator declarator: (function_declarator declarator: (identifier) @function.name))
                (pointer_declarator declarator: (function_declarator declarator: (field_identifier) @function.name))
                (qualified_identifier name: (identifier) @function.name)
                (field_identifier) @function.name
            ]
            body: (compound_statement) @function.body
        ) @function.definition
        (class_specifier
            name: [
                (type_identifier) @function.name
                (qualified_identifier name: (type_identifier) @function.name)
            ]
            body: (field_declaration_list) @function.body
        ) @function.definition
    "#)
    // --- NEW: Inference Engine Hooks ---
    .frameworks(r#"
        ;; --- CROW ROUTES ---
        ;; Captures: CROW_ROUTE(app, "/hello") -> key="CROW_ROUTE", value="/hello"
        (call_expression
            function: (identifier) @framework.key
            arguments: (argument_list
                (identifier) ;; app variable
                (string_literal) @framework.value
            )
            (#eq? @framework.key "CROW_ROUTE")
        )

        ;; --- PISTACHE ROUTES ---
        ;; Captures: Routes::Get(router, "/resource", ...) -> key="Get", value="/resource"
        (call_expression
            function: (qualified_identifier
                scope: (namespace_identifier) @scope
                name: (identifier) @framework.key
            )
            arguments: (argument_list
                (identifier) ;; router
                (string_literal) @framework.value
            )
            (#eq? @scope "Routes")
            (#match? @framework.key "^(Get|Post|Put|Delete)$")
        )
    "#)
    // --- EXISTING LOGIC ---
    .calls(r#"
        (call_expression
            function: [
                (identifier) @call.name
                (field_expression field: (field_identifier) @call.name)
                (qualified_identifier name: (identifier) @call.name)
            ]
        )
    "#)
    .imports(r#"
        (preproc_include path: [
            (string_literal) @import.source
            (system_lib_string) @import.source
        ])
    "#)
    .literals(r#"
        (string_literal) @string
    "#)
    .project_config_files(&["Makefile", "CMakeLists.txt"])
    .build()
}
