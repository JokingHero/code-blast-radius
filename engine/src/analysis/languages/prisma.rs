use crate::analysis::language::{ LanguageConfig, LanguageConfigBuilder, SupportedLanguage };

pub fn config() -> LanguageConfig {
    LanguageConfigBuilder::new(SupportedLanguage::Prisma, &["prisma"])
        // S-Exp: (model_declaration (identifier))
        .defs(r#"
            (model_declaration
            (identifier) @function.name
            ) @function.definition
            "#)
                    .docs(r#"
            ((comment)+ @function.docs . (model_declaration) @function.definition)
            "#)
        // S-Exp: (column_declaration (identifier) (column_type ...))
        .types(r#"
            (column_declaration
            (identifier) @type.ref
            (column_type)
            )
            "#)
        .build()
}
