use tree_sitter::Language;

// An enum to represent our supported languages.
#[derive(Clone, Copy, Debug)]
pub enum SupportedLanguage {
    Rust,
    TypeScript,
    Python,
    Java,
    JavaScript,
    Bash,
    Html,
    Julia,
    R,
}

// Returns the correct Tree-sitter Language object from the pre-compiled crate.
pub fn get_language(lang: SupportedLanguage) -> Language {
    match lang {
        SupportedLanguage::Rust => tree_sitter_rust::LANGUAGE.into(),
        SupportedLanguage::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        SupportedLanguage::Python => tree_sitter_python::LANGUAGE.into(),
        SupportedLanguage::Java => tree_sitter_java::LANGUAGE.into(),
        SupportedLanguage::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
        SupportedLanguage::Bash => tree_sitter_bash::LANGUAGE.into(),
        SupportedLanguage::Html => tree_sitter_html::LANGUAGE.into(),
        SupportedLanguage::Julia => tree_sitter_julia::LANGUAGE.into(),
        SupportedLanguage::R => tree_sitter_r::LANGUAGE.into(),
    }
}

// A struct to hold all language-specific configurations.
pub struct LanguageConfig {
    pub lang_enum: SupportedLanguage,
    pub file_extensions: &'static [&'static str],
    pub query_defs: &'static str,
    pub query_calls: &'static str,
    pub query_docs: &'static str,
}

// --- Existing Configurations ---

pub const RUST_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::Rust,
    file_extensions: &["rs"],
    query_defs: r#"
        (function_item
          name: (identifier) @function.name) @function.definition
    "#,
    query_calls: r#"
        (call_expression
          function: [(identifier) @call.name (field_expression field: (field_identifier) @call.name)])
    "#,
    query_docs: r#"
        (
          (line_comment)+ @function.docs
          .
          (function_item) @function.definition
        )
        (
          (block_comment) @function.docs
          .
          (function_item) @function.definition
        )
    "#,
};

pub const TYPESCRIPT_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::TypeScript,
    file_extensions: &["ts", "tsx"],
    query_defs: r#"
        [
          (function_declaration
            name: (identifier) @function.name) @function.definition
          (method_definition
            name: (property_identifier) @function.name) @function.definition
          (
            (variable_declarator
              name: (identifier) @function.name
              value: [(arrow_function) (function_expression)]) @function.definition
          )
        ]
    "#,
    query_calls: r#"
        (call_expression
          function: [(identifier) @call.name (member_expression property: (property_identifier) @call.name)])
    "#,
    query_docs: r#"
      (
        (comment)+ @function.docs
        .
        [
            (function_declaration)
            (method_definition)
            (export_statement (variable_declaration))
        ] @function.definition
      )
    "#,
};

pub const JAVASCRIPT_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::JavaScript,
    file_extensions: &["js", "jsx", "mjs", "cjs"],
    query_defs: r#"
        [
          (function_declaration
            name: (identifier) @function.name) @function.definition
          (method_definition
            name: (property_identifier) @function.name) @function.definition
          (
            (variable_declarator
              name: (identifier) @function.name
              value: [(arrow_function) (function_expression)]) @function.definition
          )
        ]
    "#,
    query_calls: r#"
        (call_expression
          function: [(identifier) @call.name (member_expression property: (property_identifier) @call.name)])
    "#,
    query_docs: r#"
      (
        (comment)+ @function.docs
        .
        [
            (function_declaration)
            (method_definition)
            (export_statement (variable_declaration))
        ] @function.definition
      )
    "#,
};

pub const PYTHON_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::Python,
    file_extensions: &["py"],
    query_defs: r#"
        (function_definition
          name: (identifier) @function.name) @function.definition
    "#,
    query_calls: r#"
        (call
          function: [(identifier) @call.name (attribute attribute: (identifier) @call.name)])
    "#,
    // Catches docstrings
    query_docs: r#"
        (function_definition
          body: (block . (expression_statement (string) @function.docs))) @function.definition
    "#,
};

pub const JAVA_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::Java,
    file_extensions: &["java"],
    query_defs: r#"
        (method_declaration
          name: (identifier) @function.name) @function.definition
    "#,
    query_calls: r#"
        (method_invocation
          name: (identifier) @call.name)
    "#,
    query_docs: r#"
        (
          (block_comment) @function.docs
          .
          (method_declaration) @function.definition
        )
    "#,
};

pub const BASH_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::Bash,
    file_extensions: &["sh", "bash"],
    query_defs: r#"
        (function_definition
          name: (word) @function.name) @function.definition
    "#,
    // This is a best-effort for bash, as commands can be complex
    query_calls: r#"
        (command
          name: (command_name (word) @call.name))
    "#,
    query_docs: r#"
        (
          (comment)+ @function.docs
          .
          (function_definition) @function.definition
        )
    "#,
};

pub const JULIA_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::Julia,
    file_extensions: &["jl"],
    query_defs: r#"
        (function_definition
            name: (identifier) @function.name) @function.definition
    "#,
    query_calls: r#"
        (call_expression
            function: (identifier) @call.name)
    "#,
    query_docs: r#"
        (
          (block_comment) @function.docs
          .
          (function_definition) @function.definition
        )
    "#,
};

pub const R_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::R,
    file_extensions: &["R", "r"],
    query_defs: r#"
        (function_definition
            name: (identifier) @function.name) @function.definition
    "#,
    query_calls: r#"
        (call_expression
            function: (identifier) @call.name)
    "#,
    query_docs: r#"
        (
          (comment)+ @function.docs
          .
          (function_definition) @function.definition
        )
    "#,
};

// NOTE: Parsing function calls from HTML is non-trivial. This implementation
// will only find globally-defined JavaScript functions within <script> tags.
// It will not parse inline event handlers or complex module scripts effectively.
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
};


// A central list of all supported language configurations.
pub fn get_language_configs() -> Vec<&'static LanguageConfig> {
    vec![
        &RUST_CONFIG,
        &TYPESCRIPT_CONFIG,
        &JAVASCRIPT_CONFIG,
        &PYTHON_CONFIG,
        &JAVA_CONFIG,
        &BASH_CONFIG,
        &JULIA_CONFIG,
        &R_CONFIG,
        &HTML_CONFIG,
    ]
}