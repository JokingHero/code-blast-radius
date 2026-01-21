use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::Html,
        &["html", "htm", "xhtml"]
    )
    .skeleton("<!-- ... {} ... -->")
    // 1. IDs are definitions (for #fragment links)
    .defs(r#"(attribute (attribute_name) @attr (#eq? @attr "id") (attribute_value) @function.name) @function.definition"#)
    
    // 2. Custom Elements (with hyphens) are CALLS to components
    // This allows walker.rs to link <app-user> to the TS definition "html:tag:app-user"
    .calls(r#"
        (start_tag
            (tag_name) @call.name
            (#match? @call.name "-")
        )
    "#)
    
    // 3. Script/Link tags are IMPORTS
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
    
    .literals(r#"(attribute_value) @string"#)
    .build()
}