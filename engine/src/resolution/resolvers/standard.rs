use crate::resolution::Indexer;
use crate::models::{ SymbolKind, SymbolNode, EdgeKind, SymbolId };
impl Indexer {
    pub(crate) fn resolve_external_imports(&mut self) {
        let mut new_symbols = Vec::new();
        // Safe: We iterate lookup.file_imports (immutable), collect data into new_symbols,
        // and only mutate self AFTER the loop.
        for (_file_id, imports) in &self.index.lookup.file_imports {
            for imp in imports {
                if
                    !imp.source.starts_with("./") &&
                    !imp.source.starts_with("../") &&
                    !imp.source.starts_with("/")
                {
                    let pkg_name = imp.source.clone();
                    let sym_name = imp.alias.clone().unwrap_or(imp.name.clone());

                    let stub_exists = self.index.lookup.symbol_map.get(&sym_name).map_or(false, |ids| {
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
                            doc_comment: Some(
                                format!("External import from package `{}`", pkg_name)
                            ),
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
            self.index.lookup.symbol_map.entry(sym.name.clone()).or_default().push(sym.id);
            self.index.symbols.insert(sym.id, sym);
        }
    }

    pub(crate) fn resolve_function_calls(&mut self) {
        // Snapshot staging.raw_calls to avoid holding borrow on self.index
        let entries: Vec<_> = self.index.staging.raw_calls
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();

        for (caller_id, called_names) in entries {
            let caller_file_id = self.index.symbols
                .get(&caller_id)
                .map(|s| s.file_id)
                .unwrap_or(0);

            for name in called_names {
                // 1. Try Resolve Single Call (Mutates cache, so requires &mut self)
                let resolved_id = self.resolve_single_call(caller_file_id, &name);

                if let Some(tid) = resolved_id {
                    self.add_edge(caller_id, tid, EdgeKind::Calls);
                } else {
                    // 2. Fallback: Symbol Map Lookup
                    // FIX: Clone candidates to vector.
                    // This drops the borrow on self.index.lookup.symbol_map immediately.
                    let candidates: Vec<SymbolId> = self.index.lookup.symbol_map
                        .get(&name)
                        .map(|v| v.clone())
                        .unwrap_or_default();

                    // Now we can mutate self safely
                    for cid in candidates {
                        self.add_edge(caller_id, cid, EdgeKind::Calls);
                    }
                }
            }
        }
    }

    pub(crate) fn resolve_type_references(&mut self) {
        // Snapshot staging.raw_type_refs
        let entries: Vec<_> = self.index.staging.raw_type_refs
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();

        for (caller_id, type_names) in entries {
            let caller_file_id = self.index.symbols
                .get(&caller_id)
                .map(|s| s.file_id)
                .unwrap_or(0);

            for type_name in type_names {
                // 1. Try Resolve (Mutates cache)
                let resolved_id = self.resolve_single_call(caller_file_id, &type_name);

                if let Some(target_id) = resolved_id {
                    self.add_edge(caller_id, target_id, EdgeKind::TypeReference);
                } else {
                    // 2. Fallback
                    // FIX: Clone candidates to drop borrow
                    let candidates: Vec<SymbolId> = self.index.lookup.symbol_map
                        .get(&type_name)
                        .map(|v| v.clone())
                        .unwrap_or_default();

                    for cid in candidates {
                        self.add_edge(caller_id, cid, EdgeKind::TypeReference);
                    }
                }
            }
        }
    }
}