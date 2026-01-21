use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::JavaScript,
        &["js", "jsx", "mjs", "cjs", "vue"]
    )
    .skeleton("{ /* ... {} ... */ }")
    .defs(r#"
          (function_declaration
            name: (identifier) @function.name
            body: (statement_block) @function.body
          ) @function.definition

          (generator_function_declaration
            name: (identifier) @function.name
            body: (statement_block) @function.body
          ) @function.definition

          (method_definition
            name: (property_identifier) @function.name
            body: (statement_block)? @function.body
          ) @function.definition

          (class_declaration
            name: (identifier) @function.name
            body: (class_body) @function.body
          ) @function.definition

          ((variable_declarator
             name: (identifier) @function.name
             value: (arrow_function body: (_) @function.body)
          ) @function.definition)

          ((variable_declarator
             name: (identifier) @function.name
             value: (function_expression body: (statement_block) @function.body)
          ) @function.definition)
          
          (variable_declarator
            name: (identifier) @function.name
            value: (call_expression
                function: (identifier) @fn_name
                (#match? @fn_name "^(create|make|define|build|atom|selector)$")
            )
          ) @function.definition
    "#)
    // --- NEW: Inference Engine Hooks ---
    .frameworks(r#"
        ;; --- EXPRESS JS ROUTES ---
        ;; Captures: app.get('/users') -> key="get", value="'/users'"
        (call_expression
            function: (member_expression
                property: (property_identifier) @framework.key
            )
            arguments: (arguments 
                (string) @framework.value
            )
            (#match? @framework.key "^(get|post|put|delete|patch|use)$")
        )

        ;; --- MONGOOSE / SEQUELIZE MODELS ---
        ;; Captures: mongoose.model('User', schema) -> key="model", value="'User'"
        (call_expression
            function: (member_expression
                property: (property_identifier) @framework.key
            )
            arguments: (arguments 
                (string) @framework.value
            )
            (#match? @framework.key "^(model|define)$")
        )

        ;; --- VUE JS COMPONENTS ---
        ;; Captures: Vue.component('my-component', ...) -> key="component", value="'my-component'"
        (call_expression
            function: (member_expression
                property: (property_identifier) @framework.key
            )
            arguments: (arguments 
                (string) @framework.value
            )
            (#eq? @framework.key "component")
        )

        ;; --- REDUX (Plain JS) ---
        ;; Captures: dispatch({ type: 'LOGIN' }) -> key="dispatch", value="'LOGIN'"
        (call_expression
            function: (identifier) @framework.key
            arguments: (arguments 
                (object 
                    (pair 
                        key: (property_identifier) @k 
                        value: [(string) (identifier)] @framework.value
                    )
                )
            )
            (#eq? @framework.key "dispatch")
            (#eq? @k "type") 
        )
    "#)
    // --- EXISTING LOGIC PRESERVED BELOW ---
    .calls(r#"
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
    "#)
    .docs(r#"
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
    "#)
    .imports(r#"
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
    "#)
    .exports(r#"
        [
            (export_statement
                (export_clause
                    (export_specifier name: (identifier) @export.name)
                )
            )
            (export_statement
                (export_clause
                    (export_specifier name: (identifier) @export.name)
                )
                source: (string) @export.source
            )
            (export_statement
                source: (string) @export.source
            )
            (export_statement
                declaration: [
                    (variable_declaration (variable_declarator name: (identifier) @export.name))
                    (function_declaration name: (identifier) @export.name)
                    (class_declaration name: (identifier) @export.name)
                ]
            )
        ]
    "#)
    .literals(r#"[ (string) (template_string) ] @string"#)
    .implements(r#"
        (class_declaration
            name: (identifier) @impl.child
            (class_heritage (identifier) @impl.parent)
        )
    "#)
    .config_keys(r#"
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
    "#)
    .vals(r#"
        (variable_declarator
            name: (identifier) @val.name
            value: [(string) (template_string)] @val.value
        )
    "#)
    .actions(r#"
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
    "#)
    .middleware(r#"
        ;; Express/Connect style middleware
        (call_expression
            function: (member_expression
                property: (property_identifier) @prop
            )
            arguments: (arguments 
                [(identifier) (call_expression)] @middleware.use
            )
            (#eq? @prop "use")
        )
    "#)
    .routes(r#"
        (call_expression
            function: (member_expression
                property: (property_identifier) @method
            )
            arguments: (arguments 
                (string) @route.path
            )
            (#match? @method "^(get|post|put|delete|patch|options|head|all)$")
        )
    "#)
    .constructor_names(&["constructor"])
    .project_config_files(&["package.json", "jsconfig.json"])
    .build()
}
