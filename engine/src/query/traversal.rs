use std::collections::{HashMap, HashSet, VecDeque};

use crate::models::{SymbolId, SymbolKind, WorkspaceIndex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum TraversalMode {
    Downstream,
    Upstream,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SliceDirection {
    Upstream,
    Downstream,
    Both,
}

pub fn find_call_chain_ids(
    index: &WorkspaceIndex,
    target_name: &str,
    direction: SliceDirection
) -> Option<Vec<SymbolId>> {
    let targets = index.symbol_map.get(target_name)?;
    if targets.is_empty() {
        return None;
    }

    let mut predecessors: HashMap<SymbolId, SymbolId> = HashMap::new();
    let mut queue: VecDeque<SymbolId> = VecDeque::new();
    let mut visited: HashSet<SymbolId> = HashSet::new();

    for &id in targets {
        queue.push_back(id);
        visited.insert(id);
    }

    while let Some(current_id) = queue.pop_front() {
        if direction == SliceDirection::Upstream || direction == SliceDirection::Both {
            for (caller_id, callees) in &index.resolved_calls {
                if callees.contains(&current_id) && !visited.contains(caller_id) {
                    visited.insert(*caller_id);
                    predecessors.insert(*caller_id, current_id);
                    queue.push_back(*caller_id);
                }
            }
        }

        if direction == SliceDirection::Downstream || direction == SliceDirection::Both {
            if let Some(callees) = index.resolved_calls.get(&current_id) {
                for &callee_id in callees {
                    if !visited.contains(&callee_id) {
                        visited.insert(callee_id);
                        predecessors.insert(callee_id, current_id);
                        queue.push_back(callee_id);
                    }
                }
            }
        }

        if let Some(children) = index.inheritance.get(&current_id) {
            for &child_id in children {
                if !visited.contains(&child_id) {
                    visited.insert(child_id);
                    predecessors.insert(child_id, current_id);
                    queue.push_back(child_id);
                }
            }
        }
    }

    let mut final_list: Vec<SymbolId> = visited.into_iter().collect();
    final_list.sort_by_key(|id| {
        let mut depth = 0;
        let mut curr = *id;
        while let Some(&p) = predecessors.get(&curr) {
            depth += 1;
            curr = p;
        }
        depth
    });

    if direction == SliceDirection::Downstream {
        final_list.reverse();
    }
    Some(final_list)
}

pub fn generate_context_from_ids(
    index: &WorkspaceIndex,
    chain: &[SymbolId],
    include_docs: bool,
    exclude_tests: bool
) -> String {
    if chain.is_empty() {
        return String::from("// No context found.");
    }

    let filtered_chain: Vec<SymbolId> = if exclude_tests {
        chain
            .iter()
            .filter(|&&id| { index.symbols.get(&id).map_or(true, |s| !s.is_test) })
            .cloned()
            .collect()
    } else {
        chain.to_vec()
    };

    if filtered_chain.is_empty() {
        return String::from("// All relevant context was filtered out (test exclusion active).");
    }

    let mut context = String::new();

    let primary_id = filtered_chain.first().unwrap();
    let primary_name = index.symbols
        .get(primary_id)
        .map(|s| s.name.as_str())
        .unwrap_or("Unknown");

    context.push_str(&format!("// Context for search: `{}`\n", primary_name));

    let names: Vec<String> = filtered_chain
        .iter()
        .filter_map(|id| index.symbols.get(id).map(|s| s.name.clone()))
        .collect();
    context.push_str(&format!("// Resolved Symbols: {}\n", names.join(", ")));

    if exclude_tests {
        context.push_str(
            "// Note: Test files and functions have been excluded from this output.\n"
        );
    }
    context.push('\n');

    let mut seen_files = HashSet::new();

    for &sym_id in &filtered_chain {
        if let Some(sym) = index.symbols.get(&sym_id) {
            if sym.is_external {
                context.push_str("// ==========================================================\n");
                context.push_str(
                    &format!(
                        "// External Library: {}\n",
                        sym.external_source.as_deref().unwrap_or("Unknown")
                    )
                );
                context.push_str("// ==========================================================\n");
                context.push_str(&format!("// Symbol: {}\n", sym.name));

                if let Some(docs) = &sym.doc_comment {
                    context.push_str(&format!("// {}\n", docs));
                }

                context.push_str("// (Source code not available for external libraries)\n");
                context.push_str("\n\n");
                continue;
            }

            if let Some(file_node) = index.files.values().find(|f| f.id == sym.file_id) {
                if !seen_files.contains(&file_node.id) {
                    context.push_str(
                        "// ==========================================================\n"
                    );
                    context.push_str(&format!("// File: {}\n", file_node.path));
                    if file_node.is_test {
                        context.push_str("// (Test File)\n");
                    }
                    context.push_str(
                        "// ==========================================================\n"
                    );
                    seen_files.insert(file_node.id);
                }

                if include_docs {
                    if let Some(docs) = &sym.doc_comment {
                        context.push_str(docs);
                        context.push('\n');
                    }
                }

                if let Ok(content) = std::fs::read_to_string(&file_node.path) {
                    if sym.range_end <= content.len() {
                        let text = String::from_utf8_lossy(
                            &content.as_bytes()[sym.range_start..sym.range_end]
                        );
                        context.push_str(&text);
                    } else {
                        context.push_str(
                            "// Error: Source range out of bounds for this file version"
                        );
                    }
                } else {
                    context.push_str("// Error: Could not read source file from disk");
                }
                context.push_str("\n\n");
            }
        }
    }

    context
}

pub fn find_related_symbols(index: &WorkspaceIndex, target_name: &str) -> Option<Vec<SymbolId>> {
    let targets = index.symbol_map.get(target_name)?;
    if targets.is_empty() {
        return None;
    }

    let mut queue: VecDeque<(SymbolId, TraversalMode)> = VecDeque::new();
    let mut visited: HashSet<(SymbolId, TraversalMode)> = HashSet::new();
    let mut result_set: HashSet<SymbolId> = HashSet::new();

    for &id in targets {
        queue.push_back((id, TraversalMode::Both));
        visited.insert((id, TraversalMode::Both));
        result_set.insert(id);
    }

    while let Some((current_id, mode)) = queue.pop_front() {
        // 1. DOWNSTREAM (Function -> Types it uses, or Function -> Calls)
        if mode == TraversalMode::Both || mode == TraversalMode::Downstream {
            if let Some(callees) = index.resolved_calls.get(&current_id) {
                for &callee_id in callees {
                    let c_name = index.symbols
                        .get(&callee_id)
                        .map(|s| s.name.clone())
                        .unwrap_or_default();
                    if c_name.contains("order.service") || c_name.contains("Order") {
                        let p_name = index.symbols
                            .get(&current_id)
                            .map(|s| s.name.clone())
                            .unwrap_or_default();
                        println!("DEBUG LEAK: {} -> {} (Call)", p_name, c_name);
                    }
                    if !visited.contains(&(callee_id, TraversalMode::Downstream)) {
                        visited.insert((callee_id, TraversalMode::Downstream));
                        result_set.insert(callee_id);
                        queue.push_back((callee_id, TraversalMode::Downstream));
                    }
                }
            }
            // Follow Type References
            if let Some(type_ids) = index.resolved_type_refs.get(&current_id) {
                for &tid in type_ids {
                    if !visited.contains(&(tid, TraversalMode::Downstream)) {
                        visited.insert((tid, TraversalMode::Downstream));
                        result_set.insert(tid);
                        queue.push_back((tid, TraversalMode::Downstream));
                    }
                }
            }
        }

        // 2. UPSTREAM (Function <- Callers, or Type <- Function using it)
        if mode == TraversalMode::Both || mode == TraversalMode::Upstream {
            for (caller_id, callees) in &index.resolved_calls {
                if callees.contains(&current_id) {
                    if !visited.contains(&(*caller_id, TraversalMode::Upstream)) {
                        visited.insert((*caller_id, TraversalMode::Upstream));
                        result_set.insert(*caller_id);
                        queue.push_back((*caller_id, TraversalMode::Upstream));
                    }
                }
            }
            // Find functions that use this Type
            for (func_id, used_types) in &index.resolved_type_refs {
                if used_types.contains(&current_id) {
                    if !visited.contains(&(*func_id, TraversalMode::Upstream)) {
                        visited.insert((*func_id, TraversalMode::Upstream));
                        result_set.insert(*func_id);
                        queue.push_back((*func_id, TraversalMode::Upstream));
                    }
                }
            }
        }

        // 3. STRUCTURAL
        if let Some(children) = index.inheritance.get(&current_id) {
            for &child_id in children {
                if !visited.contains(&(child_id, mode)) {
                    visited.insert((child_id, mode));
                    result_set.insert(child_id);
                    queue.push_back((child_id, mode));
                }
            }
        }

        for (parent_id, children) in &index.inheritance {
            if children.contains(&current_id) {
                if !visited.contains(&(*parent_id, mode)) {
                    visited.insert((*parent_id, mode));
                    result_set.insert(*parent_id);
                    queue.push_back((*parent_id, mode));
                }
            }
        }

        // 4. CONTAINMENT
        if let Some(sym) = index.symbols.get(&current_id) {
            if let Some(p_id) = sym.parent_id {
                if !visited.contains(&(p_id, mode)) {
                    visited.insert((p_id, mode));
                    result_set.insert(p_id);
                    queue.push_back((p_id, mode));
                }
            }
            // Exclude "module" to prevent irrelevant siblings in the file from being pulled in.
            if sym.kind == SymbolKind::Container {
                for (&s_id, s_node) in &index.symbols {
                    if s_node.parent_id == Some(current_id) {
                        if !visited.contains(&(s_id, mode)) {
                            visited.insert((s_id, mode));
                            result_set.insert(s_id);
                            queue.push_back((s_id, mode));
                        }
                    }
                }
            }
        }
    }

    let mut final_list: Vec<SymbolId> = result_set.into_iter().collect();
    final_list.sort_by(|a, b| {
        let sym_a = index.symbols.get(a).unwrap();
        let sym_b = index.symbols.get(b).unwrap();
        sym_a.file_id.cmp(&sym_b.file_id).then(sym_a.range_start.cmp(&sym_b.range_start))
    });

    Some(final_list)
}