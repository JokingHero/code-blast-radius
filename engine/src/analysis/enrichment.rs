//! Enriches the function skeleton with metadata like calls, types, decorators, and docs.
//! Refactored to separate concerns into distinct passes.

use std::collections::HashMap;
use tree_sitter::{Node, QueryCursor, StreamingIterator};
use crate::analysis::language::LanguageConfig;
use crate::models::{FunctionInfo, SymbolKind};
use crate::analysis::definitions::VariableHint;

/// Holds the mutable state and read-only configuration for the enrichment process.
struct EnrichmentContext<'a> {
    functions: &'a mut Vec<FunctionInfo>,
    module_info: &'a mut FunctionInfo,
    source: &'a [u8],
    root_node: Node<'a>,
    config: &'a LanguageConfig,
    constants: &'a HashMap<String, String>,
}

impl<'a> EnrichmentContext<'a> {
    /// Helper to find which function owns a range, or if it belongs to the module.
    /// Returns: Some(index) for a function, or None for the module.
    fn find_owner(&self, start: usize, end: usize) -> Option<usize> {
        let mut best_idx = None;
        let mut smallest_len = usize::MAX;

        for (i, func) in self.functions.iter().enumerate() {
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

    /// Finds an owner, or looks for a "neighbor" function immediately following the range.
    /// Used for decorators and routes defined outside the function body.
    fn find_owner_or_neighbor(&self, start: usize, end: usize) -> Option<usize> {
        // 1. Try strict containment
        if let Some(idx) = self.find_owner(start, end) {
            return Some(idx);
        }

        // 2. Try neighbor heuristic (within 200 bytes before function start)
        for (i, func) in self.functions.iter().enumerate() {
            if func.range_start > end && func.range_start - end < 200 {
                return Some(i);
            }
        }

        None
    }
}

/// The main entry point.
#[allow(clippy::too_many_arguments)]
pub fn enrich_functions(
    functions: &mut Vec<FunctionInfo>,
    module_info: &mut FunctionInfo,
    variable_hints: Vec<VariableHint>,
    root_node: Node,
    source: &[u8],
    _language: &tree_sitter::Language,
    config: &LanguageConfig,
    constants: &HashMap<String, String>
) {
    let mut context = EnrichmentContext {
        functions,
        module_info,
        source,
        root_node,
        config,
        constants,
    };

    // Execute Passes
    pass_variable_hints(&mut context, variable_hints);
    pass_config_keys(&mut context);
    pass_decorators(&mut context);
    pass_calls(&mut context);
    pass_type_refs(&mut context);
    pass_actions(&mut context);
    pass_routes(&mut context);
    pass_documentation(&mut context);
    
    // Cleanup
    finalize_functions(&mut context);
}

// --- Individual Passes ---

fn pass_variable_hints(context: &mut EnrichmentContext, hints: Vec<VariableHint>) {
    // Identify constructors for TypeScript parameter promotion
    let class_indices: Vec<usize> = context.functions
        .iter()
        .enumerate()
        .filter(|(_, f)| f.kind == SymbolKind::Container)
        .map(|(idx, _)| idx)
        .collect();

    for hint in hints {
        if let Some(idx) = context.find_owner(hint.range.start, hint.range.end) {
            let func = &mut context.functions[idx];
            
            if let Some(type_name) = &hint.type_name {
                func.local_types.insert(hint.name.clone(), type_name.clone());
            }
            if let Some(assignment) = &hint.assignment {
                func.local_assigns.insert(hint.name.clone(), assignment.clone());
            }

            // Handle Constructor Promotion
            if func.name == "constructor" {
                for &class_idx in &class_indices {
                    let class_func = &context.functions[class_idx];
                    if hint.range.start >= class_func.range_start && hint.range.end <= class_func.range_end {
                        if let Some(type_name) = &hint.type_name {
                            context.functions[class_idx].local_types.insert(hint.name.clone(), type_name.clone());
                        }
                        if let Some(assignment) = &hint.assignment {
                            context.functions[class_idx].local_assigns.insert(hint.name.clone(), assignment.clone());
                        }
                        break;
                    }
                }
            }
        } else {
            // Module Scope
            if let Some(type_name) = hint.type_name {
                context.module_info.local_types.insert(hint.name.clone(), type_name);
            }
            if let Some(assignment) = hint.assignment {
                context.module_info.local_assigns.insert(hint.name, assignment);
            }
        }
    }
}

fn pass_config_keys(context: &mut EnrichmentContext) {
    if let Some(ref q) = context.config.compiled_queries.config {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(q, context.root_node, context.source);
        
        while let Some(query_match) = matches.next() {
            for capture in query_match.captures {
                if q.capture_names()[capture.index as usize] == "config.key" {
                    let text = capture.node
                        .utf8_text(context.source)
                        .unwrap_or("")
                        .trim_matches(|c| c == '"' || c == '\'' || c == '`')
                        .to_string();

                    if !text.is_empty() {
                        let range = capture.node.byte_range();
                        if let Some(idx) = context.find_owner(range.start, range.end) {
                            context.functions[idx].config_keys.push(text);
                        } else {
                            context.module_info.config_keys.push(text);
                        }
                    }
                }
            }
        }
    }
}

fn pass_decorators(context: &mut EnrichmentContext) {
    if let Some(ref q) = context.config.compiled_queries.decorators {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(q, context.root_node, context.source);
        
        while let Some(query_match) = matches.next() {
            for capture in query_match.captures {
                let text = capture.node.utf8_text(context.source).unwrap_or("").to_string();
                let clean_name = text
                    .trim_matches(|c| c == '@' || c == '#' || c == '[' || c == ']' || c == '(' || c == ')')
                    .to_string();

                if !clean_name.is_empty() {
                    let range = capture.node.byte_range();
                    if let Some(idx) = context.find_owner_or_neighbor(range.start, range.end) {
                        context.functions[idx].decorators.push(clean_name);
                    } else {
                        context.module_info.decorators.push(clean_name);
                    }
                }
            }
        }
    }
}

fn pass_calls(context: &mut EnrichmentContext) {
    if let Some(ref q) = context.config.compiled_queries.calls {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(q, context.root_node, context.source);
        
        while let Some(query_match) = matches.next() {
            let mut method_name = None;
            let mut receiver_name = None;
            let mut dynamic_receiver = None;
            let mut call_range = None;

            for capture in query_match.captures {
                let t = capture.node.utf8_text(context.source).unwrap_or("").to_string();
                let cap_name = q.capture_names()[capture.index as usize];

                if cap_name == "call.name" {
                    method_name = Some(t);
                    call_range = Some(capture.node.byte_range());
                } else if cap_name == "call.receiver" {
                    receiver_name = Some(t);
                } else if cap_name == "call.dynamic_dispatch" {
                    dynamic_receiver = Some(t);
                    call_range = Some(capture.node.byte_range());
                }
            }

            if let (Some(m), Some(range)) = (method_name, call_range.clone()) {
                if let Some(idx) = context.find_owner(range.start, range.end) {
                    let func = &mut context.functions[idx];
                    if receiver_name.is_none() {
                        func.calls.push(m.clone());
                    }
                    if let Some(r) = receiver_name {
                        func.fingerprints.entry(r).or_default().push(m);
                    }
                } else {
                    if receiver_name.is_none() {
                        context.module_info.calls.push(m.clone());
                    }
                    if let Some(r) = receiver_name {
                        context.module_info.fingerprints.entry(r).or_default().push(m);
                    }
                }
            } else if let (Some(dr), Some(range)) = (dynamic_receiver, call_range) {
                if let Some(idx) = context.find_owner(range.start, range.end) {
                    context.functions[idx].fingerprints.entry(dr).or_default().push("*".to_string());
                } else {
                    context.module_info.fingerprints.entry(dr).or_default().push("*".to_string());
                }
            }
        }
    }
}

fn pass_type_refs(context: &mut EnrichmentContext) {
    if let Some(ref q) = context.config.compiled_queries.types {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(q, context.root_node, context.source);
        
        while let Some(query_match) = matches.next() {
            for capture in query_match.captures {
                let type_name = capture.node.utf8_text(context.source).unwrap_or("").to_string();
                if !type_name.is_empty() {
                    let range = capture.node.byte_range();
                    if let Some(idx) = context.find_owner(range.start, range.end) {
                        context.functions[idx].type_refs.push(type_name);
                    } else {
                        context.module_info.type_refs.push(type_name);
                    }
                }
            }
        }
    }
}

fn pass_actions(context: &mut EnrichmentContext) {
    if let Some(ref q) = context.config.compiled_queries.actions {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(q, context.root_node, context.source);
        
        while let Some(query_match) = matches.next() {
            for capture in query_match.captures {
                let raw_text = capture.node.utf8_text(context.source).unwrap_or("").to_string();
                
                // Resolve constant if possible
                let resolved_text = if let Some(val) = context.constants.get(&raw_text) {
                    val.clone()
                } else {
                    raw_text
                };

                let text = resolved_text
                    .trim_matches(|c| c == '"' || c == '\'' || c == '`')
                    .to_string();
                
                let capture_name = q.capture_names()[capture.index as usize];
                let range = capture.node.byte_range();

                // Actions often use the neighbor heuristic (e.g. decorators handling events)
                // but dispatching is usually inside the function.
                if let Some(idx) = context.find_owner(range.start, range.end) {
                    if capture_name == "action.dispatch" {
                        context.functions[idx].dispatched_actions.push(text);
                    } else if capture_name == "action.handle" {
                        context.functions[idx].handled_actions.push(text);
                    }
                } else if let Some(idx) = context.find_owner_or_neighbor(range.start, range.end) {
                    if capture_name == "action.handle" {
                        context.functions[idx].handled_actions.push(text);
                    }
                } else {
                    if capture_name == "action.dispatch" {
                        context.module_info.dispatched_actions.push(text);
                    } else if capture_name == "action.handle" {
                        context.module_info.handled_actions.push(text);
                    }
                }
            }
        }
    }
}

fn pass_routes(context: &mut EnrichmentContext) {
    if let Some(ref q) = context.config.compiled_queries.route_defs {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(q, context.root_node, context.source);
        
        while let Some(query_match) = matches.next() {
            for capture in query_match.captures {
                let text = capture.node
                    .utf8_text(context.source)
                    .unwrap_or("")
                    .trim_matches(|c| c == '"' || c == '\'' || c == '`');

                let route = if text.starts_with('/') {
                    text.to_string()
                } else {
                    format!("/{}", text)
                };

                if route.len() > 1 {
                    let range = capture.node.byte_range();
                    if let Some(idx) = context.find_owner_or_neighbor(range.start, range.end) {
                        context.functions[idx].routes.push(route);
                    } else {
                        context.module_info.routes.push(route);
                    }
                }
            }
        }
    }
}

fn pass_documentation(context: &mut EnrichmentContext) {
    if let Some(ref q) = context.config.compiled_queries.docs {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(q, context.root_node, context.source);
        
        while let Some(query_match) = matches.next() {
            // Find the definition node to match ranges
            let d_def = query_match.captures
                .iter()
                .find(|c| q.capture_names()[c.index as usize] == "function.definition")
                .map(|c| c.node);

            if let Some(d_node) = d_def {
                for func in context.functions.iter_mut() {
                    if func.range_start == d_node.start_byte() {
                        func.documentation = Some(
                            query_match.captures
                                .iter()
                                .filter(|c| q.capture_names()[c.index as usize] == "function.docs")
                                .map(|c| c.node.utf8_text(context.source).unwrap_or("").to_string())
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

fn finalize_functions(context: &mut EnrichmentContext) {
    let clean_func = |f: &mut FunctionInfo| {
        f.config_keys.sort(); f.config_keys.dedup();
        f.type_refs.sort(); f.type_refs.dedup();
        f.decorators.sort(); f.decorators.dedup();
        f.dispatched_actions.sort(); f.dispatched_actions.dedup();
        f.handled_actions.sort(); f.handled_actions.dedup();
        f.routes.sort(); f.routes.dedup();
    };

    for func in context.functions.iter_mut() {
        clean_func(func);
    }
    clean_func(context.module_info);
}