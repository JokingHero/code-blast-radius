use crate::analysis::language::{LanguageConfig, SupportedLanguage};

pub const JAVASCRIPT_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::JavaScript,
    file_extensions: &["js", "jsx", "mjs", "cjs", "vue"],
    query_defs: r#"
          (function_declaration name: (identifier) @function.name) @function.definition
          (generator_function_declaration name: (identifier) @function.name) @function.definition
          (method_definition name: (property_identifier) @function.name) @function.definition
          (class_declaration name: (identifier) @function.name) @function.definition
          
          ((variable_declarator 
             name: (identifier) @function.name 
             value: [(arrow_function) (function_expression)]
          ) @function.definition)
          
          (variable_declarator
            name: (identifier) @function.name
            value: (call_expression
                function: (identifier) @fn_name
                (#match? @fn_name "^(create|make|define|build|atom|selector)$")
            )
          ) @function.definition
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
        ]
    "#,
    query_docs: r#"
      (
        (comment)+ @function.docs
        .
        [ 
            (function_declaration) 
            (generator_function_declaration)
            (method_definition) 
            (export_statement (variable_declaration)) 
            (class_declaration) 
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
          (import_statement source: (string) @import.source)
          (call_expression
            function: (identifier) @req 
            arguments: (arguments [(string) (template_string)] @import.source)
            (#eq? @req "require")
          )
        ]
    "#,
    query_exports: "",
    query_literals: r#"[ (string) (template_string) ] @string"#,
    query_implements: r#"
        (class_declaration
            name: (identifier) @impl.child
            (class_heritage (identifier) @impl.parent)
        )
    "#,
    query_config: r#"
        [
          (member_expression
            object: (member_expression
              object: (identifier) @obj 
              property: (property_identifier) @prop)
            property: (property_identifier) @config.key
            (#eq? @obj "process")
            (#eq? @prop "env")
          )
          (subscript_expression
            object: (member_expression
              object: (identifier) @obj 
              property: (property_identifier) @prop)
            index: (string) @config.key
            (#eq? @obj "process")
            (#eq? @prop "env")
          )
        ]
    "#,
    query_vals: r#"
        (variable_declarator
            name: (identifier) @val.name
            value: [(string) (template_string)] @val.value
        )
    "#,
    query_types: "",
    query_decorators: "",
    query_actions: r#"
        ;; --- Redux / Object Patterns ---
        
        ;; Dispatching object with type: '...' (e.g. put({ type: 'ADD' }))
        (call_expression
            function: (identifier) @fn 
            arguments: (arguments 
                (object 
                    (pair 
                        key: (property_identifier) @k 
                        value: [(string) (template_string) (identifier)] @action.dispatch
                    )
                )
            )
            (#match? @fn "^(dispatch|put|emit|commit)$")
            (#eq? @k "type") 
        )

        ;; Switch case handling (Redux reducers)
        (switch_case value: [(string) (template_string) (identifier)] @action.handle)

        ;; Object map handling (Redux handlers map)
        (pair key: [(string) (template_string) (identifier)] @action.handle value: [(arrow_function) (function_expression)])

        ;; --- Call Patterns (Event Emitters) ---

        ;; Dispatching (emit, dispatch, etc)
        (call_expression
            function: (member_expression property: (property_identifier) @fn)
            arguments: (arguments 
                [(string) (template_string) (identifier)] @action.dispatch
            )
            (#match? @fn "^(emit|dispatch|trigger|pub|publish|commit)$")
        )
        (call_expression
            function: (identifier) @fn 
            arguments: (arguments 
                [(string) (template_string) (identifier)] @action.dispatch
            )
            (#match? @fn "^(emit|dispatch|trigger|pub|publish|commit)$")
        )

        ;; Handling (on, subscribe, etc)
        (call_expression
            function: (member_expression property: (property_identifier) @fn)
            arguments: (arguments 
                [(string) (template_string) (identifier)] @action.handle
            )
            (#match? @fn "^(on|once|subscribe|sub|listen)$")
        )
        (call_expression
            function: (identifier) @fn 
            arguments: (arguments 
                [(string) (template_string) (identifier)] @action.handle
            )
            (#match? @fn "^(on|once|subscribe|sub|listen)$")
        )
    "#,
    query_middleware: "",
    query_route_defs: "",
    di_decorators: &[],
    magic_methods: &[]
};