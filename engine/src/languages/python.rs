use crate::language::{LanguageConfig, SupportedLanguage};

pub const PYTHON_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::Python,
    file_extensions: &["py"],
    query_defs: r#"
        [
            (function_definition name: (identifier) @function.name) @function.definition
            (class_definition name: (identifier) @function.name) @function.definition
        ]
    "#,
    query_calls: r#"(call function: [(identifier) @call.name (attribute attribute: (identifier) @call.name)])"#,
    query_docs: r#"
        [
            (function_definition body: (block . (expression_statement (string) @function.docs))) @function.definition
            (class_definition body: (block . (expression_statement (string) @function.docs))) @function.definition
        ]
    "#,
    query_imports: r#"
        [
            (import_statement 
                name: (dotted_name) @import.source
            )
            (import_from_statement 
                module_name: (dotted_name) @import.source
                name: (dotted_name) @import.name
            )
            (import_from_statement
                module_name: (dotted_name) @import.source
                name: (aliased_import 
                    name: (dotted_name) @import.name
                    alias: (identifier) @import.alias
                )
            )
        ]
    "#,
    query_exports: "",
    query_literals: r#"(string) @string"#,
    query_implements: r#"
        (class_definition
            name: (identifier) @impl.child
            superclasses: (argument_list (identifier) @impl.parent)
        )
    "#,
    query_config: "",
    query_vals: r#"
        (assignment
            left: (identifier) @val.name
            right: (string) @val.value
        )
    "#,
    // Type reference extraction
    query_types: r#"
        [
            ;; def foo(x: MyType)
            (typed_parameter type: (_) @type.ref)
            
            ;; def foo(x: MyType = 1)
            (typed_default_parameter type: (_) @type.ref)

            ;; -> MyType
            (function_definition return_type: (_) @type.ref)

            ;; x: MyType = 1  (assignment with type field)
            (assignment type: (_) @type.ref)

            ;; class MyClass(BaseClass):
            (class_definition superclasses: (argument_list (_) @type.ref))
        ]
    "#,
};