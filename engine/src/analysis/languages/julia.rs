use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::Julia,
        &["jl"]
    )
    .skeleton("# ... {} ...")
    .defs(r#"
        [
            (function_definition
                (signature
                    (call_expression
                        (identifier) @function.name
                    )
                )
            ) @function.definition
            (macro_definition
                (signature
                    (call_expression
                        (identifier) @function.name
                    )
                )
            ) @function.definition
        ]
    "#)
    // --- NEW: Inference Engine Hooks ---
    .frameworks(r#"
        ;; --- GENIE.JL ROUTES ---
        ;; Captures: route("/hello") -> key="route", value="/hello"
        (call_expression
            (identifier) @framework.key
            (argument_list
                (string_literal) @framework.value
            )
            (#eq? @framework.key "route")
        )
        
        ;; Captures: @route "/hello" -> key="route", value="/hello"
        (macro_call_expression
            (identifier) @framework.key
            (argument_list
                (string_literal) @framework.value
            )
            (#eq? @framework.key "@route")
        )
    "#)
    // --- EXISTING LOGIC ---
    .calls(r#"
        [
            (call_expression
                (identifier) @call.name
            )
            (call_expression
                (field_expression
                    (identifier) @call.name
                )
            )
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
        ] @string
    "#)
    .vals(r#"
        [
            (assignment
                (identifier) @val.name
                (string_literal) @val.value
            )
        ]
    "#)
    .types(r#"
        [
            (typed_expression
                (identifier) @type.ref
            )
        ]
    "#)
    .build()
}