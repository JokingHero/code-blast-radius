use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::TypeScript,
        &["ts", "tsx"]
    )
    .skeleton("{ /* ... {} ... */ }")
    .defs(r#"
          ;; Standard Definitions
          (function_declaration
            name: (identifier) @function.name
            return_type: (type_annotation)? @function.return_type
            body: (statement_block) @function.body
          ) @function.definition

          (method_definition
            name: [(property_identifier) (identifier)] @function.name
            return_type: (type_annotation)? @function.return_type
            body: (statement_block)? @function.body
          ) @function.definition

          (method_signature
            name: [(property_identifier) (identifier)] @function.name
            return_type: (type_annotation)? @function.return_type
          ) @function.definition

          (class_declaration
            name: (type_identifier) @function.name
            body: (class_body) @function.body
          ) @function.definition

          (interface_declaration name: [(type_identifier) (identifier)] @function.name) @function.definition

          ;; Type Aliases
          (type_alias_declaration
            name: (type_identifier) @function.name
            value: (_) @function.body
          ) @function.definition

          ((variable_declarator
            name: (identifier) @function.name
            type: (type_annotation)? @variable.type
            value: (arrow_function
              body: (_) @function.body
            )
          ) @function.definition)

          ((variable_declarator
            name: (identifier) @function.name
            type: (type_annotation)? @variable.type
            value: (function_expression
              body: (statement_block) @function.body
            )
          ) @function.definition)

          ;; --- Factories / Framework Patterns ---

          ;; 1. Direct Call (e.g., const useStore = create(...))
          (variable_declarator
            name: (identifier) @function.name
            value: (call_expression
                function: (identifier) @fn_name
                arguments: (arguments) @function.body
            )
            (#match? @fn_name "^(create|make|define|build|atom|selector)$")
          ) @function.definition

          ;; 2. Member Factory (e.g., mongoose.model, sequelize.define)
          (variable_declarator
            name: (identifier) @function.name
            value: (call_expression
                function: (member_expression
                    property: [(property_identifier) (identifier)] @fn_name
                )
            )
            (#match? @fn_name "^(create|make|define|model|component|router|styled)$")
          ) @function.definition

          ;; 3. Styled Components: Tagged Template as Call (Observed Structure)
          ;; Matches: styled.h1`...` parsed as call(function: styled.h1, args: template_string)
          (variable_declarator
            name: (identifier) @function.name
            value: (call_expression
                function: (member_expression
                    object: (identifier) @obj_name
                )
                arguments: (template_string)
            )
            (#eq? @obj_name "styled")
          ) @function.definition

          ;; 4. Styled Components: Curried Call (e.g. styled('div')`...`)
          ;; Matches: styled('div')`...`
          (variable_declarator
            name: (identifier) @function.name
            value: (call_expression
                function: (call_expression
                    function: (call_expression
                        function: (identifier) @inner_fn
                    )
                )
                arguments: (template_string)
            )
            (#eq? @inner_fn "styled")
          ) @function.definition

          ;; Variable Fallback
          (variable_declarator
            name: (identifier) @variable.name
            type: (type_annotation)? @variable.type
          )

          ;; Constructor Parameters (for DI)
          (required_parameter
            pattern: (identifier) @variable.name
            type: (type_annotation)? @variable.type
          )
    "#)
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

          (call_expression
            function: (member_expression
              property: [(property_identifier) (identifier)] @call.name))
        ]
    "#)
    .docs(r#"
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
            function: (identifier) @req 
            arguments: (arguments [(string) (template_string)] @import.source)
            (#eq? @req "require")
          )

          (call_expression
            function: (identifier) @req 
            arguments: (arguments (identifier) @import.dynamic)
            (#eq? @req "require")
          )
        ]
    "#)
    .exports(r#"
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
              source: (string) @export.source
            )
        ]
    "#)
    .literals(r#"[ (string) (template_string) ] @string"#)
    .implements(r#"
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

          (call_expression
            function: (member_expression
              property: (property_identifier) @method)
            arguments: (arguments (string) @config.key)
            (#eq? @method "get")
          )
        ]
    "#)
    .vals(r#"
        (variable_declarator
            name: (identifier) @val.name
            value: [(string) (template_string)] @val.value
        )
    "#)
    // Simplified to use broad node captures.
    // - (type_identifier) catches almost all types (interfaces, generics, array types, etc.)
    // - (predefined_type) catches things like 'string', 'number' (less useful for linking but valid)
    // - Specific rules for extends/new where a simple (identifier) acts as a type.
    .types(r#"
        [
            (type_identifier) @type.ref
            (predefined_type) @type.ref
            (extends_clause value: (identifier) @type.ref)
            (new_expression constructor: (identifier) @type.ref)
        ]
    "#)
    .decorators(r#"
        (decorator 
            [
                (call_expression function: (identifier) @decorator.name)
                (identifier) @decorator.name
            ]
        )
    "#)
    .actions(r#"
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
        (switch_case value: [(string) (template_string) (identifier)] @action.handle)
        (pair key: [(string) (template_string) (identifier)] @action.handle value: [(arrow_function) (function_expression)])

        (call_expression
            function: (identifier) @fn 
            arguments: (arguments 
                [(string) (template_string) (identifier)] @action.dispatch
            )
            (#match? @fn "^(emit|dispatch|trigger|pub|publish|commit)$")
        )
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
                [(string) (template_string) (identifier)] @action.handle
            )
            (#match? @fn "^(on|once|subscribe|sub|listen)$")
        )
        (call_expression
            function: (member_expression property: (property_identifier) @fn)
            arguments: (arguments 
                [(string) (template_string) (identifier)] @action.handle
            )
            (#match? @fn "^(on|once|subscribe|sub|listen)$")
        )
    "#)
    .middleware(r#"
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
        (decorator
            (call_expression
                function: (identifier) @fn
                arguments: (arguments [(string) (template_string)] @route.path)
            )
            (#match? @fn "^(Controller|Get|Post|Put|Delete|Patch)$")
        )
    "#)
    .di_decorators(&["Injectable", "Component", "Directive", "Pipe", "Service"])
    .constructor_names(&["constructor"])
    .project_config_files(&["tsconfig.json", "package.json"])
    .build()
}