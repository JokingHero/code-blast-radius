use crate::analysis::language::{ LanguageConfig, SupportedLanguage };
pub const SQL_CONFIG: LanguageConfig = LanguageConfig {
    lang_enum: SupportedLanguage::Sql,
    file_extensions: &["sql"],
    // S-Exp: (create_table (object_reference name: (identifier)))
    query_defs: r#"
(create_table
(object_reference
name: (identifier) @function.name
)
) @function.definition
"#,
    query_calls: "",
    query_docs: r#"
((comment)+ @function.docs . (create_table) @function.definition)
"#,
    query_imports: "",
    query_exports: "",
    query_literals: "",
    query_implements: "",
    query_config: "",
    query_vals: "",
    // S-Exp: (column_definition name: (identifier))
    query_types: r#"
    (column_definition
    name: (identifier) @type.ref
    )
    "#,
    query_decorators: "",
    query_actions: "",
    query_middleware: "",
    query_route_defs: "",
    di_decorators: &[],
    magic_methods: &[]
};
