//! The orchestrator for the analysis pipeline.
//! This module coordinates the various extraction phases:
//! 1. Constants (Pre-processing)
//! 2. Structural Data (Imports/Exports)
//! 3. Definitions (Functions/Classes)
//! 4. Enrichment (Calls/Decorators)

pub mod constants;
pub mod structural;
pub mod definitions;
pub mod enrichment;
pub mod language;
pub mod languages;

use std::collections::HashMap;
use std::path::Path;
use tree_sitter::Parser;

use crate::models::FileAnalysis;
use crate::analysis::language::{LanguageConfig, get_language};

/// Analyzes a source file and extracts all relevant metadata.
///
/// # Arguments
/// * `path` - The file path (used for module naming).
/// * `source_code` - The content of the file.
/// * `config` - The language configuration to use for parsing.
pub fn analyze_source(
    path: &Path,
    source_code: &str,
    config: &LanguageConfig
) -> Result<FileAnalysis, String> {
    
    // 1. Setup Parser
    let mut parser = Parser::new();
    let language = get_language(config.lang);
    parser.set_language(&language).map_err(|e| e.to_string())?;

    // Handle empty files early
    if source_code.trim().is_empty() {
        return Ok(FileAnalysis {
            functions: vec![],
            imports: vec![],
            exports: vec![],
            literals: vec![],
            implementations: vec![],
            global_vars: HashMap::new(),
            middleware_usage: vec![],
            defined_routes: vec![],
        });
    }

    let tree = parser.parse(source_code, None).ok_or("Failed to parse code")?;
    let root_node = tree.root_node();
    let code_bytes = source_code.as_bytes();

    // 2. Phase 0: Constants (Pre-processing)
    // Extract local variable assignments to resolve dynamic patterns later.
    let constants = constants::extract_local_constants(root_node, code_bytes, &language, config);

    // 3. Phase 1: Structural Data
    // Extract file-level metadata like imports, exports, and literals.
    let structure = structural::extract_structure(root_node, code_bytes, &language, config, &constants);

    // 4. Phase 2: Definitions (Skeleton)
    // Extract functions, classes, and module info.
    let module_name = path.file_stem().unwrap_or_default().to_string_lossy();
    let definition_result = definitions::extract_definitions(
        root_node, 
        code_bytes, 
        &language, 
        config, 
        &module_name
    )?;

    let mut functions = definition_result.functions;
    let mut module_info = definition_result.module_info;
    let variable_hints = definition_result.variable_hints;

    // 5. Phase 3: Enrichment (Flesh out the skeleton)
    // Run secondary queries to attach calls, types, and decorators to the specific functions.
    enrichment::enrich_functions(
        &mut functions, 
        &mut module_info, 
        variable_hints,
        root_node, 
        code_bytes, 
        &language, 
        config, 
        &constants
    );

    // 6. Finalize
    // Add the module info as the last "function" entry, serving as the catch-all scope.
    functions.push(module_info);

    // 7. Return Result
    Ok(FileAnalysis {
        functions,
        imports: structure.imports,
        exports: structure.exports,
        literals: structure.literals,
        implementations: structure.implementations,
        global_vars: HashMap::new(), // Currently unused but reserved for future global var tracking
        middleware_usage: structure.middleware_usage,
        defined_routes: structure.defined_routes,
    })
}