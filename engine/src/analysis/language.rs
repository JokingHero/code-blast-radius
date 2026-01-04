use tree_sitter::{Language, Query};
use std::sync::Arc;
use crate::analysis::languages;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SupportedLanguage {
    Rust, TypeScript, Python, Java, JavaScript, Bash, Html,
    Julia, R, Json, Yaml, Toml, Dotenv, Sql, Prisma, Hcl,
    Go, CSharp, Php, Ruby, C, Cpp,
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
        SupportedLanguage::Go => tree_sitter_go::LANGUAGE.into(),
        SupportedLanguage::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
        SupportedLanguage::Php => tree_sitter_php::LANGUAGE_PHP.into(),
        SupportedLanguage::Ruby => tree_sitter_ruby::LANGUAGE.into(),
        SupportedLanguage::C => tree_sitter_c::LANGUAGE.into(),
        SupportedLanguage::Cpp => tree_sitter_cpp::LANGUAGE.into(),
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

/// Holds compiled Tree-Sitter queries for a language.
#[derive(Clone, Default)]
pub struct CompiledQueries {
    pub defs: Option<Arc<Query>>,
    pub calls: Option<Arc<Query>>,
    pub docs: Option<Arc<Query>>,
    pub imports: Option<Arc<Query>>,
    pub exports: Option<Arc<Query>>,
    pub literals: Option<Arc<Query>>,
    pub implements: Option<Arc<Query>>,
    pub config: Option<Arc<Query>>,
    pub vals: Option<Arc<Query>>,
    pub types: Option<Arc<Query>>,
    pub decorators: Option<Arc<Query>>,
    pub actions: Option<Arc<Query>>,
    pub middleware: Option<Arc<Query>>,
    pub route_defs: Option<Arc<Query>>,
}

/// Groups heuristic lists for framework analysis.
#[derive(Default, Clone)]
pub struct HeuristicConfig {
    pub di_decorators: &'static [&'static str],
    pub magic_methods: &'static [&'static str],
    pub constructor_names: &'static [&'static str],
    pub project_config_files: &'static [&'static str],
}

/// The main configuration struct, now cleaner and grouped.
#[derive(Clone)]
pub struct LanguageConfig {
    pub lang: SupportedLanguage,
    pub file_extensions: &'static [&'static str],
    pub queries: QueryConfig,
    pub compiled_queries: CompiledQueries,
    pub heuristics: HeuristicConfig,
    pub skeleton_template: &'static str,
}

// --- Builder Pattern ---

pub struct LanguageConfigBuilder {
    lang: SupportedLanguage,
    file_extensions: &'static [&'static str],
    queries: QueryConfig,
    heuristics: HeuristicConfig,
    skeleton_template: &'static str,
}

impl LanguageConfigBuilder {
    pub fn new(lang: SupportedLanguage, extensions: &'static [&'static str]) -> Self {
        Self {
            lang,
            file_extensions: extensions,
            queries: QueryConfig::default(),
            heuristics: HeuristicConfig::default(),
            // Default safe fallback for C-like languages
            skeleton_template: "{ /* ... {} body hidden ... */ }",
        }
    }

    // --- Query Setters ---

    pub fn defs(mut self, query: &'static str) -> Self { self.queries.defs = Some(query); self }
    pub fn calls(mut self, query: &'static str) -> Self { self.queries.calls = Some(query); self }
    pub fn docs(mut self, query: &'static str) -> Self { self.queries.docs = Some(query); self }
    pub fn imports(mut self, query: &'static str) -> Self { self.queries.imports = Some(query); self }
    pub fn exports(mut self, query: &'static str) -> Self { self.queries.exports = Some(query); self }
    pub fn literals(mut self, query: &'static str) -> Self { self.queries.literals = Some(query); self }
    pub fn implements(mut self, query: &'static str) -> Self { self.queries.implements = Some(query); self }
    pub fn config_keys(mut self, query: &'static str) -> Self { self.queries.config = Some(query); self }
    pub fn vals(mut self, query: &'static str) -> Self { self.queries.vals = Some(query); self }
    pub fn types(mut self, query: &'static str) -> Self { self.queries.types = Some(query); self }
    pub fn decorators(mut self, query: &'static str) -> Self { self.queries.decorators = Some(query); self }
    pub fn actions(mut self, query: &'static str) -> Self { self.queries.actions = Some(query); self }
    pub fn middleware(mut self, query: &'static str) -> Self { self.queries.middleware = Some(query); self }
    pub fn routes(mut self, query: &'static str) -> Self { self.queries.route_defs = Some(query); self }

    // --- Heuristic Setters ---

    pub fn di_decorators(mut self, decorators: &'static [&'static str]) -> Self {
        self.heuristics.di_decorators = decorators;
        self
    }

    pub fn magic_methods(mut self, methods: &'static [&'static str]) -> Self {
        self.heuristics.magic_methods = methods;
        self
    }

    pub fn constructor_names(mut self, constructors: &'static [&'static str]) -> Self {
        self.heuristics.constructor_names = constructors;
        self
    }

    pub fn project_config_files(mut self, files: &'static [&'static str]) -> Self {
        self.heuristics.project_config_files = files;
        self
    }

    pub fn skeleton(mut self, template: &'static str) -> Self {
        self.skeleton_template = template;
        self
    }

    pub fn build(self) -> LanguageConfig {
        let language = get_language(self.lang);
        let mut compiled = CompiledQueries::default();

        macro_rules! compile_query {
            ($field:ident) => {
                if let Some(q_str) = self.queries.$field {
                    compiled.$field = Query::new(&language, q_str).ok().map(Arc::new);
                }
            };
        }

        compile_query!(defs);
        compile_query!(calls);
        compile_query!(docs);
        compile_query!(imports);
        compile_query!(exports);
        compile_query!(literals);
        compile_query!(implements);
        compile_query!(config);
        compile_query!(vals);
        compile_query!(types);
        compile_query!(decorators);
        compile_query!(actions);
        compile_query!(middleware);
        compile_query!(route_defs);

        LanguageConfig {
            lang: self.lang,
            file_extensions: self.file_extensions,
            queries: self.queries,
            compiled_queries: compiled,
            heuristics: self.heuristics,
            skeleton_template: self.skeleton_template,
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
        languages::go::config(),
        languages::c_sharp::config(),
        languages::php::config(),
        languages::ruby::config(),
        languages::c::config(),
        languages::cpp::config(),
    ]
}