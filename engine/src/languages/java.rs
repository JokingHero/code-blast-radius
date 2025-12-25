use crate::language::{LanguageConfig, SupportedLanguage};

pub const JAVA_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::Java,
    file_extensions: &["java"],
    query_defs: r#"(method_declaration name: (identifier) @function.name) @function.definition"#,
    query_calls: r#"(method_invocation name: (identifier) @call.name)"#,
    query_docs: r#"((block_comment) @function.docs . (method_declaration) @function.definition)"#,
    query_imports: "",
    query_exports: "",
    query_literals: r#"(string_literal) @string"#,
    query_implements: r#"
        (class_declaration
            name: (identifier) @impl.child
            superclass: (superclass (type_identifier) @impl.parent)?
        )
    "#,
};