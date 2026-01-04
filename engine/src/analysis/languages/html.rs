use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::Html,
        &["html", "htm", "xhtml"]
    )
    .skeleton("<!-- ... {} ... -->")
    // Preserved logic: treat elements with 'id' attributes as definitions
    .defs(r#"(attribute (attribute_name) @attr (#eq? @attr "id") (attribute_value) @function.name) @function.definition"#)
    
    // Capture script sources and link hrefs as imports
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