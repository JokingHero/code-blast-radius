use crate::language::{LanguageConfig, SupportedLanguage};

pub const HCL_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::Hcl,
    file_extensions: &["tf", "hcl", "tfvars"],
    query_defs: r#"
        (block 
            (identifier) @function.kind
            (string_lit (template_literal) @function.name)
            (string_lit (template_literal) @resource.instance_name)
            (#match? @function.kind "^(resource|data)$")
        ) @function.definition
        
        (block 
            (identifier) @function.kind
            (string_lit (template_literal) @function.name)
            (#match? @function.kind "^(variable|output|module|provider)$")
        ) @function.definition
    "#,
    query_calls: "",
    query_docs: r#"
        ((comment) @function.docs . (block) @function.definition)
    "#,
    query_imports: "",
    query_exports: "",
    // UPDATED: Capture parent nodes to ensure text extraction works reliably
    query_literals: r#"
        [
            (string_lit) 
            (heredoc_template)
        ] @string
    "#,
    query_implements: "",
    query_config: "",
    query_vals: r#"
        (attribute
            (identifier) @val.name
            (expression (literal_value (string_lit (template_literal) @val.value)))
        )
    "#,
    query_types: r#"
        (block 
            (identifier) @btype 
            (string_lit (template_literal) @type.ref)
            (#eq? @btype "resource")
        )
    "#,
    query_decorators: "",
    query_actions: "",
    query_middleware: "",
    query_route_defs: "",
    di_decorators: &[],
    magic_methods: &[]
};