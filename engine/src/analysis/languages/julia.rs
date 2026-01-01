use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::Julia,
        &["jl"]
    )
    .defs(r#"
        [
            (function_definition name: (identifier) @function.name) @function.definition
            (macro_definition name: (identifier) @function.name) @function.definition
            (struct_definition name: (identifier) @function.name) @function.definition
            (abstract_definition name: (identifier) @function.name) @function.definition
            (module_definition name: (identifier) @function.name) @function.definition
            
            ;; Short-form function definition: f(x) = x + 1
            ;; We differentiate this from variable assignment by checking if 'left' is a call
            (assignment 
                left: (call_expression function: (identifier) @function.name)
            ) @function.definition
        ]
    "#)
    .calls(r#"
        [
            (call_expression function: (identifier) @call.name)
            (call_expression function: (field_expression field: (identifier) @call.name))
            (macro_call_expression name: (macro_identifier) @call.name)
        ]
    "#)
    .docs(r#"
        (
            (string_literal) @function.docs
            .
            [
                (function_definition)
                (macro_definition)
                (struct_definition)
                (module_definition)
                (assignment)
            ] @function.definition
        )
    "#)
    .imports(r#"
        [
            (using_statement (identifier) @import.source)
            (import_statement (identifier) @import.source)
            ;; relative imports: using .SubModule
            (using_statement (scoped_identifier) @import.source)
        ]
    "#)
    .exports(r#"
        (export_statement (identifier) @export.name)
    "#)
    .literals(r#"
        [
            (string_literal)
            (cmd_string)
        ] @string
    "#)
    .vals(r#"
        [
            (const_statement
                (assignment
                    left: (identifier) @val.name
                    right: (string_literal) @val.value
                )
            )
            (assignment
                left: (identifier) @val.name
                right: (string_literal) @val.value
            )
        ]
    "#)
    .types(r#"
        [
            (typed_expression type: (identifier) @type.ref)
            (struct_definition 
                (field_declaration type: (identifier) @type.ref)
            )
        ]
    "#)
    .build()
}