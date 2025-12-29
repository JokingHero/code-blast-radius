use crate::language::{LanguageConfig, SupportedLanguage};

pub const YAML_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::Yaml,
    file_extensions: &["yaml", "yml"],
    // In tree-sitter-yaml, keys are usually part of a block_mapping_pair
    query_defs: r#"
        (block_mapping_pair
          key: (_) @function.name
        ) @function.definition
    "#,
    query_calls: "",
    query_docs: r#"
        (
            (comment) @function.docs
            .
            (block_mapping_pair) @function.definition
        )
    "#,
    query_imports: "",
    query_exports: "",
    // We want to extract all values as literals so we can see 
    // if code literals match config values (e.g., "production")
    query_literals: r#"
        [
            (string_scalar) 
            (double_quote_scalar) 
            (single_quote_scalar)
        ] @string
    "#,
    query_implements: "",
    query_config: "",
    query_vals: "",
    query_types: "",
    query_decorators: "",
    query_actions: "",
    query_middleware: "",
    di_decorators: &[],
    magic_methods: &[]
};