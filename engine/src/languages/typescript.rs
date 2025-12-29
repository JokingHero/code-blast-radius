use crate::language::{LanguageConfig, SupportedLanguage};

pub const TYPESCRIPT_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::TypeScript,
    file_extensions: &["ts", "tsx"],
    query_defs: r#"
          ;; --- Standard Definitions ---
          (function_declaration 
            name: (identifier) @function.name 
            return_type: (type_annotation)? @function.return_type 
          ) @function.definition

          (method_definition 
            name: [(property_identifier) (identifier)] @function.name 
            return_type: (type_annotation)? @function.return_type 
          ) @function.definition

          (method_signature 
            name: [(property_identifier) (identifier)] @function.name 
            return_type: (type_annotation)? @function.return_type 
          ) @function.definition

          (class_declaration name: (type_identifier) @function.name) @function.definition
          (interface_declaration name: [(type_identifier) (identifier)] @function.name) @function.definition

          ;; --- Arrow Functions ---
          ((variable_declarator 
            name: (identifier) @function.name 
            type: (type_annotation)? @variable.type 
            value: [(arrow_function) (function_expression)]
          ) @function.definition)

          ;; --- Factory Patterns ---
          
          ;; 1. Direct Call: const useStore = create(...)
          (variable_declarator
            name: (identifier) @function.name
            value: (call_expression
                function: (identifier) @fn_name
                (#match? @fn_name "^(create|make|define|build|atom|selector)$")
            )
          ) @function.definition

          ;; 2. Member Factory: const User = mongoose.model(...)
          (variable_declarator
            name: (identifier) @function.name
            value: (call_expression
                function: (member_expression
                    object: (_)
                    property: [(property_identifier) (identifier)] @fn_name
                    (#match? @fn_name "^(create|make|define|model|component|router|styled)$")
                )
            )
          ) @function.definition

          ;; 3. Styled Component (Object Access): const Title = styled.h1`...`
          (variable_declarator
            name: (identifier) @function.name
            value: (call_expression
                function: (member_expression
                    object: (identifier) @obj_name
                    (#eq? @obj_name "styled")
                )
            )
          ) @function.definition

          ;; 4. Styled Component (Curried Call): const Box = styled('div')(...)
          (variable_declarator
            name: (identifier) @function.name
            value: (call_expression
                function: (call_expression
                    function: (identifier) @inner_fn
                    (#eq? @inner_fn "styled")
                )
            )
          ) @function.definition

          ;; --- Variables Fallback ---
          (variable_declarator
            name: (identifier) @variable.name
            type: (type_annotation)? @variable.type
          )
    "#,
    query_calls: r#"
        [
          (call_expression 
            function: (member_expression 
              object: [
                (identifier) 
                (this) 
                (member_expression) 
              ] @call.receiver
              property: [(property_identifier) (identifier)] @call.name))
          
          (call_expression 
            function: (identifier) @call.name)

          (call_expression
            function: (member_expression
              property: [(property_identifier) (identifier)] @call.name))
        ]
    "#,
    query_docs: r#"
      (
        (comment)+ @function.docs
        .
        [ 
          (function_declaration) 
          (method_definition) 
          (method_signature)
          (export_statement (variable_declaration)) 
          (class_declaration) 
          (interface_declaration) 
          (variable_declaration)
        ] @function.definition
      )
    "#,
    query_imports: r#"
        [
          (import_statement
              (import_clause
                  (named_imports
                      (import_specifier
                          name: (identifier) @import.name
                      )
                  )
              )
              source: (string) @import.source
          )
          (import_statement
            (import_clause
              (namespace_import (identifier) @import.alias)
            )
            source: (string) @import.source
          )
          (import_statement source: (string) @import.source)

          (call_expression
            function: (import)
            arguments: (arguments [(string) (template_string)] @import.source)
          )
          
          (call_expression
            function: (import)
            arguments: (arguments (identifier) @import.dynamic)
          )

          (call_expression
            function: (identifier) @req (#eq? @req "require")
            arguments: (arguments [(string) (template_string)] @import.source)
          )

          (call_expression
            function: (identifier) @req (#eq? @req "require")
            arguments: (arguments (identifier) @import.dynamic)
          )
        ]
    "#,
    query_exports: r#"
        [
            (export_statement
              (export_clause
                (export_specifier
                  name: (identifier) @export.name
                )
              )
              source: (string) @export.source
            )
            (export_statement
              (wildcard_import)
              source: (string) @export.source
            )
        ]
    "#,
    query_literals: r#"[ (string) (template_string) ] @string"#,
    query_implements: r#"
        [
          (class_declaration
            name: (type_identifier) @impl.child
            (class_heritage 
                (extends_clause value: (identifier) @impl.parent)?
                (implements_clause (type_identifier) @impl.parent)?
            )
          )
          (interface_declaration
            name: [(type_identifier) (identifier)] @impl.child
            (extends_type_clause type: [(type_identifier) (identifier)] @impl.parent)
          )
        ]
    "#,
    query_config: r#"
        [
          (member_expression
            object: (member_expression
              object: (identifier) @obj (#eq? @obj "process")
              property: (property_identifier) @prop (#eq? @prop "env"))
            property: (property_identifier) @config.key)

          (subscript_expression
            object: (member_expression
              object: (identifier) @obj (#eq? @obj "process")
              property: (property_identifier) @prop (#eq? @prop "env"))
            index: (string) @config.key)

          (call_expression
            function: (member_expression
              property: (property_identifier) @method (#eq? @method "get"))
            arguments: (arguments (string) @config.key))
        ]
    "#,
    query_vals: r#"
        (variable_declarator
            name: (identifier) @val.name
            value: [(string) (template_string)] @val.value
        )
    "#,
    query_types: r#"
        [
            (type_annotation (type_identifier) @type.ref)
            (extends_clause value: (identifier) @type.ref)
            (implements_clause (type_identifier) @type.ref)
            (new_expression constructor: (identifier) @type.ref)
            (type_arguments (type_identifier) @type.ref)
        ]
    "#,
    query_decorators: r#"
        (decorator 
            [
                (call_expression function: (identifier) @decorator.name)
                (identifier) @decorator.name
            ]
        )
    "#,
    query_actions: r#"
        (call_expression
            function: (identifier) @fn (#match? @fn "^(dispatch|put|emit|commit)$")
            arguments: (arguments 
                (object 
                    (pair 
                        key: (property_identifier) @k (#eq? @k "type") 
                        value: [(string) (template_string) (identifier)] @action.dispatch
                    )
                )
            )
        )
        (switch_case value: [(string) (template_string) (identifier)] @action.handle)
        (pair key: [(string) (template_string) (identifier)] @action.handle value: [(arrow_function) (function_expression)])

        (call_expression
            function: (identifier) @fn (#match? @fn "^(emit|dispatch|trigger|pub|publish|commit)$")
            arguments: (arguments 
                [(string) (template_string) (identifier)] @action.dispatch
            )
        )
        (call_expression
            function: (member_expression property: (property_identifier) @fn (#match? @fn "^(emit|dispatch|trigger|pub|publish|commit)$"))
            arguments: (arguments 
                [(string) (template_string) (identifier)] @action.dispatch
            )
        )
        (call_expression
            function: (identifier) @fn (#match? @fn "^(on|once|subscribe|sub|listen)$")
            arguments: (arguments 
                [(string) (template_string) (identifier)] @action.handle
            )
        )
        (call_expression
            function: (member_expression property: (property_identifier) @fn (#match? @fn "^(on|once|subscribe|sub|listen)$"))
            arguments: (arguments 
                [(string) (template_string) (identifier)] @action.handle
            )
        )
    "#,
    di_decorators: &["Injectable", "Component", "Directive", "Pipe", "Service"],
    magic_methods: &[]
};