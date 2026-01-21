use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::Java,
        &["java"]
    )
    .skeleton("{ /* ... {} ... */ }")
    .defs(r#"
        [
            (class_declaration name: (identifier) @function.name body: (class_body) @function.body) @function.definition
            (interface_declaration name: (identifier) @function.name) @function.definition
            (enum_declaration name: (identifier) @function.name) @function.definition
            (annotation_type_declaration name: (identifier) @function.name) @function.definition
            (method_declaration name: (identifier) @function.name body: (block) @function.body) @function.definition
            (constructor_declaration name: (identifier) @function.name) @function.definition
            (record_declaration name: (identifier) @function.name) @function.definition
        ]
    "#)
    .frameworks(r#"
        ;; --- SPRING BOOT / JAKARTA EE ---

        ;; 1. Class Level Marker Annotations (@Service class UserService)
        ;; Context: Key=Service, Value=UserService
        (class_declaration
            (modifiers
                (marker_annotation name: (identifier) @framework.key)
            )
            name: (identifier) @framework.value
            (#match? @framework.key "^(Service|Component|Repository|Controller|RestController|Configuration)$")
        )

        ;; 1b. Class Level Normal Annotations (@Component("myBean") class UserService)
        (class_declaration
            (modifiers
                (annotation name: (identifier) @framework.key)
            )
            name: (identifier) @framework.value
            (#match? @framework.key "^(Service|Component|Repository|Controller|RestController|Configuration)$")
        )

        ;; 2. Annotations with String Argument (@RequestMapping("/api"))
        (annotation
            name: (identifier) @framework.key
            arguments: (annotation_argument_list (string_literal) @framework.value)
        )

        ;; 3. Annotations with Key-Value Arguments (@Table(name="users"))
        (annotation
            name: (identifier) @framework.key
            arguments: (annotation_argument_list
                (element_value_pair
                    key: (identifier) @arg_name
                    value: (string_literal) @framework.value
                )
            )
            (#match? @arg_name "^(value|path|name)$")
        )

        ;; 4. Bean Producers (Methods)
        (method_declaration
            (modifiers (marker_annotation name: (identifier) @framework.key))
            name: (identifier) @framework.value
            (#eq? @framework.key "Bean")
        )
    "#)
    .calls(r#"
        [
            (method_invocation name: (identifier) @call.name)
            (object_creation_expression type: (type_identifier) @call.name)
        ]
    "#)
    .imports(r#"
        (import_declaration (scoped_identifier) @import.source)
    "#)
    .docs(r#"
        ((block_comment) @function.docs . [
            (class_declaration) (interface_declaration) (annotation_type_declaration)
            (enum_declaration) (method_declaration) (constructor_declaration) (record_declaration)
        ] @function.definition)
    "#)
    .literals(r#"(string_literal) @string"#)
    .implements(r#"
        (class_declaration name: (identifier) @impl.child superclass: (superclass (type_identifier) @impl.parent)? interfaces: (super_interfaces (type_list (type_identifier) @impl.parent))?)
        (interface_declaration name: (identifier) @impl.child)
        (record_declaration name: (identifier) @impl.child interfaces: (super_interfaces (type_list (type_identifier) @impl.parent))?)
    "#)
    .vals(r#"
        (variable_declarator name: (identifier) @val.name value: (string_literal) @val.value)
    "#)
    .types(r#"
        [ (formal_parameter type: (type_identifier) @type.ref) (method_declaration type: (type_identifier) @type.ref)
          (field_declaration type: (type_identifier) @type.ref) (local_variable_declaration type: (type_identifier) @type.ref) ]
    "#)
    .decorators(r#"
        (marker_annotation name: (identifier) @decorator.name)
        (annotation name: (identifier) @decorator.name arguments: (_)? )
    "#)
    .di_decorators(&["Service", "Component", "Repository", "Controller", "RestController", "Bean", "Configuration", "Inject", "Autowired"])
    .build()
}
