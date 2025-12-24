use crate::language::{LanguageConfig, SupportedLanguage};

pub const TYPESCRIPT_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::TypeScript,
    file_extensions: &["ts", "tsx"],
    query_defs: r#"
        [
          (function_declaration name: (identifier) @function.name) @function.definition
          ;; Capture methods in classes
          (method_definition name: [(property_identifier) (identifier)] @function.name) @function.definition
          ;; Capture methods in interfaces
          (method_signature name: [(property_identifier) (identifier)] @function.name) @function.definition
          ;; Capture containers
          (class_declaration name: (type_identifier) @function.name) @function.definition
          (interface_declaration name: [(type_identifier) (identifier)] @function.name) @function.definition
          ;; Capture variable-assigned functions
          ((variable_declarator name: (identifier) @function.name value: [(arrow_function) (function_expression)]) @function.definition)
        ]
    "#,
    query_calls: r#"
        [
          ;; Method calls: obj.method()
          (call_expression 
            function: (member_expression 
              object: [(identifier) (this)] @call.receiver
              property: [(property_identifier) (identifier)] @call.name))
          
          ;; Plain calls: method()
          (call_expression 
            function: (identifier) @call.name)

          ;; Fallback for complex property access
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
          (import_statement source: (string) @import.source)
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
};