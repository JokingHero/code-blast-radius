use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::R,
        &["R", "r", "Rscript"]
    )
    .defs(r#"
        [
            ;; R functions are assignments: f <- function() {}
            (binary_operator
                lhs: (identifier) @function.name
                rhs: (function_definition)
            ) @function.definition
        ]
    "#)
    .calls(r#"
        [
            (call function: (identifier) @call.name)
            
            ;; Capture namespaced calls: pkg::func()
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
            ;; library(dplyr) or require(data.table)
            (call
                function: (identifier) @fn
                arguments: (arguments [(identifier) (string)] @import.source)
                (#match? @fn "^(library|require)$")
            )
            ;; source("utils.R")
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