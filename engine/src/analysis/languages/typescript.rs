use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::TypeScript,
        &["ts", "tsx"]
    )
    .skeleton("{ /* ... {} ... */ }")
    .defs(r#"
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

          (variable_declarator
            name: (identifier) @function.name
            value: (call_expression
                function: (identifier) @fn_name
                arguments: (arguments) @function.body
            )
            (#match? @fn_name "^(create|make|define|build|atom|selector)$")
          ) @function.definition

          (variable_declarator
            name: (identifier) @function.name
            value: (call_expression
                function: (member_expression
                    property: [(property_identifier) (identifier)] @fn_name
                )
            )
            (#match? @fn_name "^(create|make|define|model|component|router|styled)$")
          ) @function.definition

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

          (variable_declarator
            name: (identifier) @variable.name
            type: (type_annotation)? @variable.type
          )

          (required_parameter
            pattern: (identifier) @variable.name
            type: (type_annotation)? @variable.type
          )
    "#)
    .frameworks(r#"
        ;; --- DECORATORS (Angular / NestJS) ---
        
        ;; 1. String Arg: @Get('/api')
        (decorator
            (call_expression
                function: (identifier) @framework.key
                arguments: (arguments (string) @framework.value)
            )
        )
        
        ;; 2. Object Arg: @Component({ ... })
        (decorator
            (call_expression
                function: (identifier) @framework.key
                arguments: (arguments (object) @framework.value)
            )
        )

        ;; 3. Template String Arg
        (decorator
            (call_expression
                function: (identifier) @framework.key
                arguments: (arguments (template_string) @framework.value)
            )
        )

        ;; 4. No Args / Marker: @Injectable()
        (decorator
            (call_expression
                function: (identifier) @framework.key
                arguments: (arguments)
            )
        )

        ;; --- EXPRESS / HTTP FRAMEWORKS ---
        (call_expression
            function: (member_expression
                property: (property_identifier) @framework.key
            )
            arguments: (arguments 
                (string) @framework.value
            )
            (#match? @framework.key "^(get|post|put|delete|patch|use)$")
        )

        ;; --- REDUX ---
        (call_expression
            function: (member_expression
                property: (property_identifier) @framework.key
            )
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
    .calls(r#"
        [
          (call_expression 
            function: (member_expression 
              object: [ (identifier) (this) (member_expression) ] @call.receiver
              property: [(property_identifier) (identifier)] @call.name))
          (call_expression function: (identifier) @call.name)
          (call_expression function: (member_expression property: [(property_identifier) (identifier)] @call.name))
        ]
    "#)
    .docs(r#"
      ((comment)+ @function.docs . [ 
          (function_declaration) (method_definition) (method_signature)
          (export_statement (variable_declaration)) (class_declaration) 
          (interface_declaration) (variable_declaration)
      ] @function.definition)
    "#)
    .imports(r#"
        [
          (import_statement (import_clause (named_imports (import_specifier name: (identifier) @import.name))) source: (string) @import.source)
          (import_statement (import_clause (namespace_import (identifier) @import.alias)) source: (string) @import.source)
          (import_statement source: (string) @import.source)
          (call_expression function: (import) arguments: (arguments [(string) (template_string)] @import.source))
          (call_expression function: (import) arguments: (arguments (identifier) @import.dynamic))
          (call_expression function: (identifier) @req arguments: (arguments [(string) (template_string)] @import.source) (#eq? @req "require"))
          (call_expression function: (identifier) @req arguments: (arguments (identifier) @import.dynamic) (#eq? @req "require"))
        ]
    "#)
    .exports(r#"
        [
            (export_statement (export_clause (export_specifier name: (identifier) @export.name)) source: (string) @export.source)
            (export_statement source: (string) @export.source)
        ]
    "#)
    .literals(r#"[ (string) (template_string) ] @string"#)
    .implements(r#"
        [
          (class_declaration name: (type_identifier) @impl.child (class_heritage (extends_clause value: (identifier) @impl.parent)? (implements_clause (type_identifier) @impl.parent)?))
          (interface_declaration name: [(type_identifier) (identifier)] @impl.child (extends_type_clause type: [(type_identifier) (identifier)] @impl.parent))
        ]
    "#)
    .config_keys(r#"
        [
          (member_expression object: (member_expression object: (identifier) @obj property: (property_identifier) @prop) property: (property_identifier) @config.key (#eq? @obj "process") (#eq? @prop "env"))
          (subscript_expression object: (member_expression object: (identifier) @obj property: (property_identifier) @prop) index: (string) @config.key (#eq? @obj "process") (#eq? @prop "env"))
          (call_expression function: (member_expression property: (property_identifier) @method) arguments: (arguments (string) @config.key) (#eq? @method "get"))
        ]
    "#)
    .vals(r#"
        (variable_declarator name: (identifier) @val.name value: [(string) (template_string)] @val.value)
    "#)
    .types(r#"
        [ (type_identifier) @type.ref (predefined_type) @type.ref (extends_clause value: (identifier) @type.ref) (new_expression constructor: (identifier) @type.ref) ]
    "#)
    .decorators(r#"
        (decorator [ (call_expression function: (identifier) @decorator.name) (identifier) @decorator.name ])
    "#)
    .di_decorators(&["Injectable", "Component", "Directive", "Pipe", "Service"])
    .constructor_names(&["constructor"])
    .project_config_files(&["tsconfig.json", "package.json"])
    .build()
}
