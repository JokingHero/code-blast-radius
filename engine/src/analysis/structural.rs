//! Extracts file-level structural information like imports, exports, literals, etc.

use std::collections::HashMap;
use tree_sitter::{Node, QueryCursor, StreamingIterator};
use crate::analysis::language::{LanguageConfig, SupportedLanguage};
use crate::models::{ImportNode, ExportNode};

/// A container for all extracted file-level (non-function-specific) data.
pub struct StructuralData {
    pub imports: Vec<ImportNode>,
    pub exports: Vec<ExportNode>,
    pub literals: Vec<String>,
    pub implementations: Vec<(String, String)>,
    pub middleware_usage: Vec<String>,
    pub defined_routes: Vec<String>,
}

/// Extracts all structural data from a source file.
///
/// This function is responsible for finding imports, exports, string literals,
/// class/interface implementations, middleware usage patterns, and explicitly
/// defined routes. It uses the provided `constants` map to resolve dynamic values.
pub fn extract_structure(
    root_node: Node,
    source: &[u8],
    _language: &tree_sitter::Language,
    config: &LanguageConfig,
    constants: &HashMap<String, String>
) -> StructuralData {
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut literals = Vec::new();
    let mut implementations = Vec::new();
    let mut middleware_usage = Vec::new();
    let mut defined_routes = Vec::new();

    // --- 1. Imports ---
    // --- 1. Imports ---
    if let Some(ref q) = config.compiled_queries.imports {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(q, root_node, source);
        while let Some(m) = matches.next() {
            let mut src = String::new();
            let mut name = String::new();
            let mut alias = None;
            for cap in m.captures {
                let text = cap.node.utf8_text(source).unwrap_or("").to_string();
                let capture_name = q.capture_names()[cap.index as usize];
                if capture_name == "import.source" {
                    if let Some(resolved) = constants.get(&text) {
                        src = resolved.clone();
                    } else {
                        src = text.replace(['"', '\''], "");
                    }
                } else if capture_name == "import.dynamic" {
                    if let Some(resolved) = constants.get(&text) {
                        src = resolved.clone();
                        src = src.replace(['"', '\'', '`'], "");
                    }
                } else if capture_name == "import.name" {
                    name = text;
                } else if capture_name == "import.alias" {
                    name = "*".to_string();
                    alias = Some(text);
                }
            }
            if !src.is_empty() {
                if config.lang == SupportedLanguage::Python {
                    if src.contains('.') && !src.starts_with("./") && !src.starts_with("../") {
                        if src != "." && src != ".." {
                            src = src.replace('.', "/");
                        }
                    }
                }
                imports.push(ImportNode { name, source: src, alias });
            }
        }
    }

    // --- 2. Exports ---
    // --- 2. Exports ---
    if let Some(ref q) = config.compiled_queries.exports {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(q, root_node, source);
        while let Some(m) = matches.next() {
            let mut src = String::new();
            let mut name = None;
            for cap in m.captures {
                let text = cap.node.utf8_text(source).unwrap_or("").to_string();
                let capture_name = q.capture_names()[cap.index as usize];
                if capture_name == "export.source" {
                    if let Some(resolved) = constants.get(&text) {
                        src = resolved.clone();
                    } else {
                        src = text.replace(['"', '\''], "");
                    }
                } else if capture_name == "export.name" {
                    name = Some(text);
                }
            }
            if !src.is_empty() {
                exports.push(ExportNode { name, source: src });
            }
        }
    }

    // --- 3. Literals & Template Expansion ---
    // --- 3. Literals & Template Expansion ---
    if let Some(ref q) = config.compiled_queries.literals {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(q, root_node, source);

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let node = cap.node;
                let node_kind = node.kind();

                if node_kind == "template_string" || node_kind == "string" {
                    let mut synthetic = String::new();
                    let mut is_complex = false;

                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        let k = child.kind();

                        if k == "string_fragment" || k == "string_content" {
                            synthetic.push_str(&child.utf8_text(source).unwrap_or(""));
                        } else if k == "template_substitution" || k == "interpolation" {
                            is_complex = true;
                            let mut found_const = false;

                            let mut sub_cursor = child.walk();
                            for sub_child in child.children(&mut sub_cursor) {
                                if sub_child.kind() == "identifier" {
                                    let var_name = sub_child
                                        .utf8_text(source)
                                        .unwrap_or("");
                                    if let Some(val) = constants.get(var_name) {
                                        let raw_val = val.trim_matches(
                                            |c| c == '"' || c == '\'' || c == '`'
                                        );
                                        synthetic.push_str(raw_val);
                                        found_const = true;
                                    }
                                    break;
                                }
                            }

                            if !found_const {
                                synthetic.push('*');
                            }
                        }
                    }

                    if is_complex && !synthetic.is_empty() {
                        literals.push(synthetic.clone());
                    }
                }

                let text = node
                    .utf8_text(source)
                    .unwrap_or("")
                    .trim_matches(|c| c == '"' || c == '\'' || c == '`')
                    .to_string();

                if text.len() > 1 {
                    literals.push(text);
                }
            }
        }
    }
    for val in constants.values() {
        if val.len() > 1 {
            literals.push(val.clone());
        }
    }

    // --- 4. Implementations ---
    // --- 4. Implementations ---
    if let Some(ref q) = config.compiled_queries.implements {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(q, root_node, source);
        while let Some(m) = matches.next() {
            let mut child = String::new();
            let mut parent = String::new();
            for cap in m.captures {
                let name = q.capture_names()[cap.index as usize];
                let text = cap.node.utf8_text(source).unwrap_or("").to_string();
                if name == "impl.child" {
                    child = text;
                } else if name == "impl.parent" {
                    parent = text;
                }
            }
            if !child.is_empty() && !parent.is_empty() {
                implementations.push((child, parent));
            }
        }
    }
    
    // --- 5. Explicit Route Definitions ---
    // --- 5. Explicit Route Definitions ---
    if let Some(ref q) = config.compiled_queries.route_defs {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(q, root_node, source);

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let text = cap.node
                    .utf8_text(source)
                    .unwrap_or("")
                    .trim_matches(|c| c == '"' || c == '\'' || c == '`');

                let route = if text.starts_with('/') {
                    text.to_string()
                } else {
                    format!("/{}", text)
                };

                if route.len() > 1 {
                    defined_routes.push(route);
                }
            }
        }
    }

    // --- 6. Middleware Usage ---
    if let Some(ref q) = config.compiled_queries.middleware {
        let mut mw_cursor = QueryCursor::new();
        let mut mw_matches = mw_cursor.matches(q, root_node, source);

        while let Some(m) = mw_matches.next() {
            for cap in m.captures {
                let capture_name = q.capture_names()[cap.index as usize];
                let text = cap.node.utf8_text(source).unwrap_or("").to_string();
                let clean_text = text
                    .trim_matches(|c| c == '"' || c == '\'' || c == '`')
                    .to_string();

                if capture_name == "middleware.use" || capture_name == "middleware.config" {
                    middleware_usage.push(clean_text);
                }
            }
        }
    }

    StructuralData {
        imports,
        exports,
        literals,
        implementations,
        middleware_usage,
        defined_routes,
    }
}