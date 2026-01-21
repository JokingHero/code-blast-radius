use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::CSharp,
        &["cs"]
    )
    .skeleton("{ /* ... {} ... */ }")
    .defs(r#"
        (method_declaration name: (identifier) @function.name) @function.definition
        (class_declaration name: (identifier) @function.name) @function.definition
        (interface_declaration name: (identifier) @function.name) @function.definition
        (struct_declaration name: (identifier) @function.name) @function.definition
        (record_declaration name: (identifier) @function.name) @function.definition

        ;; Support for C# 9+ top-level statements / local functions
        (local_function_statement name: (identifier) @function.name) @function.definition

        ;; Robust fallback
        (method_declaration 
            (identifier) @function.name
            (parameter_list)
        ) @function.definition
    "#)
    // --- NEW: Inference Engine Hooks ---
    .frameworks(r#"
        ;; --- ASP.NET CORE ATTRIBUTES ---
        ;; Captures: [Route("api/users")] -> key="Route", value="api/users"
        (attribute
            name: (identifier) @framework.key
            (attribute_argument_list
                (string_literal) @framework.value
            )
            (#match? @framework.key "^(Route|HttpGet|HttpPost|HttpPut|HttpDelete)$")
        )
        
        ;; Captures: [ApiController] -> key="ApiController" (no value)
        (attribute
            name: (identifier) @framework.key
            (#eq? @framework.key "ApiController")
        )
    "#)
    // --- EXISTING LOGIC ---
    .calls(r#"
        (invocation_expression
            function: [
                (identifier) @call.name
                (member_access_expression name: (identifier) @call.name)
            ]
        )
    "#)
    .imports(r#"
        (using_directive (identifier) @import.source)
        (using_directive (qualified_name) @import.source)
    "#)
    .literals(r#"
        [(string_literal) (verbatim_string_literal)] @string
    "#)
    .project_config_files(&["*.csproj", "Package.toml", "NuGet.Config"])
    .build()
}