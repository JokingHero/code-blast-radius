use std::collections::HashMap;
use std::path::Path;
use crate::models::{ FileId, SymbolId, SymbolKind, EdgeKind, WorkspaceIndex, StagingArea, SymbolIndex };
use crate::analysis::language::LanguageConfig;
use crate::resolution::resolvers::{core, add_edge, link_modules, constants};

pub fn resolve_implicit_routes(index: &mut WorkspaceIndex, staging: &StagingArea, lookup: &SymbolIndex) {
    let mut new_links = Vec::new();
    
    let route_definitions: Vec<(String, SymbolId)> = lookup.implicit_routes
        .iter()
        .flat_map(|(route, ids)| ids.iter().map(move |&id| (route.clone(), id)))
        .collect();
        
    for (src_file_id, literals) in &staging.raw_literals {
        for lit in literals {
            let clean_lit = lit.trim_matches(constants::QUOTE_CHARS);

            // Handle exact matches (One-to-Many)
            if let Some(target_sym_ids) = lookup.implicit_routes.get(clean_lit) {
                for &target_sym_id in target_sym_ids {
                    if index.symbols[&target_sym_id].file_id != *src_file_id {
                        new_links.push((*src_file_id, target_sym_id));
                    }
                }
                // Don't continue here, as wildcards might *also* apply, 
                // though usually exact match is preferred. 
                // For simplicity, we can continue if exact match found to reduce noise.
                continue;
            }

            // Handle Wildcard Prefix matches
            if clean_lit.contains('*') {
                let prefix = clean_lit.split('*').next().unwrap_or("");
                if prefix.len() > 3 {
                    for (route_def, target_sym_id) in &route_definitions {
                        if index.symbols[target_sym_id].file_id == *src_file_id {
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

    // Edge creation logic remains the same, just processing more links now
    for (src_file_id, tgt_sym_id) in new_links {
        let tgt_file_id = index.symbols[&tgt_sym_id].file_id;
        
        let deps = index.file_dependencies.entry(src_file_id).or_default();
        if !deps.contains(&tgt_file_id) { deps.push(tgt_file_id); }

        let src_mod = index.symbols
            .values()
            .find(|s| s.file_id == src_file_id && s.kind == SymbolKind::Module)
            .map(|s| s.id);

        if let Some(s) = src_mod {
            add_edge(index, s, tgt_sym_id, EdgeKind::Calls); 
        }
        
        link_modules(index, lookup, src_file_id, tgt_file_id);
    }
}

pub fn resolve_dependency_injection(
    index: &mut WorkspaceIndex,
    staging: &StagingArea,
    lookup: &SymbolIndex,
    cache: &mut core::ResolutionCache,
    // Updated signature to remove &'static
    configs: &HashMap<String, LanguageConfig>
) {
    let mut file_configs: HashMap<FileId, &LanguageConfig> = HashMap::new();
    for file in index.files.values() {
        let path = Path::new(&file.path);
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        
        // This lookup logic remains the same, but now returns &LanguageConfig from the HashMap
        let config = configs.get(ext)
            .or_else(|| configs.get(filename))
            .or_else(|| if filename.starts_with('.') { configs.get(&filename[1..]) } else { None });
            
        if let Some(c) = config {
            file_configs.insert(file.id, c);
        }
    }

    let mut interface_to_providers: HashMap<SymbolId, Vec<SymbolId>> = HashMap::new();
    let mut name_to_providers: HashMap<String, Vec<SymbolId>> = HashMap::new();

    for sym in index.symbols.values() {
        if sym.decorators.is_empty() { continue; }
        if let Some(config) = file_configs.get(&sym.file_id) {
            let is_provider = sym.decorators.iter().any(|d| {
                let clean = d.trim_start_matches('@').split('(').next().unwrap_or("").trim();
                // Access via .heuristics
                config.heuristics.di_decorators.contains(&clean)
            });
            if is_provider {
                name_to_providers.entry(sym.name.clone()).or_default().push(sym.id);

                if let Some(edges) = index.graph.get(&sym.id) {
                    for edge in edges {
                        if edge.kind == EdgeKind::Inherits || edge.kind == EdgeKind::Implements {
                            interface_to_providers.entry(edge.target_id).or_default().push(sym.id);
                        }
                    }
                }
            }
        }
    }

    let mut new_links = Vec::new();
    let mut injection_points = Vec::new();
    for (scope_id, var_types) in &staging.local_variable_types {
        if let Some(scope_sym) = index.symbols.get(scope_id) {
            let config = file_configs.get(&scope_sym.file_id);
            let is_constructor = config.map_or(false, |c| c.heuristics.constructor_names.contains(&scope_sym.name.as_str()));
            
            let link_source_id = if is_constructor {
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
        let type_sym_id_opt = core::resolve_single_call(index, lookup, cache, file_id, &type_name);

        if let Some(type_id) = type_sym_id_opt {
            if let Some(providers) = interface_to_providers.get(&type_id) {
                for &p_id in providers { new_links.push((src_id, p_id)); }
            } else if let Some(providers) = name_to_providers.get(&type_name) {
                if providers.contains(&type_id) { new_links.push((src_id, type_id)); }
            }
        } else if let Some(providers) = name_to_providers.get(&type_name) {
            for &p_id in providers { new_links.push((src_id, p_id)); }
        }
    }

    for (src, target) in new_links {
        if src != target { add_edge(index, src, target, EdgeKind::Injects); }
    }
}

pub fn resolve_middleware_injection(
    index: &mut WorkspaceIndex,
    staging: &StagingArea,
    lookup: &SymbolIndex,
    cache: &mut core::ResolutionCache
) {
    let mut new_links = Vec::new();

    for (hub_file_id, middleware_names) in &staging.raw_middleware_usage {
        let mut middleware_ids = Vec::new();
        for name in middleware_names {
            if let Some(symbol_id) = core::resolve_single_call(index, lookup, cache, *hub_file_id, name) {
                middleware_ids.push(symbol_id);
            }
        }
        if middleware_ids.is_empty() { continue; }

        let imports = lookup.file_imports.get(hub_file_id).cloned().unwrap_or_default();
        for imp in imports {
            if let Some(imported_file_id) = core::resolve_import_path(index, lookup, *hub_file_id, &imp.source) {
                if middleware_ids.iter().any(|&middleware_id| index.symbols[&middleware_id].file_id == imported_file_id) {
                    continue;
                }

                let target_mod_id = index.symbols
                    .values()
                    .find(|s| s.file_id == imported_file_id && s.kind == SymbolKind::Module)
                    .map(|s| s.id);

                if let Some(target_id) = target_mod_id {
                    for &middleware_id in &middleware_ids { new_links.push((target_id, middleware_id)); }
                }
            }
        }
    }

    for (router_id, middleware_id) in new_links {
        add_edge(index, router_id, middleware_id, EdgeKind::Injects); 
    }
}