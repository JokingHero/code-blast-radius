//! Enriches the function skeleton with metadata like calls, types, decorators, and docs.
//! Refactored to separate concerns into distinct passes.

use std::collections::HashMap;
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};
use crate::analysis::language::LanguageConfig;
use crate::models::{FunctionInfo, SymbolKind};
use crate::analysis::definitions::VariableHint;

/// Holds the mutable state and read-only configuration for the enrichment process.
struct EnrichmentContext<'a> {
    functions: &'a mut Vec<FunctionInfo>,
    module_info: &'a mut FunctionInfo,
    source: &'a [u8],
    root_node: Node<'a>,
    language: &'a tree_sitter::Language,
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
    language: &tree_sitter::Language,
    config: &LanguageConfig,
    constants: &HashMap<String, String>
) {
    let mut ctx = EnrichmentContext {
        functions,
        module_info,
        source,
        root_node,
        language,
        config,
        constants,
    };

    // Execute Passes
    pass_variable_hints(&mut ctx, variable_hints);
    pass_config_keys(&mut ctx);
    pass_decorators(&mut ctx);
    pass_calls(&mut ctx);
    pass_type_refs(&mut ctx);
    pass_actions(&mut ctx);
    pass_routes(&mut ctx);
    pass_documentation(&mut ctx);
    
    // Cleanup
    finalize_functions(&mut ctx);
}

// --- Individual Passes ---

fn pass_variable_hints(ctx: &mut EnrichmentContext, hints: Vec<VariableHint>) {
    // Identify constructors for TypeScript parameter promotion
    let class_indices: Vec<usize> = ctx.functions
        .iter()
        .enumerate()
        .filter(|(_, f)| f.kind == SymbolKind::Container)
        .map(|(i, _)| i)
        .collect();

    for hint in hints {
        if let Some(idx) = ctx.find_owner(hint.range.start, hint.range.end) {
            let func = &mut ctx.functions[idx];
            
            if let Some(t) = &hint.type_name {
                func.local_types.insert(hint.name.clone(), t.clone());
            }
            if let Some(a) = &hint.assignment {
                func.local_assigns.insert(hint.name.clone(), a.clone());
            }

            // Handle Constructor Promotion
            if func.name == "constructor" {
                for &class_idx in &class_indices {
                    let class_func = &ctx.functions[class_idx];
                    if hint.range.start >= class_func.range_start && hint.range.end <= class_func.range_end {
                        if let Some(t) = &hint.type_name {
                            ctx.functions[class_idx].local_types.insert(hint.name.clone(), t.clone());
                        }
                        if let Some(a) = &hint.assignment {
                            ctx.functions[class_idx].local_assigns.insert(hint.name.clone(), a.clone());
                        }
                        break;
                    }
                }
            }
        } else {
            // Module Scope
            if let Some(t) = hint.type_name {
                ctx.module_info.local_types.insert(hint.name.clone(), t);
            }
            if let Some(a) = hint.assignment {
                ctx.module_info.local_assigns.insert(hint.name, a);
            }
        }
    }
}

fn pass_config_keys(ctx: &mut EnrichmentContext) {
    // We cannot use the generic run_query easily here because we need capture names from the Query object.
    // However, for brevity in this refactor, we'll keep the direct logic but isolated.
    if ctx.config.query_config.is_empty() { return; }

    if let Ok(q) = Query::new(ctx.language, ctx.config.query_config) {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&q, ctx.root_node, ctx.source);
        
        while let Some(m) = matches.next() {
            for cap in m.captures {
                if q.capture_names()[cap.index as usize] == "config.key" {
                    let text = cap.node
                        .utf8_text(ctx.source)
                        .unwrap_or("")
                        .trim_matches(|c| c == '"' || c == '\'' || c == '`')
                        .to_string();

                    if !text.is_empty() {
                        let range = cap.node.byte_range();
                        if let Some(idx) = ctx.find_owner(range.start, range.end) {
                            ctx.functions[idx].config_keys.push(text);
                        } else {
                            ctx.module_info.config_keys.push(text);
                        }
                    }
                }
            }
        }
    }
}

fn pass_decorators(ctx: &mut EnrichmentContext) {
    if ctx.config.query_decorators.is_empty() { return; }

    if let Ok(q) = Query::new(ctx.language, ctx.config.query_decorators) {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&q, ctx.root_node, ctx.source);
        
        while let Some(m) = matches.next() {
            for cap in m.captures {
                let text = cap.node.utf8_text(ctx.source).unwrap_or("").to_string();
                let clean_name = text
                    .trim_matches(|c| c == '@' || c == '#' || c == '[' || c == ']' || c == '(' || c == ')')
                    .to_string();

                if !clean_name.is_empty() {
                    let range = cap.node.byte_range();
                    if let Some(idx) = ctx.find_owner_or_neighbor(range.start, range.end) {
                        ctx.functions[idx].decorators.push(clean_name);
                    } else {
                        ctx.module_info.decorators.push(clean_name);
                    }
                }
            }
        }
    }
}

fn pass_calls(ctx: &mut EnrichmentContext) {
    if ctx.config.query_calls.is_empty() { return; }

    if let Ok(q) = Query::new(ctx.language, ctx.config.query_calls) {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&q, ctx.root_node, ctx.source);
        
        while let Some(m) = matches.next() {
            let mut m_name = None;
            let mut r_name = None;
            let mut dynamic_receiver = None;
            let mut call_range = None;

            for cap in m.captures {
                let t = cap.node.utf8_text(ctx.source).unwrap_or("").to_string();
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
                if let Some(idx) = ctx.find_owner(range.start, range.end) {
                    let func = &mut ctx.functions[idx];
                    if r_name.is_none() {
                        func.calls.push(m.clone());
                    }
                    if let Some(r) = r_name {
                        func.fingerprints.entry(r).or_default().push(m);
                    }
                } else {
                    if r_name.is_none() {
                        ctx.module_info.calls.push(m.clone());
                    }
                    if let Some(r) = r_name {
                        ctx.module_info.fingerprints.entry(r).or_default().push(m);
                    }
                }
            } else if let (Some(dr), Some(range)) = (dynamic_receiver, call_range) {
                if let Some(idx) = ctx.find_owner(range.start, range.end) {
                    ctx.functions[idx].fingerprints.entry(dr).or_default().push("*".to_string());
                } else {
                    ctx.module_info.fingerprints.entry(dr).or_default().push("*".to_string());
                }
            }
        }
    }
}

fn pass_type_refs(ctx: &mut EnrichmentContext) {
    if ctx.config.query_types.is_empty() { return; }

    if let Ok(q) = Query::new(ctx.language, ctx.config.query_types) {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&q, ctx.root_node, ctx.source);
        
        while let Some(m) = matches.next() {
            for cap in m.captures {
                let type_name = cap.node.utf8_text(ctx.source).unwrap_or("").to_string();
                if !type_name.is_empty() {
                    let range = cap.node.byte_range();
                    if let Some(idx) = ctx.find_owner(range.start, range.end) {
                        ctx.functions[idx].type_refs.push(type_name);
                    } else {
                        ctx.module_info.type_refs.push(type_name);
                    }
                }
            }
        }
    }
}

fn pass_actions(ctx: &mut EnrichmentContext) {
    if ctx.config.query_actions.is_empty() { return; }

    if let Ok(q) = Query::new(ctx.language, ctx.config.query_actions) {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&q, ctx.root_node, ctx.source);
        
        while let Some(m) = matches.next() {
            for cap in m.captures {
                let raw_text = cap.node.utf8_text(ctx.source).unwrap_or("").to_string();
                
                // Resolve constant if possible
                let resolved_text = if let Some(val) = ctx.constants.get(&raw_text) {
                    val.clone()
                } else {
                    raw_text
                };

                let text = resolved_text
                    .trim_matches(|c| c == '"' || c == '\'' || c == '`')
                    .to_string();
                
                let capture_name = q.capture_names()[cap.index as usize];
                let range = cap.node.byte_range();

                // Actions often use the neighbor heuristic (e.g. decorators handling events)
                // but dispatching is usually inside the function.
                if let Some(idx) = ctx.find_owner(range.start, range.end) {
                    if capture_name == "action.dispatch" {
                        ctx.functions[idx].dispatched_actions.push(text);
                    } else if capture_name == "action.handle" {
                        ctx.functions[idx].handled_actions.push(text);
                    }
                } else if let Some(idx) = ctx.find_owner_or_neighbor(range.start, range.end) {
                    if capture_name == "action.handle" {
                         ctx.functions[idx].handled_actions.push(text);
                    }
                } else {
                     if capture_name == "action.dispatch" {
                        ctx.module_info.dispatched_actions.push(text);
                    } else if capture_name == "action.handle" {
                        ctx.module_info.handled_actions.push(text);
                    }
                }
            }
        }
    }
}

fn pass_routes(ctx: &mut EnrichmentContext) {
    if ctx.config.query_route_defs.is_empty() { return; }

    if let Ok(q) = Query::new(ctx.language, ctx.config.query_route_defs) {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&q, ctx.root_node, ctx.source);
        
        while let Some(m) = matches.next() {
            for cap in m.captures {
                let text = cap.node
                    .utf8_text(ctx.source)
                    .unwrap_or("")
                    .trim_matches(|c| c == '"' || c == '\'' || c == '`');

                let route = if text.starts_with('/') {
                    text.to_string()
                } else {
                    format!("/{}", text)
                };

                if route.len() > 1 {
                    let range = cap.node.byte_range();
                    if let Some(idx) = ctx.find_owner_or_neighbor(range.start, range.end) {
                        ctx.functions[idx].routes.push(route);
                    } else {
                        ctx.module_info.routes.push(route);
                    }
                }
            }
        }
    }
}

fn pass_documentation(ctx: &mut EnrichmentContext) {
    if ctx.config.query_docs.is_empty() { return; }

    if let Ok(q) = Query::new(ctx.language, ctx.config.query_docs) {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&q, ctx.root_node, ctx.source);
        
        while let Some(m) = matches.next() {
            // Find the definition node to match ranges
            let d_def = m.captures
                .iter()
                .find(|c| q.capture_names()[c.index as usize] == "function.definition")
                .map(|c| c.node);

            if let Some(d_node) = d_def {
                for func in ctx.functions.iter_mut() {
                    if func.range_start == d_node.start_byte() {
                        func.documentation = Some(
                            m.captures
                                .iter()
                                .filter(|c| q.capture_names()[c.index as usize] == "function.docs")
                                .map(|c| c.node.utf8_text(ctx.source).unwrap_or("").to_string())
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

fn finalize_functions(ctx: &mut EnrichmentContext) {
    let clean_func = |f: &mut FunctionInfo| {
        f.config_keys.sort(); f.config_keys.dedup();
        f.type_refs.sort(); f.type_refs.dedup();
        f.decorators.sort(); f.decorators.dedup();
        f.dispatched_actions.sort(); f.dispatched_actions.dedup();
        f.handled_actions.sort(); f.handled_actions.dedup();
        f.routes.sort(); f.routes.dedup();
    };

    for func in ctx.functions.iter_mut() {
        clean_func(func);
    }
    clean_func(ctx.module_info);
}