use crate::analysis::language::{LanguageConfig, LanguageConfigBuilder, SupportedLanguage};

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(
        SupportedLanguage::Hcl,
        &["tf", "hcl", "tfvars"]
    )
    .skeleton("{ # ... {} ... }")
    .defs(r#"
        ;; Resources and Data sources have 2 labels: type and name
        (block
            (identifier) @function.kind
            (string_lit (template_literal) @function.name)
            (string_lit (template_literal) @resource.instance_name)
            (body)? @function.body
            (#match? @function.kind "^(resource|data)$")
        ) @function.definition

        ;; Variables, Outputs, Modules, Providers have 1 label: name
        (block
            (identifier) @function.kind
            (string_lit (template_literal) @function.name)
            (body)? @function.body
            (#match? @function.kind "^(variable|output|module|provider)$")
        ) @function.definition
    "#)
    .docs(r#"
        ((comment) @function.docs . (block) @function.definition)
    "#)
    .literals(r#"
        [
            (string_lit) 
            (heredoc_template)
        ] @string
    "#)
    .vals(r#"
        (attribute
            (identifier) @val.name
            (expression (literal_value (string_lit (template_literal) @val.value)))
        )
    "#)
    .types(r#"
        ;; Capture the resource type (e.g., "aws_s3_bucket") as a type reference
        ;; This allows heuristics to link app code importing "aws_s3_bucket" to this block.
        (block 
            (identifier) @btype 
            (string_lit (template_literal) @type.ref)
            (#eq? @btype "resource")
        )
    "#)
    .build()
}