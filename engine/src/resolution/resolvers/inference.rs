use crate::resolution::Indexer;
use crate::models::{SymbolId, SymbolKind};

impl Indexer {
    pub(crate) fn resolve_type_sniffing(&mut self) {
        let mut new_links = Vec::new();

        for (&caller_id, receiver_map) in &self.index.fingerprints {
            for (receiver_var_name, methods_called) in receiver_map {
                let lookup_name = receiver_var_name.trim_start_matches("this.").trim_start_matches("self.");

                // 1. Walk scope to find variable type
                let mut type_hint = None;
                let mut curr_scope = Some(caller_id);
                while let Some(sid) = curr_scope {
                    if let Some(vars) = self.index.local_variable_types.get(&sid) {
                        if let Some(h) = vars.get(lookup_name) {
                            type_hint = Some(h);
                            break;
                        }
                    }
                    curr_scope = self.index.symbols.get(&sid).and_then(|s| s.parent_id);
                }

                // 2. Resolve Type
                if let Some(hint) = type_hint {
                    let mut resolved_type_name = None;
                    if hint.starts_with("returns:") {
                         if let Some(targets) = self.index.symbol_map.get(&hint[8..]) {
                             resolved_type_name = self.index.symbols.get(&targets[0]).and_then(|s| s.return_type.clone());
                         }
                    } else {
                        resolved_type_name = Some(hint.clone());
                    }

                    if let Some(tn) = resolved_type_name {
                        let clean_type_name = tn.split('<').next().unwrap_or(&tn).trim();
                        if let Some(type_symbol_ids) = self.index.symbol_map.get(clean_type_name) {
                            for &type_id in type_symbol_ids {
                                new_links.push((caller_id, type_id)); // Link to Class
                                // Link to Methods
                                if let Some(known_methods) = self.index.container_methods.get(&type_id) {
                                    for method_called in methods_called {
                                        if method_called == "*" { continue; }
                                        if known_methods.contains(method_called) {
                                            if let Some(method_ids) = self.index.symbol_map.get(method_called) {
                                                new_links.extend(method_ids.iter()
                                                    .filter(|&&mid| self.index.symbols.get(&mid).map_or(false, |s| s.parent_id == Some(type_id)))
                                                    .map(|&mid| (caller_id, mid))
                                                );
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

        for (caller, target) in new_links {
            let calls = self.index.resolved_calls.entry(caller).or_default();
            if !calls.contains(&target) { calls.push(target); }
        }
    }

    pub(crate) fn resolve_fingerprints(&mut self) {
        let mut links = Vec::new();
        for (&cid, fprints) in &self.index.fingerprints {
            // Skip if already resolved by type sniffing
            let existing_calls = self.index.resolved_calls.get(&cid).cloned().unwrap_or_default();
            
            for (receiver_var, meths) in fprints {
                let already_resolved = existing_calls.iter().any(|&target_id| {
                    self.index.container_methods.get(&target_id).map_or(false, |cm| meths.iter().all(|m| cm.contains(m)))
                });
                if already_resolved { continue; }

                // Structural Candidates
                let mut candidates = Vec::new();
                for (&cont_id, cont_meths) in &self.index.container_methods {
                    if self.index.symbols[&cont_id].kind == SymbolKind::Module { continue; }
                    if meths.iter().all(|m| cont_meths.contains(m)) {
                        candidates.push(cont_id);
                    }
                }

                // Heuristic filtering by name
                let receiver_hint = receiver_var.split('.').last().unwrap_or(receiver_var).to_lowercase();
                let clean_hint = receiver_hint.trim_matches(|c| c == '_' || c == '$');
                
                let final_candidates = if clean_hint.len() > 1 {
                    let filtered: Vec<SymbolId> = candidates.iter().filter(|&&pid| {
                        let sym_name = self.index.symbols[&pid].name.to_lowercase();
                        sym_name.contains(clean_hint) || clean_hint.contains(&sym_name)
                    }).cloned().collect();
                    if !filtered.is_empty() { filtered } else { candidates }
                } else {
                    candidates
                };

                for cont_id in final_candidates {
                    links.push((cid, cont_id));
                    if let Some(_cont_meths) = self.index.container_methods.get(&cont_id) {
                         for m in meths {
                            if let Some(m_ids) = self.index.symbol_map.get(m) {
                                for &mid in m_ids {
                                    if self.index.symbols[&mid].parent_id == Some(cont_id) {
                                        links.push((cid, mid));
                                    }
                                }
                            }
                         }
                    }
                }
            }
        }
        for (c, t) in links {
            let entry = self.index.resolved_calls.entry(c).or_default();
            entry.push(t);
            entry.sort();
            entry.dedup();
        }
    }

    pub(crate) fn resolve_implicit_connections(&mut self) {
        self.index.inheritance.clear();
        for (cid, parents) in self.index.raw_implementations.clone() {
            for p in parents {
                if let Some(pids) = self.index.symbol_map.get(&p) {
                    for &pid in pids {
                        self.index.inheritance.entry(pid).or_default().push(cid);
                    }
                }
            }
        }
    }
}