use crate::language::{LanguageConfig, SupportedLanguage};

pub const TYPESCRIPT_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::TypeScript,
    file_extensions: &["ts", "tsx"],
    query_defs: r#"
        [
          (function_declaration 
            name: (identifier) @function.name 
            return_type: (type_annotation)? @function.return_type  ;;
          ) @function.definition

          (method_definition 
            name: [(property_identifier) (identifier)] @function.name 
            return_type: (type_annotation)? @function.return_type  ;;
          ) @function.definition

          (method_signature 
            name: [(property_identifier) (identifier)] @function.name 
            return_type: (type_annotation)? @function.return_type  ;;
          ) @function.definition

          (class_declaration name: (type_identifier) @function.name) @function.definition
          (interface_declaration name: [(type_identifier) (identifier)] @function.name) @function.definition

          ((variable_declarator 
            name: (identifier) @function.name 
            type: (type_annotation)? @variable.type               ;;
            value: [(arrow_function) (function_expression)]
          ) @function.definition)

          ;; Variable Hints (Metadata only - No @function.definition)
          ;; so we know 'const user: User'
          (variable_declarator
            name: (identifier) @variable.name
            type: (type_annotation)? @variable.type
          )
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