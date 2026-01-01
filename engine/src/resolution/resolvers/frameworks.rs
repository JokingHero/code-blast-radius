use std::collections::HashMap;
use std::path::Path;
use crate::resolution::Indexer;
use crate::analysis::language::LanguageConfig;
use crate::models::{ FileId, SymbolId, SymbolKind, EdgeKind };

impl Indexer {
    pub(crate) fn resolve_implicit_routes(&mut self) {
        let mut new_links = Vec::new();
        let route_definitions: Vec<(String, SymbolId)> = self.lookup.implicit_routes
            .iter()
            .map(|(r, s)| (r.clone(), *s))
            .collect();
        // Uses staging.raw_literals
        for (src_file_id, literals) in &self.staging.raw_literals {
            for lit in literals {
                let clean_lit = lit.trim_matches(|c| c == '"' || c == '\'' || c == '`');

                if let Some(&target_sym_id) = self.lookup.implicit_routes.get(clean_lit) {
                    if self.index.symbols[&target_sym_id].file_id != *src_file_id {
                        new_links.push((*src_file_id, target_sym_id));
                    }
                    continue;
                }

                if clean_lit.contains('*') {
                    let prefix = clean_lit.split('*').next().unwrap_or("");
                    if prefix.len() > 3 {
                        for (route_def, target_sym_id) in &route_definitions {
                            if self.index.symbols[target_sym_id].file_id == *src_file_id {
                                continue;
                            }
                            if route_def.starts_with(prefix) {
                                new_links.push((*src_file_id, *target_sym_id));
                            }
                        }
                    }
                }
            }
        }

        for (src_file_id, tgt_sym_id) in new_links {
            let tgt_file_id = self.index.symbols[&tgt_sym_id].file_id;
            
            // FIX: Restore File Dependency (Frontend -> Backend)
            let deps = self.index.file_dependencies.entry(src_file_id).or_default();
            if !deps.contains(&tgt_file_id) { deps.push(tgt_file_id); }

            let src_mod = self.index.symbols
                .values()
                .find(|s| s.file_id == src_file_id && s.kind == SymbolKind::Module)
                .map(|s| s.id);

            if let Some(s) = src_mod {
                self.add_edge(s, tgt_sym_id, EdgeKind::Calls); 
            }
            
            self.link_modules(src_file_id, tgt_file_id);
        }
    }

    pub(crate) fn resolve_dependency_injection(&mut self) {
        let mut file_configs: HashMap<FileId, &LanguageConfig> = HashMap::new();
        for file in self.index.files.values() {
            let path = Path::new(&file.path);
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            let filename = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            let config = self.configs
                .get(ext)
                .or_else(|| self.configs.get(filename))
                .or_else(|| if filename.starts_with('.') {
                        self.configs.get(&filename[1..])
                    } else {
                        None
                    } );
            if let Some(c) = config {
                file_configs.insert(file.id, c);
            }
        }

        let mut interface_to_providers: HashMap<SymbolId, Vec<SymbolId>> = HashMap::new();
        let mut name_to_providers: HashMap<String, Vec<SymbolId>> = HashMap::new();

        for sym in self.index.symbols.values() {
            if sym.decorators.is_empty() {
                continue;
            }
            if let Some(config) = file_configs.get(&sym.file_id) {
                let is_provider = sym.decorators.iter().any(|d| {
                    let clean = d.trim_start_matches('@').split('(').next().unwrap_or("").trim();
                    config.di_decorators.contains(&clean)
                });
                if is_provider {
                    name_to_providers.entry(sym.name.clone()).or_default().push(sym.id);

                    if let Some(edges) = self.index.graph.get(&sym.id) {
                        for edge in edges {
                            if edge.kind == EdgeKind::Inherits || edge.kind == EdgeKind::Implements {
                                interface_to_providers
                                    .entry(edge.target_id)
                                    .or_default()
                                    .push(sym.id);
                            }
                        }
                    }
                }
            }
        }

        let mut new_links = Vec::new();
        let mut injection_points = Vec::new();
        // Uses staging.local_variable_types
        for (scope_id, var_types) in &self.staging.local_variable_types {
            if let Some(scope_sym) = self.index.symbols.get(scope_id) {
                let link_source_id = if
                    scope_sym.name == "constructor" ||
                    scope_sym.name == "__init__"
                {
                    scope_sym.parent_id.unwrap_or(*scope_id)
                } else {
                    *scope_id
                };

                for type_name in var_types.values() {
                    let clean = type_name.split('<').next().unwrap_or(type_name).trim().to_string();
                    injection_points.push((link_source_id, scope_sym.file_id, clean));
                }
            }
        }

        for (src_id, file_id, type_name) in injection_points {
            let type_sym_id_opt = self.resolve_single_call(file_id, &type_name);

            if let Some(type_id) = type_sym_id_opt {
                if let Some(providers) = interface_to_providers.get(&type_id) {
                    for &p_id in providers {
                        new_links.push((src_id, p_id));
                    }
                } else if
                    let Some(providers) = name_to_providers.get(&type_name)
                {
                    if providers.contains(&type_id) {
                        new_links.push((src_id, type_id));
                    }
                }
            } else if
                let Some(providers) = name_to_providers.get(&type_name)
            {
                for &p_id in providers {
                    new_links.push((src_id, p_id));
                }
            }
        }

        for (src, target) in new_links {
            if src != target {
                self.add_edge(src, target, EdgeKind::Injects);
            }
        }
    }

    pub(crate) fn resolve_middleware_injection(&mut self) {
        let mut new_links = Vec::new();
        // Uses staging.raw_middleware_usage
        let usage_snapshot: Vec<(FileId, Vec<String>)> = self.staging.raw_middleware_usage
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();

        for (hub_file_id, middleware_names) in usage_snapshot {
            let mut middleware_ids = Vec::new();
            for name in &middleware_names {
                if let Some(sid) = self.resolve_single_call(hub_file_id, name) {
                    middleware_ids.push(sid);
                }
            }
            if middleware_ids.is_empty() {
                continue;
            }

            let imports = self.lookup.file_imports.get(&hub_file_id).cloned().unwrap_or_default();
            for imp in imports {
                if let Some(imported_file_id) = self.resolve_import_path(hub_file_id, &imp.source) {
                    if
                        middleware_ids
                            .iter()
                            .any(|&mid| self.index.symbols[&mid].file_id == imported_file_id)
                    {
                        continue;
                    }

                    let target_mod_id = self.index.symbols
                        .values()
                        .find(|s| s.file_id == imported_file_id && s.kind == SymbolKind::Module)
                        .map(|s| s.id);

                    if let Some(target_id) = target_mod_id {
                        for &mid in &middleware_ids {
                            new_links.push((target_id, mid));
                        }
                    }
                }
            }
        }

        for (router_id, middleware_id) in new_links {
            self.add_edge(router_id, middleware_id, EdgeKind::Injects); 
        }
    }
}