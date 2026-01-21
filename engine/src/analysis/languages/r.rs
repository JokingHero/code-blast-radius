use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::R,
        &["R", "r", "Rscript"]
    )
    .skeleton("{}")
    .defs(r#"
        [
            ;; R functions are assignments: f <- function() {}
            (binary_operator
                lhs: (identifier) @function.name
                rhs: (function_definition)
            ) @function.definition
        ]
    "#)
    // --- NEW: Inference Engine Hooks ---
    .frameworks(r#"
        ;; --- SHINY APPS ---
        ;; Captures: ui <- fluidPage(...) -> key="fluidPage", value="ui"
        (binary_operator
            lhs: (identifier) @framework.value
            rhs: (call
                function: (identifier) @framework.key
            )
            (#eq? @framework.key "fluidPage")
        )
    "#)
    // --- EXISTING LOGIC ---
    .calls(r#"
        [
            (call function: (identifier) @call.name)
            (call 
                function: (namespace_operator 
                    rhs: (identifier) @call.name
                )
            )
        ]
    "#)
    .docs(r#"
        (
            (comment)+ @function.docs
            .
            [
                (binary_operator rhs: (function_definition))
            ] @function.definition
        )
    "#)
    .imports(r#"
        [
            (call
                function: (identifier) @fn
                arguments: (arguments [(identifier) (string)] @import.source)
                (#match? @fn "^(library|require)$")
            )
            (call
                function: (identifier) @fn
                arguments: (arguments (string) @import.source)
                (#eq? @fn "source")
            )
        ]
    "#)
    .literals(r#"(string) @string"#)
    .vals(r#"
        [
            (binary_operator
                lhs: (identifier) @val.name
                rhs: (string) @val.value
            )
        ]
    "#)
    .build()
}