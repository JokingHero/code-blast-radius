use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::C,
        &["c", "h"]
    )
    .defs(r#"
        (function_definition
            declarator: [
                (identifier) @function.name
                (function_declarator declarator: (identifier) @function.name)
                (pointer_declarator declarator: (function_declarator declarator: (identifier) @function.name))
            ]
        ) @function.definition
    "#)
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
