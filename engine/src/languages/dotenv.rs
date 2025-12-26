use crate::language::{LanguageConfig, SupportedLanguage};

pub const DOTENV_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::Dotenv,
    file_extensions: &["env", "env.example", "env.template"],
    // Use the specific bash grammar nodes for assignments
    query_defs: r#"(variable_assignment name: (variable_name) @function.name) @function.definition"#,
    query_calls: "",
    // Simplified docs query (removed the strict anchor)
    query_docs: r#"((comment) @function.docs (variable_assignment))"#,
    query_imports: "",
    query_exports: "",
    query_literals: r#"(string) @string"#,
    query_implements: "",
    query_config: "",
    query_vals: r#"
        (variable_assignment
            name: (variable_name) @val.name
            value: (word) @val.value
        )
    "#,
    query_types: "",
};