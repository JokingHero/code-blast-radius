use crate::resolution::Indexer;
use crate::models::{SymbolKind, SymbolNode};

impl Indexer {
    pub(crate) fn resolve_external_imports(&mut self) {
        let mut new_symbols = Vec::new();

        for (_file_id, imports) in &self.index.file_imports {
            for imp in imports {
                if !imp.source.starts_with("./") && !imp.source.starts_with("../") && !imp.source.starts_with("/") {
                    let pkg_name = imp.source.clone();
                    let sym_name = imp.alias.clone().unwrap_or(imp.name.clone());

                    let stub_exists = self.index.symbol_map.get(&sym_name).map_or(false, |ids| {
                        ids.iter().any(|&id| {
                            let s = &self.index.symbols[&id];
                            s.is_external && s.external_source.as_deref() == Some(pkg_name.as_str())
                        })
                    });

                    if !stub_exists {
                        let new_id = self.index.next_symbol_id;
                        self.index.next_symbol_id += 1;

                        new_symbols.push(SymbolNode {
                            id: new_id,
                            file_id: 0,
                            parent_id: None,
                            name: sym_name.clone(),
                            kind: SymbolKind::External,
                            range_start: 0,
                            range_end: 0,
                            doc_comment: Some(format!("External import from package `{}`", pkg_name)),
                            return_type: None,
                            is_test: false,
                            is_external: true,
                            external_source: Some(pkg_name.clone()),
                            decorators: Vec::new(),
                            routes: Vec::new(),
                        });
                    }
                }
            }
        }

        for sym in new_symbols {
            self.index.symbol_map.entry(sym.name.clone()).or_default().push(sym.id);
            self.index.symbols.insert(sym.id, sym);
        }
    }

    pub(crate) fn resolve_function_calls(&mut self) {
        let entries: Vec<_> = self.index.raw_calls.iter().map(|(k, v)| (*k, v.clone())).collect();
        for (caller_id, called_names) in entries {
            let caller_file_id = self.index.symbols[&caller_id].file_id;
            for name in called_names {
                // Optimization: Skip if already resolved
                let already_resolved = self.index.resolved_calls.get(&caller_id)
                    .map_or(false, |r| r.iter().any(|&rid| self.index.symbols[&rid].name == name));
                
                if !already_resolved {
                    if let Some(tid) = self.resolve_single_call(caller_file_id, &name) {
                        self.index.resolved_calls.entry(caller_id).or_default().push(tid);
                    } else if let Some(candidates) = self.index.symbol_map.get(&name) {
                        // Fallback: Link to all candidates if we can't narrow it down
                        let mut guesses = candidates.clone();
                        self.index.resolved_calls.entry(caller_id).or_default().append(&mut guesses);
                    }
                }
            }
        }
        // Cleanup
        for calls in self.index.resolved_calls.values_mut() {
            calls.sort();
            calls.dedup();
        }
    }

    pub(crate) fn resolve_type_references(&mut self) {
        let entries: Vec<_> = self.index.raw_type_refs.iter().map(|(k, v)| (*k, v.clone())).collect();
        for (caller_id, type_names) in entries {
            let caller_file_id = self.index.symbols[&caller_id].file_id;
            for type_name in type_names {
                if let Some(target_id) = self.resolve_single_call(caller_file_id, &type_name) {
                    self.index.resolved_type_refs.entry(caller_id).or_default().push(target_id);
                } else if let Some(candidates) = self.index.symbol_map.get(&type_name) {
                    let mut guesses = candidates.clone();
                    self.index.resolved_type_refs.entry(caller_id).or_default().append(&mut guesses);
                }
            }
            if let Some(refs) = self.index.resolved_type_refs.get_mut(&caller_id) {
                refs.sort();
                refs.dedup();
            }
        }
    }
}