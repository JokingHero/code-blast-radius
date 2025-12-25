use crate::language::{LanguageConfig, SupportedLanguage};

pub const DOTENV_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::Dotenv,
    file_extensions: &["env", "env.example", "env.template"],
    // In Bash grammar, VAR=VAL is a variable_assignment
    query_defs: r#"(variable_assignment name: (variable_name) @function.name) @function.definition"#,
    query_calls: "",
    query_docs: r#"(comment) @function.docs . (variable_assignment)"#,
    query_imports: "",
    query_exports: "",
    query_literals: r#"(string) @string"#,
    query_implements: "",
};