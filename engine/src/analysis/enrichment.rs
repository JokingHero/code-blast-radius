//! Enriches the function skeleton with metadata like calls, types, decorators, and docs.
//! This module performs the "heavy lifting" of linking specific code patterns (like
//! API routes or database calls) to the functions that contain them.

use std::collections::HashMap;
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};
use crate::analysis::language::LanguageConfig;
use crate::models::FunctionInfo;
use crate::analysis::definitions::VariableHint;

/// Helper to determine which function "owns" a specific byte range.
/// Returns the index of the most specific (smallest) function containing the range.
fn get_owner_index(start: usize, end: usize, funcs: &[FunctionInfo]) -> Option<usize> {
    let mut best_idx = None;
    let mut smallest_len = usize::MAX;

    for (i, func) in funcs.iter().enumerate() {
        if start >= func.range_start && end <= func.range_end {
            let len = func.range_end - func.range_start;
            if len < smallest_len {
                smallest_len = len;
                best_idx = Some(i);
            }
        }
    }
    best_idx
}

/// The main enrichment pass. Mutates `functions` and `module_info` in place.
#[allow(clippy::too_many_arguments)]
pub fn enrich_functions(
    functions: &mut Vec<FunctionInfo>,
    module_info: &mut FunctionInfo,
    variable_hints: Vec<VariableHint>,
    root_node: Node,
    source: &[u8],
    language: &tree_sitter::Language,
    config: &LanguageConfig,
    constants: &HashMap<String, String>
) {
    // --- 1. Distribute Variable Hints (Step 7) ---
    // Identify class indices to handle constructor parameter promotion (TypeScript)
    let class_indices: Vec<usize> = functions
        .iter()
        .enumerate()
        .filter(|(_, f)| f.kind == "container")
        .map(|(i, _)| i)
        .collect();

    for hint in variable_hints {
        if let Some(idx) = get_owner_index(hint.range.start, hint.range.end, functions) {
            let func = &mut functions[idx];
            
            if let Some(t) = hint.type_name.clone() {
                func.local_types.insert(hint.name.clone(), t);
            }
            if let Some(a) = hint.assignment.clone() {
                func.local_assigns.insert(hint.name.clone(), a);
            }

            // If this is a constructor, also add to the parent class context
            if func.name == "constructor" {
                for &class_idx in &class_indices {
                    let class_func = &functions[class_idx];
                    if hint.range.start >= class_func.range_start && hint.range.end <= class_func.range_end {
                        // Access the parent class mutably via index
                        if let Some(t) = hint.type_name.clone() {
                            functions[class_idx].local_types.insert(hint.name.clone(), t);
                        }
                        if let Some(a) = hint.assignment.clone() {
                            functions[class_idx].local_assigns.insert(hint.name.clone(), a);
                        }
                        break;
                    }
                }
            }
        } else {
            // Belongs to module scope
            if let Some(t) = hint.type_name {
                module_info.local_types.insert(hint.name.clone(), t);
            }
            if let Some(a) = hint.assignment {
                module_info.local_assigns.insert(hint.name.clone(), a);
            }
        }
    }

    // --- 2. Config Keys (Step 8) ---
    if !config.query_config.is_empty() {
        if let Ok(q) = Query::new(language, config.query_config) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&q, root_node, source);
            while let Some(m) = matches.next() {
                for cap in m.captures {
                    if q.capture_names()[cap.index as usize] == "config.key" {
                        let text = cap.node
                            .utf8_text(source)
                            .unwrap_or("")
                            .trim_matches(|c| c == '"' || c == '\'' || c == '`')
                            .to_string();

                        if !text.is_empty() {
                            let range = cap.node.byte_range();
                            if let Some(idx) = get_owner_index(range.start, range.end, functions) {
                                functions[idx].config_keys.push(text);
                            } else {
                                module_info.config_keys.push(text);
                            }
                        }
                    }
                }
            }
        }
    }

    // --- 3. Decorators (Step 8.5) ---
    if !config.query_decorators.is_empty() {
        if let Ok(q) = Query::new(language, config.query_decorators) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&q, root_node, source);
            while let Some(m) = matches.next() {
                for cap in m.captures {
                    let text = cap.node.utf8_text(source).unwrap_or("").to_string();
                    let clean_name = text
                        .trim_matches(|c| c == '@' || c == '#' || c == '[' || c == ']' || c == '(' || c == ')')
                        .to_string();

                    if !clean_name.is_empty() {
                        let range = cap.node.byte_range();
                        if let Some(idx) = get_owner_index(range.start, range.end, functions) {
                            functions[idx].decorators.push(clean_name);
                        } else {
                            // Neighbor check: Decorator often sits immediately BEFORE the function definition,
                            // technically outside the function body range.
                            let mut found_neighbor = false;
                            for func in functions.iter_mut() {
                                if func.range_start > range.end && func.range_start - range.end < 200 {
                                    func.decorators.push(clean_name.clone());
                                    found_neighbor = true;
                                    break;
                                }
                            }
                            if !found_neighbor {
                                module_info.decorators.push(clean_name);
                            }
                        }
                    }
                }
            }
        }
    }

    // --- 4. Function Calls (Step 9) ---
    if let Ok(q) = Query::new(language, config.query_calls) {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&q, root_node, source);
        while let Some(m) = matches.next() {
            let mut m_name = None;
            let mut r_name = None;
            let mut dynamic_receiver = None;
            let mut call_range = None;

            for cap in m.captures {
                let t = cap.node.utf8_text(source).unwrap_or("").to_string();
                let cap_name = q.capture_names()[cap.index as usize];

                if cap_name == "call.name" {
                    m_name = Some(t);
                    call_range = Some(cap.node.byte_range());
                } else if cap_name == "call.receiver" {
                    r_name = Some(t);
                } else if cap_name == "call.dynamic_dispatch" {
                    dynamic_receiver = Some(t);
                    call_range = Some(cap.node.byte_range());
                }
            }

            if let (Some(m), Some(range)) = (m_name, call_range.clone()) {
                if let Some(idx) = get_owner_index(range.start, range.end, functions) {
                    let func = &mut functions[idx];
                    if r_name.is_none() {
                        func.calls.push(m.clone());
                    }
                    if let Some(r) = r_name {
                        func.fingerprints.entry(r).or_default().push(m);
                    }
                } else {
                    if r_name.is_none() {
                        module_info.calls.push(m.clone());
                    }
                    if let Some(r) = r_name {
                        module_info.fingerprints.entry(r).or_default().push(m);
                    }
                }
            } else if let (Some(dr), Some(range)) = (dynamic_receiver, call_range) {
                if let Some(idx) = get_owner_index(range.start, range.end, functions) {
                    functions[idx].fingerprints.entry(dr).or_default().push("*".to_string());
                } else {
                    module_info.fingerprints.entry(dr).or_default().push("*".to_string());
                }
            }
        }
    }

    // --- 5. Type References (Step 9.5) ---
    if !config.query_types.is_empty() {
        if let Ok(q) = Query::new(language, config.query_types) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&q, root_node, source);
            while let Some(m) = matches.next() {
                for cap in m.captures {
                    let type_name = cap.node.utf8_text(source).unwrap_or("").to_string();
                    if !type_name.is_empty() {
                        let range = cap.node.byte_range();
                        if let Some(idx) = get_owner_index(range.start, range.end, functions) {
                            functions[idx].type_refs.push(type_name);
                        } else {
                            module_info.type_refs.push(type_name);
                        }
                    }
                }
            }
        }
    }

    // --- 6. State Actions (Step 9.6) ---
    if !config.query_actions.is_empty() {
        if let Ok(q) = Query::new(language, config.query_actions) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&q, root_node, source);
            while let Some(m) = matches.next() {
                for cap in m.captures {
                    let raw_text = cap.node.utf8_text(source).unwrap_or("").to_string();
                    let resolved_text = if let Some(val) = constants.get(&raw_text) {
                        val.clone()
                    } else {
                        raw_text
                    };

                    let text = resolved_text
                        .trim_matches(|c| c == '"' || c == '\'' || c == '`')
                        .to_string();
                    
                    let capture_name = q.capture_names()[cap.index as usize];
                    let range = cap.node.byte_range();

                    if let Some(idx) = get_owner_index(range.start, range.end, functions) {
                        if capture_name == "action.dispatch" {
                            functions[idx].dispatched_actions.push(text);
                        } else if capture_name == "action.handle" {
                            functions[idx].handled_actions.push(text);
                        }
                    } else {
                        // Neighbor check for handlers
                        let mut found_neighbor = false;
                        if capture_name == "action.handle" {
                            for func in functions.iter_mut() {
                                if func.range_start > range.end && func.range_start - range.end < 200 {
                                    func.handled_actions.push(text.clone());
                                    found_neighbor = true;
                                    break;
                                }
                            }
                        }
                        if !found_neighbor {
                            if capture_name == "action.dispatch" {
                                module_info.dispatched_actions.push(text);
                            } else if capture_name == "action.handle" {
                                module_info.handled_actions.push(text);
                            }
                        }
                    }
                }
            }
        }
    }

    // --- 7. Route Definitions (Step 9.6b) ---
    // Attaches explicit routes to functions (e.g. @Get('/users') -> getUsers)
    if !config.query_route_defs.is_empty() {
        if let Ok(q) = Query::new(language, config.query_route_defs) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&q, root_node, source);
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
                        let range = cap.node.byte_range();
                        if let Some(idx) = get_owner_index(range.start, range.end, functions) {
                            functions[idx].routes.push(route);
                        } else {
                            // Neighbor check
                            let mut found_neighbor = false;
                            for func in functions.iter_mut() {
                                if func.range_start > range.end && func.range_start - range.end < 200 {
                                    func.routes.push(route.clone());
                                    found_neighbor = true;
                                    break;
                                }
                            }
                            if !found_neighbor {
                                module_info.routes.push(route);
                            }
                        }
                    }
                }
            }
        }
    }

    // --- Cleanup: Sort and Dedup ---
    for func in functions.iter_mut() {
        func.config_keys.sort();
        func.config_keys.dedup();
        func.type_refs.sort();
        func.type_refs.dedup();
        func.decorators.sort();
        func.decorators.dedup();
        func.dispatched_actions.sort();
        func.dispatched_actions.dedup();
        func.handled_actions.sort();
        func.handled_actions.dedup();
    }
    module_info.config_keys.sort();
    module_info.config_keys.dedup();
    module_info.type_refs.sort();
    module_info.type_refs.dedup();
    module_info.decorators.sort();
    module_info.decorators.dedup();

    // --- 8. Documentation (Step 10) ---
    // Extract docs last so we can match them to the final function ranges
    if let Ok(q) = Query::new(language, config.query_docs) {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&q, root_node, source);
        while let Some(m) = matches.next() {
            let d_def = m.captures
                .iter()
                .find(|c| q.capture_names()[c.index as usize] == "function.definition")
                .map(|c| c.node);

            if let Some(d_node) = d_def {
                for func in functions.iter_mut() {
                    if func.range_start == d_node.start_byte() {
                        func.documentation = Some(
                            m.captures
                                .iter()
                                .filter(|c| q.capture_names()[c.index as usize] == "function.docs")
                                .map(|c| c.node.utf8_text(source).unwrap_or("").to_string())
                                .collect::<Vec<_>>()
                                .join("\n")
                        );
                        break;
                    }
                }
            }
        }
    }
}