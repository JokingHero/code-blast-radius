use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::C,
        &["c", "h"]
    )
    .skeleton("{ /* ... {} ... */ }")
    .defs(r#"
        (function_definition
            declarator: [
                (identifier) @function.name
                (function_declarator declarator: (identifier) @function.name)
                (pointer_declarator declarator: (function_declarator declarator: (identifier) @function.name))
            ]
            body: (compound_statement) @function.body
        ) @function.definition

        (struct_specifier
            name: (type_identifier) @function.name
            body: (field_declaration_list) @function.body
        ) @function.definition
    "#)
    // --- NEW: Inference Engine Hooks (Generic Macro Support) ---
    .frameworks(r#"
        ;; Captures: DEFINE_COMMAND("ping", ping_handler) -> key="DEFINE_COMMAND", value="ping"
        (call_expression
            function: (identifier) @framework.key
            arguments: (argument_list
                (string_literal) @framework.value
            )
            (#match? @framework.key "^[A-Z_]+$") ;; Heuristic: Macros are usually UPPERCASE
        )
    "#)
    // --- EXISTING LOGIC ---
    .calls(r#"
        (call_expression
            function: (identifier) @call.name
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