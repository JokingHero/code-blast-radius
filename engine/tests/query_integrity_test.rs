use blast_radius_engine::analysis::language::{
    get_config_by_language, SupportedLanguage, ALL_LANGUAGES,
};

#[test]
fn test_core_queries_compile() {
    let mut failures = Vec::new();

    for &lang in ALL_LANGUAGES {
        let config = match get_config_by_language(lang) {
            Some(c) => c,
            None => {
                failures.push(format!("[{:?}] Config not found / failed to load", lang));
                continue;
            }
        };

        let name = format!("{:?}", lang);

        // 1. Definitions Query
        if config.queries.definitions.is_none() {
            failures.push(format!("[{}] Definitions query failed to compile", name));
        }

        // 2. References Query
        if config.queries.references.is_none() {
            failures.push(format!("[{}] References query failed to compile", name));
        }

        // 3. Imports Query
        let expects_imports = match lang {
            SupportedLanguage::Json
            | SupportedLanguage::Yaml
            | SupportedLanguage::Toml
            | SupportedLanguage::Sql
            | SupportedLanguage::Dotenv
            | SupportedLanguage::Prisma
            | SupportedLanguage::Hcl => false,
            _ => true,
        };

        if expects_imports && config.queries.imports.is_none() {
            failures.push(format!("[{}] Imports query failed to compile", name));
        }
    }

    if !failures.is_empty() {
        panic!("Query Integrity Check Failed:\n\n{}", failures.join("\n"));
    }
}
