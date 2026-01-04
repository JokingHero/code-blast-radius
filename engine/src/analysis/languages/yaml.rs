use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::Yaml,
        &["yaml", "yml"]
    )
    .skeleton("")
    .defs(r#"
        (block_mapping_pair
          key: (_) @function.name
          value: (_) @function.body
        ) @function.definition
    "#)
    .docs(r#"
        (
            (comment) @function.docs
            .
            (block_mapping_pair) @function.definition
        )
    "#)
    // We want to extract all values as literals so we can see 
    // if code literals match config values (e.g., "production")
    .literals(r#"
        [
            (string_scalar) 
            (double_quote_scalar) 
            (single_quote_scalar)
        ] @string
    "#)
    .vals(r#"
        ;; Standard key: value
        (block_mapping_pair
            key: (flow_node (plain_scalar (string_scalar) @val.name))
            value: (flow_node (plain_scalar (string_scalar) @val.value))
        )
        
        ;; Docker Compose / K8s "environment:" list syntax
        ;; - NAME=VALUE
        (block_sequence_item
            (flow_node (plain_scalar (string_scalar) @env_pair))
            (#match? @env_pair "=")
        ) @val.name ;; We will need to split this string in analyzer, or accept fuzzy match
    "#).build()
}