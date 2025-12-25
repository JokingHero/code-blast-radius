use crate::language::{LanguageConfig, SupportedLanguage};

pub const PYTHON_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::Python,
    file_extensions: &["py"],
    query_defs: r#"(function_definition name: (identifier) @function.name) @function.definition"#,
    query_calls: r#"(call function: [(identifier) @call.name (attribute attribute: (identifier) @call.name)])"#,
    query_docs: r#"(function_definition body: (block . (expression_statement (string) @function.docs))) @function.definition"#,
    query_imports: "", 
    query_exports: "",
    query_literals: r#"(string) @string"#,
    query_implements: r#"
        (class_definition
            name: (identifier) @impl.child
            superclasses: (argument_list (identifier) @impl.parent)
        )
    "#,
    query_config: "",
};