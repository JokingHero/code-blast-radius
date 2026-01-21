use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::Html,
        &["html", "htm", "xhtml"]
    )
    .skeleton("<!-- ... {} ... -->")
    // 1. IDs are definitions (for #fragment links)
    .defs(r#"(attribute (attribute_name) @attr (#eq? @attr "id") (attribute_value) @function.name) @function.definition"#)
    
    // 2. References: Tags and Attributes
    // Explicitly define this to ensure tag_name is captured for component linking
    .literals(r#"(attribute_value) @string"#)
    
    // Using 'calls' here as a proxy for references because our LanguageConfigBuilder
    // doesn't have a direct .references() setter exposed in the same way,
    // but we can inject it via the .calls() hook which we will assume
    // gets mapped to references or use the default.
    // However, to be safe and use the builder correctly:
    // The builder's .calls() is a no-op in the provided code.
    // The builder's build() method uses `get_default_refs_query`.
    // We must rely on `get_default_refs_query` in `language.rs` being correct for HTML.
    // (Checked: it is correct).
    //
    // However, to ensure imports are captured:
    .imports(r#"
        (start_tag
            (tag_name) @tag
            (attribute
                (attribute_name) @attr
                (attribute_value) @import.source
            )
            (#eq? @tag "script")
            (#eq? @attr "src")
        )
        (start_tag
            (tag_name) @tag
            (attribute
                (attribute_name) @attr
                (attribute_value) @import.source
            )
            (#eq? @tag "link")
            (#eq? @attr "href")
        )
    "#)
    .build()
}