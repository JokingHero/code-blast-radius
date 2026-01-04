use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::Bash,
        &["sh", "bash", "zsh"]
    )
    .skeleton(" : # {} body hidden")
    .defs(r#"
        (function_definition 
            name: (word) @function.name
        ) @function.definition
    "#)
    .calls(r#"
        (command 
            name: (command_name (word) @call.name)
        )
    "#)
    .imports(r#"
        (command
            name: (command_name (word) @cmd)
            argument: [(word) (string)] @import.source
            (#match? @cmd "^(source|\\.)$")
        )
    "#)
    .exports(r#"
        (declaration_command
            (variable_assignment
                name: (variable_name) @export.name
            )
        )
    "#)
    .vals(r#"
        (variable_assignment
            name: (variable_name) @val.name
            value: [(word) (string) (raw_string)] @val.value
        )
    "#)
    .docs(r#"
        (
            (comment)+ @function.docs 
            . 
            (function_definition) @function.definition
        )
    "#)
    .literals(r#"
        [
            (string)
            (raw_string)
            (heredoc_body)
            (word)
        ] @string
    "#)
    .build()
}