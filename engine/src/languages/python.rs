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
    query_calls: r#"
        [
            (call function: [(identifier) @call.name (attribute attribute: (identifier) @call.name)])
            
            (call
                function: (identifier) @fn
                arguments: (argument_list 
                    (identifier) @call.dynamic_dispatch
                    (_) 
                )
                (#eq? @fn "getattr")
            )
        ]
    "#,
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
            (call
                function: (attribute
                    object: (identifier) @obj 
                    attribute: (identifier) @meth 
                )
                arguments: (argument_list (string) @import.source)
                (#eq? @obj "importlib")
                (#eq? @meth "import_module")
            )
            (call
                function: (attribute
                    object: (identifier) @obj 
                    attribute: (identifier) @meth 
                )
                arguments: (argument_list (identifier) @import.dynamic)
                (#eq? @obj "importlib")
                (#eq? @meth "import_module")
            )
            (call
                function: (identifier) @fn 
                arguments: (argument_list (string) @import.source)
                (#eq? @fn "__import__")
            )
            (call
                function: (identifier) @fn 
                arguments: (argument_list (identifier) @import.dynamic)
                (#eq? @fn "__import__")
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
        (call
            function: (attribute attribute: (identifier) @fn)
            arguments: (argument_list 
                [(string) (identifier)] @action.dispatch
            )
            (#match? @fn "^(send|emit|dispatch|publish)$")
        )
        (decorator
            (call
                function: (identifier) @fn 
                arguments: (argument_list [(string) (identifier)] @action.handle)
            )
            (#match? @fn "^(receiver|on|subscribe)$")
        )
        (comparison_operator
            (identifier)
            [(string) (identifier)] @action.handle
        )
        (comparison_operator
            [(string) (identifier)] @action.handle
            (identifier)
        )
    "#,
    // Middleware Detection
    // 1. Django: MIDDLEWARE = [...]
    // 2. Flask: @app.before_request (no parens) OR @app.before_request() (parens)
    query_middleware: r#"
        (assignment
            left: (identifier) @var 
            right: (list (string) @middleware.config)
            (#eq? @var "MIDDLEWARE")
        )

        ;; Flask: @app.before_request (Attribute, no call)
        (decorated_definition
            (decorator
                (attribute attribute: (identifier) @attr)
            )
            (function_definition name: (identifier) @middleware.use)
            (#eq? @attr "before_request")
        )

        ;; Flask: @app.before_request() (Call inside decorator)
        (decorated_definition
            (decorator
                (call
                    function: (attribute attribute: (identifier) @attr)
                )
            )
            (function_definition name: (identifier) @middleware.use)
            (#eq? @attr "before_request")
        )
    "#,
    di_decorators: &["dataclass", "Component", "Service"],
    magic_methods: &["__getattr__", "__getattribute__"], 
};