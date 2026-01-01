use tree_sitter::Language;
use crate::analysis::languages;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SupportedLanguage {
    Rust, TypeScript, Python, Java, JavaScript, Bash, Html,
    Julia, R, Json, Yaml, Toml, Dotenv, Sql, Prisma, Hcl,
}

pub fn get_language(lang: SupportedLanguage) -> Language {
    match lang {
        SupportedLanguage::Rust => tree_sitter_rust::LANGUAGE.into(),
        SupportedLanguage::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        SupportedLanguage::Python => tree_sitter_python::LANGUAGE.into(),
        SupportedLanguage::Java => tree_sitter_java::LANGUAGE.into(),
        SupportedLanguage::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
        SupportedLanguage::Bash | SupportedLanguage::Dotenv => tree_sitter_bash::LANGUAGE.into(),
        SupportedLanguage::Html => tree_sitter_html::LANGUAGE.into(),
        SupportedLanguage::Julia => tree_sitter_julia::LANGUAGE.into(),
        SupportedLanguage::R => tree_sitter_r::LANGUAGE.into(),
        SupportedLanguage::Json => tree_sitter_json::LANGUAGE.into(),
        SupportedLanguage::Yaml => tree_sitter_yaml::LANGUAGE.into(),
        SupportedLanguage::Toml => tree_sitter_toml_ng::LANGUAGE.into(),
        SupportedLanguage::Sql => tree_sitter_sequel::LANGUAGE.into(),
        SupportedLanguage::Prisma => tree_sitter_prisma_io::LANGUAGE.into(),
        SupportedLanguage::Hcl => tree_sitter_hcl::LANGUAGE.into(),
    }
}

/// Groups all Tree-Sitter query strings.
/// Using Option<&str> allows us to skip compiling queries for languages that don't need them.
#[derive(Default, Clone)]
pub struct QueryConfig {
    pub defs: Option<&'static str>,
    pub calls: Option<&'static str>,
    pub docs: Option<&'static str>,
    pub imports: Option<&'static str>,
    pub exports: Option<&'static str>,
    pub literals: Option<&'static str>,
    pub implements: Option<&'static str>,
    pub config: Option<&'static str>,
    pub vals: Option<&'static str>,
    pub types: Option<&'static str>,
    pub decorators: Option<&'static str>,
    pub actions: Option<&'static str>,
    pub middleware: Option<&'static str>,
    pub route_defs: Option<&'static str>,
}

/// Groups heuristic lists for framework analysis.
#[derive(Default, Clone)]
pub struct HeuristicConfig {
    pub di_decorators: &'static [&'static str],
    pub magic_methods: &'static [&'static str],
}

/// The main configuration struct, now cleaner and grouped.
#[derive(Clone)]
pub struct LanguageConfig {
    pub lang: SupportedLanguage,
    pub file_extensions: &'static [&'static str],
    pub queries: QueryConfig,
    pub heuristics: HeuristicConfig,
}

// --- Builder Pattern ---

pub struct LanguageConfigBuilder {
    lang: SupportedLanguage,
    file_extensions: &'static [&'static str],
    queries: QueryConfig,
    heuristics: HeuristicConfig,
}

impl LanguageConfigBuilder {
    pub fn new(lang: SupportedLanguage, extensions: &'static [&'static str]) -> Self {
        Self {
            lang,
            file_extensions: extensions,
            queries: QueryConfig::default(),
            heuristics: HeuristicConfig::default(),
        }
    }

    // --- Query Setters ---

    pub fn defs(mut self, q: &'static str) -> Self { self.queries.defs = Some(q); self }
    pub fn calls(mut self, q: &'static str) -> Self { self.queries.calls = Some(q); self }
    pub fn docs(mut self, q: &'static str) -> Self { self.queries.docs = Some(q); self }
    pub fn imports(mut self, q: &'static str) -> Self { self.queries.imports = Some(q); self }
    pub fn exports(mut self, q: &'static str) -> Self { self.queries.exports = Some(q); self }
    pub fn literals(mut self, q: &'static str) -> Self { self.queries.literals = Some(q); self }
    pub fn implements(mut self, q: &'static str) -> Self { self.queries.implements = Some(q); self }
    pub fn config_keys(mut self, q: &'static str) -> Self { self.queries.config = Some(q); self }
    pub fn vals(mut self, q: &'static str) -> Self { self.queries.vals = Some(q); self }
    pub fn types(mut self, q: &'static str) -> Self { self.queries.types = Some(q); self }
    pub fn decorators(mut self, q: &'static str) -> Self { self.queries.decorators = Some(q); self }
    pub fn actions(mut self, q: &'static str) -> Self { self.queries.actions = Some(q); self }
    pub fn middleware(mut self, q: &'static str) -> Self { self.queries.middleware = Some(q); self }
    pub fn routes(mut self, q: &'static str) -> Self { self.queries.route_defs = Some(q); self }

    // --- Heuristic Setters ---

    pub fn di_decorators(mut self, d: &'static [&'static str]) -> Self {
        self.heuristics.di_decorators = d;
        self
    }

    pub fn magic_methods(mut self, m: &'static [&'static str]) -> Self {
        self.heuristics.magic_methods = m;
        self
    }

    pub fn build(self) -> LanguageConfig {
        LanguageConfig {
            lang: self.lang,
            file_extensions: self.file_extensions,
            queries: self.queries,
            heuristics: self.heuristics,
        }
    }
}

pub fn get_language_configs() -> Vec<LanguageConfig> {
    vec![
        languages::rust::config(),
        languages::typescript::config(),
        languages::javascript::config(),
        languages::python::config(),
        languages::java::config(),
        languages::bash::config(),
        languages::julia::config(),
        languages::r::config(),
        languages::html::config(),
        languages::json::config(),
        languages::yaml::config(),
        languages::toml::config(),
        languages::dotenv::config(),
        languages::sql::config(),
        languages::prisma::config(),
        languages::hcl::config(),
    ]
}