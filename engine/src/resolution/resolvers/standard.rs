use std::collections::HashMap;

use crate::models::{ EXTERNAL_FILE_ID, EdgeKind, StagingArea, SymbolIndex, SymbolKind, SymbolNode, WorkspaceIndex };
use crate::resolution::resolvers::{core, add_edge};

pub fn resolve_external_imports(index: &mut WorkspaceIndex, lookup: &mut SymbolIndex) {
    let mut new_symbols = Vec::new();
    
    for (_file_id, imports) in &lookup.file_imports {
        for imp in imports {
            if !imp.source.starts_with("./") &&
               !imp.source.starts_with("../") &&
               !imp.source.starts_with("/") 
            {
                let pkg_name = imp.source.clone();
                let sym_name = imp.alias.clone().unwrap_or(imp.name.clone());

                let stub_exists = lookup.symbol_map.get(&sym_name).map_or(false, |ids| {
                    ids.iter().any(|&id| {
                        let s = &index.symbols[&id];
                        s.is_external && s.external_source.as_deref() == Some(pkg_name.as_str())
                    })
                });

                if !stub_exists {
                    let new_id = index.next_symbol_id;
                    index.next_symbol_id += 1;

                    new_symbols.push(SymbolNode {
                        id: new_id,
                        file_id: EXTERNAL_FILE_ID,
                        parent_id: None,
                        name: sym_name.clone(),
                        kind: SymbolKind::External,
                        range_start: 0,
                        range_end: 0,
                        body_start: None,
                        doc_comment: Some(format!("External import from package `{}`", pkg_name)),
                        return_type: None,
                        is_test: false,
                        is_external: true,
                        external_source: Some(pkg_name.clone()),
                        decorators: Vec::new(),
                        routes: Vec::new(),
                        calls: Vec::new(),
                        type_refs: Vec::new(),
                        fingerprints: HashMap::new(),
                        local_types: HashMap::new(),
                        config_keys: Vec::new(),
                        dispatched_actions: Vec::new(),
                        handled_actions: Vec::new(),
                    });
                }
            }
        }
    }

    for sym in new_symbols {
        lookup.symbol_map.entry(sym.name.clone()).or_default().push(sym.id);
        index.symbols.insert(sym.id, sym);
    }
}

pub fn resolve_function_calls(
    index: &mut WorkspaceIndex,
    staging: &StagingArea,
    lookup: &SymbolIndex,
    cache: &mut core::ResolutionCache
) {
    // No cloning needed!
    for (caller_id, called_names) in &staging.raw_calls {
        let caller_file_id = index.symbols
            .get(caller_id)
            .map(|s| s.file_id)
            .unwrap_or(0);

        for name in called_names {
            // 1. Try Resolve Single Call
            let resolved_id = core::resolve_single_call(index, lookup, cache, caller_file_id, name);

            if let Some(target_id) = resolved_id {
                add_edge(index, *caller_id, target_id, EdgeKind::Calls);
            } else {
                // 2. Fallback: Symbol Map Lookup
                if let Some(candidates) = lookup.symbol_map.get(name) {
                    for &candidate_id in candidates {
                        add_edge(index, *caller_id, candidate_id, EdgeKind::Calls);
                    }
                }
            }
        }
    }
}

pub fn resolve_type_references(
    index: &mut WorkspaceIndex,
    staging: &StagingArea,
    lookup: &SymbolIndex,
    cache: &mut core::ResolutionCache
) {
    for (caller_id, type_names) in &staging.raw_type_refs {
        let caller_file_id = index.symbols
            .get(caller_id)
            .map(|s| s.file_id)
            .unwrap_or(0);

        for type_name in type_names {
            let resolved_id = core::resolve_single_call(index, lookup, cache, caller_file_id, type_name);

            if let Some(target_id) = resolved_id {
                add_edge(index, *caller_id, target_id, EdgeKind::TypeReference);
            } else {
                if let Some(candidates) = lookup.symbol_map.get(type_name) {
                    for &candidate_id in candidates {
                        add_edge(index, *caller_id, candidate_id, EdgeKind::TypeReference);
                    }
                }
            }
        }
    }
}