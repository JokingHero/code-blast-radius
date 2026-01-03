//! Extracts function, class, and module definitions from the source code.
//! This creates the "skeleton" of the file analysis, which is later enriched
//! with calls, types, and other metadata.

use std::collections::HashMap;
use tree_sitter::{Node, QueryCursor, StreamingIterator};
use crate::analysis::language::LanguageConfig;
use crate::models::{FunctionInfo, SymbolKind};

/// Represents a potential variable definition that carries type or assignment info.
/// These are extracted during the definition pass but distributed to their owners
/// in the enrichment pass.
pub struct VariableHint {
    pub range: std::ops::Range<usize>,
    pub name: String,
    pub type_name: Option<String>,
    pub assignment: Option<String>,
}

/// The result of the definition extraction pass.
pub struct DefinitionsResult {
    pub functions: Vec<FunctionInfo>,
    pub module_info: FunctionInfo,
    pub variable_hints: Vec<VariableHint>,
}

/// Extracts definitions (functions, containers, macros) and variable hints.
pub fn extract_definitions(
    root_node: Node,
    source: &[u8],
    _language: &tree_sitter::Language,
    config: &LanguageConfig,
    module_name: &str, 
) -> Result<DefinitionsResult, String> {
    let mut functions = Vec::new();
    let mut variable_hints = Vec::new();

    // Initialize the module-level container
    let module_info = FunctionInfo {
        name: format!("(module) {}", module_name),
        kind: SymbolKind::Module,
        is_anonymous: false,
        range_start: root_node.start_byte(),
        range_end: root_node.end_byte(),
        body_start: None, 
        source_code: String::new(),
        documentation: None,
        calls: Vec::new(),
        type_refs: Vec::new(),
        decorators: Vec::new(),
        fingerprints: HashMap::new(),
        return_type: None,
        local_types: HashMap::new(),
        local_assigns: HashMap::new(),
        config_keys: Vec::new(),
        dispatched_actions: Vec::new(),
        handled_actions: Vec::new(),
        routes: Vec::new(),
    };

    let defs_query = match config.compiled_queries.defs {
        Some(ref q) => q,
        None => return Ok(DefinitionsResult {
            functions,
            module_info,
            variable_hints,
        }),
    };

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(defs_query, root_node, source);

    while let Some(query_match) = matches.next() {
        let mut def_node = None;
        let mut name_opt: Option<String> = None;
        let mut body_start: Option<usize> = None;
        let mut kind_opt: Option<String> = None;
        let mut return_type = None;
        
        // Variable hint specific
        let mut v_name = None;
        let mut v_type = None;
        let mut v_assign = None;

        for capture in query_match.captures {
            let cap_name = defs_query.capture_names()[capture.index as usize];
            let text = capture.node.utf8_text(source).unwrap_or("");

            match cap_name {
                "function.definition" => {
                    def_node = Some(capture.node);
                }
                "function.name" => {
                    name_opt = Some(text.to_string());
                }
                "function.body" => { 
                    body_start = Some(capture.node.start_byte());
                }
                "function.kind" => {
                    kind_opt = Some(text.to_string());
                }
                "function.return_type" => {
                    return_type = Some(
                        text
                            .trim_start_matches(|c| c == ':' || c == '=' || c == '>')
                            .trim()
                            .to_string()
                    );
                }
                "variable.name" => {
                    v_name = Some(text.to_string());
                    // Check for factory pattern assignments: const x = create(...)
                    if let Some(parent) = capture.node.parent() {
                        if let Some(val) = parent.child_by_field_name("value") {
                            if val.kind() == "call_expression" {
                                if let Some(function_node) = val.child_by_field_name("function") {
                                    let fn_name = if function_node.kind() == "member_expression" {
                                        function_node.child_by_field_name("property")
                                            .and_then(|p| p.utf8_text(source).ok())
                                            .unwrap_or("")
                                    } else {
                                        function_node.utf8_text(source).unwrap_or("")
                                    };
                                    if !fn_name.is_empty() {
                                        v_assign = Some(fn_name.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
                "variable.type" => {
                    v_type = Some(text.trim_start_matches(':').trim().to_string());
                }
                _ => {}
            }
        }

        if let Some(node) = def_node {
            let node_kind = node.kind();
            
            // Determine the "Kind" of definition
            let kind = if let Some(kind_str) = kind_opt {
                match kind_str.as_str() {
                    "resource" | "data" => SymbolKind::Resource,
                    "variable" | "output" | "provider" => SymbolKind::Variable,
                    _ => SymbolKind::Function, // Fallback for captured strings
                }
            } else if node_kind == "macro_definition" {
                SymbolKind::Macro
            } else if node_kind == "macro_invocation" {
                SymbolKind::MacroGenerated
            } else if
                node_kind.contains("class") ||
                node_kind.contains("interface") ||
                node_kind.contains("struct") ||
                node_kind.contains("impl") ||
                node_kind.contains("module")
            {
                SymbolKind::Container
            } else {
                SymbolKind::Function
            };

            functions.push(FunctionInfo {
                name: name_opt.clone().unwrap_or_else(|| "anonymous".to_string()),
                kind,
                is_anonymous: name_opt.is_none(),
                range_start: node.start_byte(),
                range_end: node.end_byte(),
                body_start,
                source_code: node.utf8_text(source).unwrap_or("").to_string(),
                documentation: None,
                calls: Vec::new(),
                type_refs: Vec::new(),
                decorators: Vec::new(),
                fingerprints: HashMap::new(),
                return_type,
                local_types: HashMap::new(),
                local_assigns: HashMap::new(),
                config_keys: Vec::new(),
                dispatched_actions: Vec::new(),
                handled_actions: Vec::new(),
                routes: Vec::new(),
            });
        } else if let Some(vn) = v_name {
            // It wasn't a function definition, but it matched variable captures
            variable_hints.push(VariableHint {
                range: query_match.captures[0].node.byte_range(),
                name: vn,
                type_name: v_type,
                assignment: v_assign,
            });
        }
    }

    Ok(DefinitionsResult {
        functions,
        module_info,
        variable_hints,
    })
}