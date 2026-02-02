use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(SupportedLanguage::Julia, &["jl"])
        .skeleton("# ... {} ...")
        .defs(
            r#"
        [
            ;; 1. Standard Function Definitions
            ;; Matches: function foo() ... end
            ;; Matches: function move!(...) where T ... end
            (function_definition
                (signature 
                    [
                        (call_expression (identifier) @function.name)
                        (call_expression (field_expression) @function.name)
                        (where_expression 
                            [
                                (call_expression (identifier) @function.name)
                                (call_expression (field_expression) @function.name)
                            ]
                        )
                    ]
                )
                (_) @function.body
            ) @function.definition

            ;; 2. Short-form Assignment Function: f(x) = ...
            ;; Note: 'assignment' node has no fields in this grammar, so we match children positions.
            ;; Structure: (assignment (call_expression) (operator) (body))
            (assignment
                [
                    (call_expression (identifier) @function.name)
                    (call_expression (field_expression) @function.name)
                    (where_expression
                        [
                            (call_expression (identifier) @function.name)
                            (call_expression (field_expression) @function.name)
                        ]
                    )
                ]
                (operator)
                (_) @function.body
            ) @function.definition

            ;; 3. Macros
            (macro_definition
                (signature (call_expression (identifier) @function.name))
                (_) @function.body
            ) @function.definition

            ;; 4. Modules
            (module_definition
                name: (identifier) @function.name
                (_) @function.body
            ) @function.definition

            ;; 5. Structs
            ;; Simple: struct Foo ... end
            ;; Parametric: struct Foo{T} ... end
            (struct_definition
                (type_head
                    [
                        (identifier) @function.name
                        (parametrized_type_expression
                            (identifier) @function.name
                        )
                    ]
                )
                (_) @function.body
            ) @function.definition
        ]
    "#,
        )
        // --- INFERENCE HOOKS ---
        .frameworks(
            r#"
        ;; Genie.jl Routes
        (call_expression
            (identifier) @framework.key
            (argument_list (string_literal) @framework.value)
            (#eq? @framework.key "route")
        )
        (macro_call_expression
            (identifier) @framework.key
            (argument_list (string_literal) @framework.value)
            (#eq? @framework.key "@route")
        )
    "#,
        )
        .calls(
            r#"
        [
            (call_expression (identifier) @call.name)
            (call_expression (field_expression) @call.name)
        ]
    "#,
        )
        .docs(
            r#"
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
    "#,
        )
        .imports(
            r#"
        [
            (using_statement (identifier) @import.source)
            (import_statement (identifier) @import.source)
            (using_statement (scoped_identifier) @import.source)
        ]
    "#,
        )
        .exports(r#"(export_statement (identifier) @export.name)"#)
        .literals(r#"[(string_literal)] @string"#)
        .vals(r#"(assignment (identifier) @val.name (operator) (string_literal) @val.value)"#)
        .types(r#"[(typed_expression (identifier) @type.ref)]"#)
        .build()
}