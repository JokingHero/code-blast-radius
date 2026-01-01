use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::Java,
        &["java"]
    )
    .defs(r#"
        [
            (class_declaration name: (identifier) @function.name) @function.definition
            (interface_declaration name: (identifier) @function.name) @function.definition
            (enum_declaration name: (identifier) @function.name) @function.definition
            (annotation_type_declaration name: (identifier) @function.name) @function.definition
            (method_declaration name: (identifier) @function.name) @function.definition
            (constructor_declaration name: (identifier) @function.name) @function.definition
            (record_declaration name: (identifier) @function.name) @function.definition
        ]
    "#)
    .calls(r#"
        [
            (method_invocation name: (identifier) @call.name)
            (object_creation_expression type: (type_identifier) @call.name)
        ]
    "#)
    .imports(r#"
        (import_declaration
            (scoped_identifier) @import.source
        )
    "#)
    .docs(r#"
        (
            (block_comment) @function.docs 
            . 
            [
                (class_declaration)
                (interface_declaration)
                (annotation_type_declaration)
                (enum_declaration)
                (method_declaration)
                (constructor_declaration)
                (record_declaration)
            ] @function.definition
        )
    "#)
    .literals(r#"(string_literal) @string"#)
    .implements(r#"
        (class_declaration
            name: (identifier) @impl.child
            superclass: (superclass (type_identifier) @impl.parent)?
            interfaces: (super_interfaces (type_identifier) @impl.parent)?
        )
        (interface_declaration
            name: (identifier) @impl.child
            extends_interfaces: (extends_interfaces (type_identifier) @impl.parent)?
        )
        (record_declaration
            name: (identifier) @impl.child
            interfaces: (super_interfaces (type_identifier) @impl.parent)?
        )
    "#)
    .vals(r#"
        (variable_declarator
            name: (identifier) @val.name
            value: (string_literal) @val.value
        )
    "#)
    .types(r#"
        [
            (formal_parameter type: (type_identifier) @type.ref)
            (method_declaration type: (type_identifier) @type.ref)
            (field_declaration type: (type_identifier) @type.ref)
            (local_variable_declaration type: (type_identifier) @type.ref)
        ]
    "#)
    .decorators(r#"
        (marker_annotation name: (identifier) @decorator.name)
        (annotation 
            name: (identifier) @decorator.name
            arguments: (_)?
        )
    "#)
    .di_decorators(&[
        "Service", "Component", "Repository", "Controller", 
        "RestController", "Bean", "Configuration", "Inject", "Autowired"
    ])
    .build()
}