use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::Ruby,
        &["rb"]
    )
    .defs(r#"
        (method name: (identifier) @function.name body: (body_statement) @function.body) @function.definition
        (class name: [
            (constant) @function.name
            (scope_resolution name: (constant) @function.name)
        ]) @function.definition
        (module name: [
            (constant) @function.name
            (scope_resolution name: (constant) @function.name)
        ]) @function.definition
    "#)
    .calls(r#"
        (call method: (identifier) @call.name)
    "#)
    .imports(r#"
        (call
            method: (identifier) @m
            arguments: (argument_list (string) @import.source)
            (#match? @m "^(require|require_relative|load)$")
        )
    "#)
    .literals(r#"
        [(string) (bare_string) (heredoc_body)] @string
    "#)
    .project_config_files(&["Gemfile", "Rakefile"])
    .build()
}
