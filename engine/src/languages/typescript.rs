use crate::language::{LanguageConfig, SupportedLanguage};

pub const TYPESCRIPT_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::TypeScript,
    file_extensions: &["ts", "tsx"],
    query_defs: r#"
        [
          (function_declaration name: (identifier) @function.name) @function.definition
          (method_definition name: (property_identifier) @function.name) @function.definition
          (class_declaration name: (type_identifier) @function.name) @function.definition
          (interface_declaration name: (type_identifier) @function.name) @function.definition
          ((variable_declarator name: (identifier) @function.name value: [(arrow_function) (function_expression)]) @function.definition)
        ]
    "#,
    query_calls: r#"(call_expression function: [(identifier) @call.name (member_expression property: (property_identifier) @call.name)])"#,
    query_docs: r#"
      (
        (comment)+ @function.docs
        .
        [ (function_declaration) (method_definition) (export_statement (variable_declaration)) (class_declaration) (interface_declaration) ] @function.definition
      )
    "#,
    query_imports: r#"
        [
          ;; 1. Your original working named imports query
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
          ;; 2. Side-effect imports: import "./file"
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
            name: (type_identifier) @impl.child
            (extends_type_clause type: (type_identifier) @impl.parent)
          )
        ]
    "#,
};