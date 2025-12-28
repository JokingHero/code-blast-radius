use crate::language::{LanguageConfig, SupportedLanguage};

pub const TYPESCRIPT_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::TypeScript,
    file_extensions: &["ts", "tsx"],
    query_defs: r#"
        [
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

          ((variable_declarator 
            name: (identifier) @function.name 
            type: (type_annotation)? @variable.type 
            value: [(arrow_function) (function_expression)]
          ) @function.definition)

          (variable_declarator
            name: (identifier) @variable.name
            type: (type_annotation)? @variable.type
          )
        ]
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

          ;; --- Dynamic Import & Require Support ---
          
          ;; import("literal")
          (call_expression
            function: (import)
            arguments: (arguments [(string) (template_string)] @import.source)
          )
          
          ;; import(variable) -> @import.dynamic
          (call_expression
            function: (import)
            arguments: (arguments (identifier) @import.dynamic)
          )

          ;; require("literal")
          (call_expression
            function: (identifier) @req (#eq? @req "require")
            arguments: (arguments [(string) (template_string)] @import.source)
          )

          ;; require(variable) -> @import.dynamic
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
        ;; --- 1. Redux / Object Pattern ---
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
        ;; UPDATED: Allow identifiers in switch cases
        (switch_case value: [(string) (template_string) (identifier)] @action.handle)
        ;; UPDATED: Allow identifiers in object keys (computed property names or simple keys)
        (pair key: [(string) (template_string) (identifier)] @action.handle value: [(arrow_function) (function_expression)])

        ;; --- 2. Event Emitter Patterns ---
        
        ;; Case A: Direct Call -> emit('event') OR emit(VARIABLE)
        (call_expression
            function: (identifier) @fn (#match? @fn "^(emit|dispatch|trigger|pub|publish|commit)$")
            arguments: (arguments 
                [(string) (template_string) (identifier)] @action.dispatch
            )
        )

        ;; Case B: Method Call -> app.emit('event') OR app.emit(VARIABLE)
        (call_expression
            function: (member_expression property: (property_identifier) @fn (#match? @fn "^(emit|dispatch|trigger|pub|publish|commit)$"))
            arguments: (arguments 
                [(string) (template_string) (identifier)] @action.dispatch
            )
        )

        ;; Case C: Direct Listener -> on('event') OR on(VARIABLE)
        (call_expression
            function: (identifier) @fn (#match? @fn "^(on|once|subscribe|sub|listen)$")
            arguments: (arguments 
                [(string) (template_string) (identifier)] @action.handle
            )
        )

        ;; Case D: Method Listener -> app.on('event') OR app.on(VARIABLE)
        (call_expression
            function: (member_expression property: (property_identifier) @fn (#match? @fn "^(on|once|subscribe|sub|listen)$"))
            arguments: (arguments 
                [(string) (template_string) (identifier)] @action.handle
            )
        )
    "#,
    di_decorators: &["Injectable", "Component", "Directive", "Pipe", "Service"],
};