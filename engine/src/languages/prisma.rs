use crate::language::{ LanguageConfig, SupportedLanguage };
pub const PRISMA_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::Prisma,
    file_extensions: &["prisma"],
    // S-Exp: (model_declaration (identifier))
    query_defs: r#"
(model_declaration
(identifier) @function.name
) @function.definition
"#,
    query_calls: "",
    query_docs: r#"
((comment)+ @function.docs . (model_declaration) @function.definition)
"#,
    query_imports: "",
    query_exports: "",
    query_literals: "",
    query_implements: "",
    query_config: "",
    query_vals: "",
    // S-Exp: (column_declaration (identifier) (column_type ...))
    query_types: r#"
    (column_declaration
    (identifier) @type.ref
    (column_type)
    )
    "#,
    query_decorators: "",
    query_actions: "",
    di_decorators: &[],
    magic_methods: &[]
};
