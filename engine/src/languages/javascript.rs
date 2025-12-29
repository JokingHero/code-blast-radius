use crate::{language::{LanguageConfig, SupportedLanguage}, languages::typescript::TYPESCRIPT_CONFIG};

pub const JAVASCRIPT_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::JavaScript,
    file_extensions: &["js", "jsx", "mjs", "cjs", "vue"],
    // REMOVED OUTER BRACKETS [ ... ]
    query_defs: r#"
          (function_declaration name: (identifier) @function.name) @function.definition
          (generator_function_declaration name: (identifier) @function.name) @function.definition
          (method_definition name: (property_identifier) @function.name) @function.definition
          (class_declaration name: (identifier) @function.name) @function.definition
          
          ((variable_declarator 
             name: (identifier) @function.name 
             value: [(arrow_function) (function_expression)]
          ) @function.definition)

          ;; --- Factory Patterns ---
          
          ;; 1. Direct Call
          (variable_declarator
            name: (identifier) @function.name
            value: (call_expression
                function: (identifier) @fn_name
                (#match? @fn_name "^(create|make|define|build|atom|selector)$")
            )
          ) @function.definition

          ;; 2. Member Factory
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

          ;; 3. Styled Component (Object Access)
          (variable_declarator
            name: (identifier) @function.name
            value: (call_expression
                function: (member_expression
                    object: (identifier) @obj_name
                    (#eq? @obj_name "styled")
                )
            )
          ) @function.definition

          ;; 4. Styled Component (Function Call)
          (variable_declarator
            name: (identifier) @function.name
            value: (call_expression
                function: (call_expression
                    function: (identifier) @inner_fn
                    (#eq? @inner_fn "styled")
                )
            )
          ) @function.definition
    "#,
    query_calls: TYPESCRIPT_CONFIG.query_calls,
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
    query_imports: TYPESCRIPT_CONFIG.query_imports,
    query_exports: "",
    query_literals: r#"[ (string) (template_string) ] @string"#,
    query_implements: r#"
        (class_declaration
            name: (identifier) @impl.child
            (class_heritage (identifier) @impl.parent)
        )
    "#,
    query_config: "",
    query_vals: r#"
        (variable_declarator
            name: (identifier) @val.name
            value: [(string) (template_string)] @val.value
        )
    "#,
    query_types: "",
    query_decorators: "",
    query_actions: TYPESCRIPT_CONFIG.query_actions,
    di_decorators: &[],
    magic_methods: &[]
};