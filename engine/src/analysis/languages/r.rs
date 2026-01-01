use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::R,
        &["R", "r", "Rscript"]
    )
    .defs(r#"
        [
            ;; R functions are assignments: f <- function() {}
            (left_assignment
                name: (identifier) @function.name
                value: (function_definition)
            ) @function.definition
            
            (equals_assignment
                name: (identifier) @function.name
                value: (function_definition)
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
                (left_assignment value: (function_definition))
                (equals_assignment value: (function_definition))
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
            (left_assignment
                name: (identifier) @val.name
                value: (string) @val.value
            )
            (equals_assignment
                name: (identifier) @val.name
                value: (string) @val.value
            )
        ]
    "#)
    .build()
}