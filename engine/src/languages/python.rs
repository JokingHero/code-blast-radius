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

            ;; --- Dynamic Import Support ---

            ;; importlib.import_module("literal")
            (call
                function: (attribute
                    object: (identifier) @obj (#eq? @obj "importlib")
                    attribute: (identifier) @meth (#eq? @meth "import_module")
                )
                arguments: (argument_list (string) @import.source)
            )

            ;; importlib.import_module(variable) -> @import.dynamic
            (call
                function: (attribute
                    object: (identifier) @obj (#eq? @obj "importlib")
                    attribute: (identifier) @meth (#eq? @meth "import_module")
                )
                arguments: (argument_list (identifier) @import.dynamic)
            )

            ;; __import__("literal")
            (call
                function: (identifier) @fn (#eq? @fn "__import__")
                arguments: (argument_list (string) @import.source)
            )
             
            ;; __import__(variable) -> @import.dynamic
            (call
                function: (identifier) @fn (#eq? @fn "__import__")
                arguments: (argument_list (identifier) @import.dynamic)
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
    query_types: r#"
        [
            (typed_parameter type: (_) @type.ref)
            (typed_default_parameter type: (_) @type.ref)
            (function_definition return_type: (_) @type.ref)
            (assignment type: (_) @type.ref)
            (class_definition superclasses: (argument_list (_) @type.ref))
        ]
    "#,
    query_decorators: r#"
        (decorator) @decorator.name
    "#,
    query_actions: r#"
        ;; --- Dispatchers ---
        (call
            function: (attribute attribute: (identifier) @fn (#match? @fn "^(send|emit|dispatch|publish)$"))
            arguments: (argument_list 
                [(string) (identifier)] @action.dispatch
            )
        )

        ;; --- Handlers (Decorators) ---
        (decorator
            (call
                function: (identifier) @fn (#match? @fn "^(receiver|on|subscribe)$")
                arguments: (argument_list [(string) (identifier)] @action.handle)
            )
        )
        
        ;; --- Handlers (Comparisons) ---
        (comparison_operator
            (identifier)
            [(string) (identifier)] @action.handle
        )
        (comparison_operator
            [(string) (identifier)] @action.handle
            (identifier)
        )
    "#,
    di_decorators: &["dataclass", "Component", "Service"],
};