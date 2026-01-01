use crate::models::{ SymbolKind, EdgeKind, WorkspaceIndex, StagingArea, SymbolIndex };
use crate::resolution::resolvers::{ add_edge };

pub fn resolve_type_sniffing(index: &mut WorkspaceIndex, staging: &StagingArea, lookup: &SymbolIndex) {
    let mut new_links = Vec::new();
    
    for (&caller_id, receiver_map) in &staging.fingerprints {
        for (receiver_var_name, methods_called) in receiver_map {
            let lookup_name = receiver_var_name.trim_start_matches("this.").trim_start_matches("self.");

            // 1. Walk scope to find variable type
            let mut type_hint = None;
            let mut curr_scope = Some(caller_id);
            while let Some(sid) = curr_scope {
                if let Some(vars) = staging.local_variable_types.get(&sid) {
                    if let Some(h) = vars.get(lookup_name) {
                        type_hint = Some(h);
                        break;
                    }
                }
                curr_scope = index.symbols.get(&sid).and_then(|s| s.parent_id);
            }

            // 2. Resolve Type
            if let Some(hint) = type_hint {
                let mut resolved_type_name = None;
                if hint.starts_with("returns:") {
                    if let Some(targets) = lookup.symbol_map.get(&hint[8..]) {
                        resolved_type_name = index.symbols.get(&targets[0]).and_then(|s| s.return_type.clone());
                    }
                } else {
                    resolved_type_name = Some(hint.clone());
                }

                if let Some(tn) = resolved_type_name {
                    let clean_type_name = tn.split('<').next().unwrap_or(&tn).trim();
                    if let Some(type_symbol_ids) = lookup.symbol_map.get(clean_type_name) {
                        for &type_id in type_symbol_ids {
                            new_links.push((caller_id, type_id, EdgeKind::TypeReference));
                            // Link to Methods
                            if let Some(known_methods) = staging.container_methods.get(&type_id) {
                                for method_called in methods_called {
                                    if method_called == "*" { continue; }
                                    if known_methods.contains(method_called) {
                                        if let Some(method_ids) = lookup.symbol_map.get(method_called) {
                                            for &mid in method_ids {
                                                if index.symbols.get(&mid).map_or(false, |s| s.parent_id == Some(type_id)) {
                                                    new_links.push((caller_id, mid, EdgeKind::Calls));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    for (src, tgt, kind) in new_links {
        add_edge(index, src, tgt, kind);
    }
}

pub fn resolve_fingerprints(index: &mut WorkspaceIndex, staging: &StagingArea, lookup: &SymbolIndex) {
    let mut links = Vec::new();
    
    for (&cid, fprints) in &staging.fingerprints {
        for (receiver_var, meths) in fprints {
            let lookup_name = receiver_var.trim_start_matches("this.").trim_start_matches("self.");

            let mut has_type_hint = false;
            let mut curr_scope = Some(cid);
            while let Some(sid) = curr_scope {
                if let Some(vars) = staging.local_variable_types.get(&sid) {
                    if vars.contains_key(lookup_name) {
                        has_type_hint = true;
                        break;
                    }
                }
                curr_scope = index.symbols.get(&sid).and_then(|s| s.parent_id);
            }

            if has_type_hint { continue; }

            // Structural Candidates (Fuzzy Fallback)
            let mut candidates = Vec::new();
            for (&cont_id, cont_meths) in &staging.container_methods {
                if index.symbols[&cont_id].kind == SymbolKind::Module { continue; }
                if meths.iter().all(|m| cont_meths.contains(m)) {
                    candidates.push(cont_id);
                }
            }

            // Heuristic filtering by name
            let receiver_hint = receiver_var.split('.').last().unwrap_or(receiver_var).to_lowercase();
            let clean_hint = receiver_hint.trim_matches(|c| c == '_' || c == '$');

            let final_candidates = if clean_hint.len() > 1 {
                let filtered: Vec<_> = candidates.iter().filter(|&&pid| {
                    let sym_name = index.symbols[&pid].name.to_lowercase();
                    sym_name.contains(clean_hint) || clean_hint.contains(&sym_name)
                }).cloned().collect();
                if !filtered.is_empty() { filtered } else { candidates }
            } else {
                candidates
            };

            for cont_id in final_candidates {
                links.push((cid, cont_id, EdgeKind::TypeReference)); 
                if let Some(_cont_meths) = staging.container_methods.get(&cont_id) {
                    for m in meths {
                        if let Some(m_ids) = lookup.symbol_map.get(m) {
                            for &mid in m_ids {
                                if index.symbols[&mid].parent_id == Some(cont_id) {
                                    links.push((cid, mid, EdgeKind::Calls));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    for (src, tgt, kind) in links {
        add_edge(index, src, tgt, kind);
    }
}

pub fn resolve_implicit_connections(index: &mut WorkspaceIndex, staging: &StagingArea, lookup: &SymbolIndex) {
    let mut new_edges = Vec::new();

    for (cid, parents) in &staging.raw_implementations {
        for p in parents {
            if let Some(pids) = lookup.symbol_map.get(p) {
                for &pid in pids {
                    new_edges.push((*cid, pid));
                }
            }
        }
    }

    for (cid, pid) in new_edges {
        add_edge(index, cid, pid, EdgeKind::Inherits);
    }
}