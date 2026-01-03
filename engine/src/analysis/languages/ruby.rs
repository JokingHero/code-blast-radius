use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::Ruby,
        &["rb"]
    )
    .defs(r#"
        ;; Methods - body is optional for one-liners like "def foo; end"
        (method
            name: (identifier) @function.name
            body: (body_statement)? @function.body
        ) @function.definition
        ;; Singleton methods (def self.foo)
        (singleton_method
            name: (identifier) @function.name
            body: (body_statement)? @function.body
        ) @function.definition
        ;; Classes
        (class name: [
            (constant) @function.name
            (scope_resolution name: (constant) @function.name)
        ]) @function.definition
        ;; Modules
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
