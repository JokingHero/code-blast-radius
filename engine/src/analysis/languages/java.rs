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
    // --- NEW: Inference Engine Hooks ---
    .frameworks(r#"
        ;; --- SPRING BOOT / JAKARTA EE ---

        ;; 1. DI Providers (Implicit Class Name)
        ;; Captures: @Service class UserService -> key="Service", value="UserService"
        (class_declaration
            modifiers: (modifiers (marker_annotation name: (identifier) @framework.key))
            name: (identifier) @framework.value
            (#match? @framework.key "^(Service|Component|Repository|Controller|RestController|Configuration)$")
        )

        ;; 2. Routes & Config (Simple Value)
        ;; Captures: @GetMapping("/users") -> key="GetMapping", value="'/users'"
        ;; Captures: @Service("myService") -> key="Service", value="'myService'"
        (annotation
            name: (identifier) @framework.key
            arguments: (annotation_argument_list (string_literal) @framework.value)
        )

        ;; 3. Routes & Config (Named Attributes)
        ;; Captures: @RequestMapping(value = "/users") -> key="RequestMapping", value="'/users'"
        ;; Captures: @Table(name = "users") -> key="Table", value="'users'"
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
        ;; Captures: @Bean public DataSource myDataSource() -> key="Bean", value="myDataSource"
        (method_declaration
            modifiers: (modifiers (marker_annotation name: (identifier) @framework.key))
            name: (identifier) @framework.value
            (#eq? @framework.key "Bean")
        )
    "#)
    // --- EXISTING LOGIC PRESERVED BELOW ---
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
            interfaces: (super_interfaces (type_list (type_identifier) @impl.parent))?
        )
        (interface_declaration
            name: (identifier) @impl.child
        )
        (record_declaration
            name: (identifier) @impl.child
            interfaces: (super_interfaces (type_list (type_identifier) @impl.parent))?
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