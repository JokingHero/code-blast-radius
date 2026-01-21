use tree_sitter::{ Language, Query };
use std::sync::{ Arc, OnceLock };
use crate::analysis::languages;

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
    Json,
    Yaml,
    Toml,
    Dotenv,
    Sql,
    Prisma,
    Hcl,
    Go,
    CSharp,
    Php,
    Ruby,
    C,
    Cpp,
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

#[derive(Clone, Default)]
pub struct CompiledQueries {
    pub definitions: Option<Arc<Query>>,
    pub imports: Option<Arc<Query>>,
    pub references: Option<Arc<Query>>,
    pub literals: Option<Arc<Query>>,
    pub frameworks: Option<Arc<Query>>,
}

#[derive(Default, Clone)]
pub struct HeuristicConfig {
    pub di_decorators: &'static [&'static str],
    pub magic_methods: &'static [&'static str],
    pub constructor_names: &'static [&'static str],
    pub project_config_files: &'static [&'static str],
}

#[derive(Clone)]
pub struct LanguageConfig {
    pub lang: SupportedLanguage,
    pub file_extensions: &'static [&'static str],
    pub queries: CompiledQueries,
    pub heuristics: HeuristicConfig,
    pub skeleton_template: &'static str,
}

pub struct LanguageConfigBuilder {
    lang: SupportedLanguage,
    file_extensions: &'static [&'static str],

    // Primary Queries
    defs_query: Option<&'static str>,
    imports_query: Option<&'static str>,
    literals_query: Option<&'static str>,
    frameworks_query: Option<&'static str>,

    // Heuristics (Keep for compatibility, though largely unused in dumb mode)
    heuristics: HeuristicConfig,
    skeleton_template: &'static str,
}

impl LanguageConfigBuilder {
    pub fn new(lang: SupportedLanguage, extensions: &'static [&'static str]) -> Self {
        Self {
            lang,
            file_extensions: extensions,
            defs_query: None,
            imports_query: None,
            literals_query: None,
            frameworks_query: None,
            heuristics: HeuristicConfig::default(),
            skeleton_template: " ... ",
        }
    }

    // --- Core Setters ---
    pub fn defs(mut self, q: &'static str) -> Self {
        self.defs_query = Some(q);
        self
    }
    pub fn imports(mut self, q: &'static str) -> Self {
        self.imports_query = Some(q);
        self
    }
    pub fn skeleton(mut self, s: &'static str) -> Self {
        self.skeleton_template = s;
        self
    }

    // This allows us to use the complex regexes/node-types defined in your language files
    // (e.g., heredocs in Bash, template strings in JS).
    pub fn literals(mut self, q: &'static str) -> Self {
        self.literals_query = Some(q);
        self
    }
    pub fn frameworks(mut self, q: &'static str) -> Self {
        self.frameworks_query = Some(q);
        self
    }

    // --- Legacy Setters (No-ops) ---
    // We keep these so that existing language files (which are very detailed)
    // compile without changes, even if we don't use this metadata currently.
    pub fn calls(self, _q: &'static str) -> Self {
        self
    }
    pub fn docs(self, _q: &'static str) -> Self {
        self
    }
    pub fn exports(self, _q: &'static str) -> Self {
        self
    }
    pub fn implements(self, _q: &'static str) -> Self {
        self
    }
    pub fn config_keys(self, _q: &'static str) -> Self {
        self
    }
    pub fn vals(self, _q: &'static str) -> Self {
        self
    }
    pub fn types(self, _q: &'static str) -> Self {
        self
    }
    pub fn decorators(self, _q: &'static str) -> Self {
        self
    }
    pub fn actions(self, _q: &'static str) -> Self {
        self
    }
    pub fn middleware(self, _q: &'static str) -> Self {
        self
    }
    pub fn routes(self, _q: &'static str) -> Self {
        self
    }

    // --- Heuristic Setters (Keep Metadata) ---
    pub fn di_decorators(mut self, d: &'static [&'static str]) -> Self {
        self.heuristics.di_decorators = d;
        self
    }
    pub fn magic_methods(mut self, m: &'static [&'static str]) -> Self {
        self.heuristics.magic_methods = m;
        self
    }
    pub fn constructor_names(mut self, c: &'static [&'static str]) -> Self {
        self.heuristics.constructor_names = c;
        self
    }
    pub fn project_config_files(mut self, f: &'static [&'static str]) -> Self {
        self.heuristics.project_config_files = f;
        self
    }

    pub fn build(self) -> LanguageConfig {
        let language = get_language(self.lang);

        let compile = |q: Option<&str>| -> Option<Arc<Query>> {
            q.and_then(|source| { Query::new(&language, source).ok().map(Arc::new) })
        };

        // Inject the generic reference query automatically
        let refs_source = get_default_refs_query(self.lang);
        let refs_query = Query::new(&language, refs_source).ok().map(Arc::new);

        LanguageConfig {
            lang: self.lang,
            file_extensions: self.file_extensions,
            queries: CompiledQueries {
                definitions: compile(self.defs_query),
                imports: compile(self.imports_query),
                references: refs_query,
                literals: compile(self.literals_query),
                frameworks: compile(self.frameworks_query),
            },
            heuristics: self.heuristics,
            skeleton_template: self.skeleton_template,
        }
    }
}

/// Provides a "good enough" generic identifier matcher for every supported language.
/// This replaces the specific 'calls', 'types', 'decorators' queries.
fn get_default_refs_query(lang: SupportedLanguage) -> &'static str {
    match lang {
        SupportedLanguage::Rust =>
            r#"
            (identifier) @ref
            (type_identifier) @ref
            (field_identifier) @ref
        "#,
        SupportedLanguage::TypeScript =>
            r#"
            (identifier) @ref
            (property_identifier) @ref
            (type_identifier) @ref
            (shorthand_property_identifier_pattern) @ref
        "#,
        SupportedLanguage::JavaScript =>
            r#"
            (identifier) @ref
            (property_identifier) @ref
            (shorthand_property_identifier_pattern) @ref
        "#,
        SupportedLanguage::Python =>
            r#"
            (identifier) @ref
            (attribute attribute: (identifier) @ref)
        "#,
        SupportedLanguage::Go =>
            r#"
            (identifier) @ref
            (field_identifier) @ref
            (type_identifier) @ref
            (package_identifier) @ref
        "#,
        SupportedLanguage::Java =>
            r#"
            (identifier) @ref
            (type_identifier) @ref
        "#,
        SupportedLanguage::CSharp => r#"
            (identifier) @ref
        "#,
        SupportedLanguage::Ruby =>
            r#"
            (identifier) @ref
            (constant) @ref
            (simple_symbol) @ref
            (hash_key_symbol) @ref
        "#,
        SupportedLanguage::Php =>
            r#"
            (name) @ref
            (variable_name) @ref
        "#,
        SupportedLanguage::Bash | SupportedLanguage::Dotenv =>
            r#"
            (variable_name) @ref
            (word) @ref
            (command_name) @ref
        "#,
        SupportedLanguage::Html =>
            r#"
            (tag_name) @ref
            (attribute_name) @ref
        "#,
        SupportedLanguage::Json => r#"
            (string_content) @ref
        "#,
        SupportedLanguage::Yaml => r#"
            (string_scalar) @ref
        "#,
        SupportedLanguage::Toml => r#"
            (bare_key) @ref
        "#,
        SupportedLanguage::Hcl => r#"
            (identifier) @ref
        "#,
        SupportedLanguage::Sql => r#"
            (identifier) @ref
        "#,
        SupportedLanguage::Prisma => r#"
            (identifier) @ref
        "#,
        SupportedLanguage::C =>
            r#"
            (identifier) @ref
            (type_identifier) @ref
            (field_identifier) @ref
        "#,
        SupportedLanguage::Cpp =>
            r#"
            (identifier) @ref
            (type_identifier) @ref
            (field_identifier) @ref
            (namespace_identifier) @ref
        "#,
        _ => "(identifier) @ref",
    }
}

pub fn get_config_for_extension(ext: &str) -> Option<&'static LanguageConfig> {
    match ext {
        // Rust
        "rs" => Some(get_rust_config()),

        // JavaScript / TypeScript ecosystem
        "ts" | "tsx" => Some(get_typescript_config()),
        "js" | "jsx" | "mjs" | "cjs" | "vue" => Some(get_javascript_config()),
        "json" => Some(get_json_config()),

        // Python
        "py" | "pyi" | "pyw" => Some(get_python_config()),

        // Backend / Systems
        "java" => Some(get_java_config()),
        "go" => Some(get_go_config()),
        "cs" => Some(get_c_sharp_config()),
        "c" | "h" => Some(get_c_config()),
        "cpp" | "hpp" | "cc" | "cxx" | "hh" => Some(get_cpp_config()),
        "php" => Some(get_php_config()),
        "rb" => Some(get_ruby_config()),

        // Data / Config
        "sql" => Some(get_sql_config()),
        "yaml" | "yml" => Some(get_yaml_config()),
        "toml" => Some(get_toml_config()),
        "tf" | "hcl" | "tfvars" => Some(get_hcl_config()),
        "prisma" => Some(get_prisma_config()),
        | "env"
        | "env.example"
        | "env.template"
        | "env.local"
        | "env.development"
        | "env.test"
        | "env.production" => Some(get_dotenv_config()),

        // Scripts / Other
        "sh" | "bash" | "zsh" => Some(get_bash_config()),
        "html" | "htm" | "xhtml" => Some(get_html_config()),
        "jl" => Some(get_julia_config()),
        "R" | "r" | "Rscript" => Some(get_r_config()),

        _ => None,
    }
}

/// Macro to generate lazy accessors for language configs.
/// This keeps the global namespace clean and avoids 22 blocks of repetitive code.
macro_rules! define_lazy_configs {
    ($($name:ident => $module:ident),* $(,)?) => {
        $(
            fn $name() -> &'static LanguageConfig {
                static CONFIG: OnceLock<LanguageConfig> = OnceLock::new();
                CONFIG.get_or_init(|| languages::$module::config())
            }
        )*
    };
}

// Generate the lazy loaders
define_lazy_configs!(
    get_rust_config => rust,
    get_typescript_config => typescript,
    get_javascript_config => javascript,
    get_python_config => python,
    get_java_config => java,
    get_bash_config => bash,
    get_julia_config => julia,
    get_r_config => r,
    get_html_config => html,
    get_json_config => json,
    get_yaml_config => yaml,
    get_toml_config => toml,
    get_dotenv_config => dotenv,
    get_sql_config => sql,
    get_prisma_config => prisma,
    get_hcl_config => hcl,
    get_go_config => go,
    get_c_sharp_config => c_sharp,
    get_php_config => php,
    get_ruby_config => ruby,
    get_c_config => c,
    get_cpp_config => cpp,
);

/// A list of all supported languages, primarily used for integrity testing.
pub const ALL_LANGUAGES: &[SupportedLanguage] = &[
    SupportedLanguage::Rust,
    SupportedLanguage::TypeScript,
    SupportedLanguage::JavaScript,
    SupportedLanguage::Python,
    SupportedLanguage::Java,
    SupportedLanguage::Go,
    SupportedLanguage::CSharp,
    SupportedLanguage::Ruby,
    SupportedLanguage::Php,
    SupportedLanguage::C,
    SupportedLanguage::Cpp,
    SupportedLanguage::Bash,
    SupportedLanguage::Html,
    SupportedLanguage::Julia,
    SupportedLanguage::R,
    SupportedLanguage::Json,
    SupportedLanguage::Yaml,
    SupportedLanguage::Toml,
    SupportedLanguage::Dotenv,
    SupportedLanguage::Sql,
    SupportedLanguage::Prisma,
    SupportedLanguage::Hcl,
];

/// Helper for tests and internal logic to get a config directly from the Enum.
/// It maps the Enum to a representative file extension and triggers the lazy load.
pub fn get_config_by_language(lang: SupportedLanguage) -> Option<&'static LanguageConfig> {
    let ext = match lang {
        SupportedLanguage::Rust => "rs",
        SupportedLanguage::TypeScript => "ts",
        SupportedLanguage::JavaScript => "js",
        SupportedLanguage::Python => "py",
        SupportedLanguage::Java => "java",
        SupportedLanguage::Bash | SupportedLanguage::Dotenv => "sh",
        SupportedLanguage::Html => "html",
        SupportedLanguage::Julia => "jl",
        SupportedLanguage::R => "r",
        SupportedLanguage::Json => "json",
        SupportedLanguage::Yaml => "yaml",
        SupportedLanguage::Toml => "toml",
        SupportedLanguage::Sql => "sql",
        SupportedLanguage::Prisma => "prisma",
        SupportedLanguage::Hcl => "tf",
        SupportedLanguage::Go => "go",
        SupportedLanguage::CSharp => "cs",
        SupportedLanguage::Php => "php",
        SupportedLanguage::Ruby => "rb",
        SupportedLanguage::C => "c",
        SupportedLanguage::Cpp => "cpp",
    };

    if lang == SupportedLanguage::Dotenv {
        return get_config_for_extension("env");
    }

    get_config_for_extension(ext)
}