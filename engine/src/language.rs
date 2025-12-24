use tree_sitter::Language;
use crate::languages; 

#[derive(Clone, Copy, Debug, PartialEq)]
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

/// Central mapping from the enum to the actual Tree-Sitter grammar object.
/// This is used by the Analyzer to initialize the parser.
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

/// The structure used by the Indexer to know how to parse a specific file type.
pub struct LanguageConfig {
    pub lang_enum: SupportedLanguage,
    pub file_extensions: &'static [&'static str],
    pub query_defs: &'static str,
    pub query_calls: &'static str,
    pub query_docs: &'static str,
    pub query_imports: &'static str,
    pub query_literals: &'static str,
    pub query_implements: &'static str,
}

/// Collects all configurations from the sub-modules in the `languages/` folder.
/// This is used by the Indexer to build its extension-to-config map.
pub fn get_language_configs() -> Vec<&'static LanguageConfig> {
    vec![
        &languages::rust::RUST_CONFIG,
        &languages::typescript::TYPESCRIPT_CONFIG,
        &languages::javascript::JAVASCRIPT_CONFIG,
        &languages::python::PYTHON_CONFIG,
        &languages::java::JAVA_CONFIG,
        &languages::bash::BASH_CONFIG,
        &languages::julia::JULIA_CONFIG,
        &languages::r::R_CONFIG,
        &languages::html::HTML_CONFIG,
    ]
}