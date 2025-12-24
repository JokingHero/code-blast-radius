use crate::language::{LanguageConfig, SupportedLanguage};

pub const HTML_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::Html,
    file_extensions: &["html", "htm"],
    query_defs: r#"
      (script_element
        (raw_text) @script_content
        (#match? @script_content "function\\s+([a-zA-Z0-9_]+)")
        (function_declaration name: (identifier) @function.name) @function.definition
      )
    "#,
    query_calls: r#"
      (script_element
        (call_expression
          function: (identifier) @call.name
        )
      )
    "#,
    query_docs: r#"
      (script_element
        (comment) @function.docs
        .
        (function_declaration) @function.definition
      )
    "#,
    query_imports: "",
    query_literals: r#"(attribute_value) @string"#,
    query_implements: "",
};