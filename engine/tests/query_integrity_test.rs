use blast_radius_engine::analysis::language::{get_language_configs, get_language};
use tree_sitter::Query;

#[test]
fn test_all_queries_compile() {
    let configs = get_language_configs();
    let mut failures = Vec::new();

    for config in configs {
        let lang_enum = config.lang;
        let language = get_language(lang_enum);
        let name = format!("{:?}", lang_enum);

        println!("Checking {}", name);

        // Helper macro to check a specific query field
        macro_rules! check_query {
            ($field:ident, $query_name:expr) => {
                if let Some(source) = config.queries.$field {
                    if let Err(e) = Query::new(&language, source) {
                        failures.push(format!(
                            "[{}] {} Query Failed:\nError: {:?}\nSource: {:.50}...",
                            name, $query_name, e, source.replace('\n', " ")
                        ));
                    }
                }
            };
        }

        check_query!(defs, "Definitions");
        check_query!(calls, "Calls");
        check_query!(docs, "Docs");
        check_query!(imports, "Imports");
        check_query!(exports, "Exports");
        check_query!(literals, "Literals");
        check_query!(implements, "Implements");
        check_query!(config, "Config Keys");
        check_query!(vals, "Vals");
        check_query!(types, "Types");
        check_query!(decorators, "Decorators");
        check_query!(actions, "Actions");
        check_query!(middleware, "Middleware");
        check_query!(route_defs, "Routes");
    }

    if !failures.is_empty() {
        panic!("Query Integrity Failures:\n\n{}", failures.join("\n\n"));
    }
}