use crate::language::{LanguageConfig, SupportedLanguage};

pub const JAVA_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::Java,
    file_extensions: &["java"],
    // UPDATED: Now captures classes, interfaces, enums, and annotations
    query_defs: r#"
        [
            (class_declaration name: (identifier) @function.name) @function.definition
            (interface_declaration name: (identifier) @function.name) @function.definition
            (annotation_type_declaration name: (identifier) @function.name) @function.definition
            (enum_declaration name: (identifier) @function.name) @function.definition
            (method_declaration name: (identifier) @function.name) @function.definition
        ]
    "#,
    query_calls: r#"(method_invocation name: (identifier) @call.name)"#,
    // UPDATED: Docs support for all types
    query_docs: r#"
        (
            (block_comment) @function.docs 
            . 
            [
                (class_declaration)
                (interface_declaration)
                (annotation_type_declaration)
                (enum_declaration)
                (method_declaration)
            ] @function.definition
        )
    "#,
    query_imports: r#"
        (import_declaration
            (scoped_identifier) @import.source
        )
    "#,
    query_exports: "", 
    query_literals: r#"(string_literal) @string"#,
    query_implements: r#"
        (class_declaration
            name: (identifier) @impl.child
            superclass: (superclass (type_identifier) @impl.parent)?
            interfaces: (super_interfaces (type_identifier) @impl.parent)?
        )
        (interface_declaration
            name: (identifier) @impl.child
            extends_interfaces: (extends_interfaces (type_identifier) @impl.parent)?
        )
    "#,
    query_config: "",
    query_vals: r#"
        (variable_declarator
            name: (identifier) @val.name
            value: (string_literal) @val.value
        )
    "#,
    query_types: r#"
        [
            (formal_parameter type: (type_identifier) @type.ref)
            (method_declaration type: (type_identifier) @type.ref)
            (field_declaration type: (type_identifier) @type.ref)
        ]
    "#,
    query_decorators: r#"
        (marker_annotation name: (identifier) @decorator.name)
        (annotation 
            name: (identifier) @decorator.name
            arguments: (_)?
        )
    "#,
    query_actions: "",
    di_decorators: &["Service", "Component", "Repository", "Controller", "Bean", "Configuration"],
};