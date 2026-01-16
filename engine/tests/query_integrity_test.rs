use blast_radius_engine::analysis::language::{get_language_configs, SupportedLanguage};

#[test]
fn test_core_queries_compile() {
    let configs = get_language_configs();
    let mut failures = Vec::new();

    for config in configs {
        let name = format!("{:?}", config.lang);
        
        // 1. Definitions Query
        // Every language module in the current codebase defines a 'defs' query.
        // If this is None, it means the TreeSitter query string failed to compile during the build step.
        if config.queries.definitions.is_none() {
            failures.push(format!("[{}] Definitions query failed to compile (or is missing)", name));
        }

        // 2. References Query
        // This is auto-injected by the LanguageConfigBuilder using a generic identifier matcher.
        // If this fails, it usually means the underlying TreeSitter Grammar (language crate) 
        // does not support the node types used in the generic query (e.g. 'identifier').
        if config.queries.references.is_none() {
            failures.push(format!("[{}] References query failed to compile", name));
        }

        // 3. Imports Query
        // Not all languages have imports (e.g. JSON, YAML, TOML).
        // However, for languages that DO, we want to ensure they compiled.
        // We use an explicit match list for languages where we expect imports.
        let expects_imports = match config.lang {
            SupportedLanguage::Json | 
            SupportedLanguage::Yaml | 
            SupportedLanguage::Toml | 
            SupportedLanguage::Sql | 
            SupportedLanguage::Dotenv |
            SupportedLanguage::Prisma |
            SupportedLanguage::Hcl => false,
            _ => true,
        };

        if expects_imports && config.queries.imports.is_none() {
             // Note: This might also trigger if a language legitimately hasn't implemented imports yet,
             // but identifying it as a failure helps track what is unfinished or broken.
             failures.push(format!("[{}] Imports query failed to compile (or is missing)", name));
        }
    }

    if !failures.is_empty() {
        panic!("Query Integrity Check Failed:\n\nThe following queries returned None, indicating syntax errors in the TreeSitter query strings:\n\n{}", failures.join("\n"));
    }
}