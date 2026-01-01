use std::path::Path;
use std::fs;
use std::collections::{HashMap, HashSet};
use ignore::WalkBuilder;
use blake3;

use crate::manifest::scan_manifests;
use crate::models::{
    FileNode, FileId, SymbolNode, SymbolKind, SymbolId,
    WorkspaceIndex, StagingArea, SymbolIndex
};
use crate::analysis::analyze_source;
use crate::analysis::language::{get_language_configs, LanguageConfig};
use crate::resolution::utils;

pub struct FileScanner {
    // FIX: This map stores references, matching the static definitions
    pub configs: HashMap<String, &'static LanguageConfig>,
}

impl FileScanner {
    pub fn new() -> Self {
        let mut config_map = HashMap::new();
        for config in get_language_configs() {
            for ext in config.file_extensions {
                // FIX: Insert the reference directly (config), do NOT dereference (*config)
                config_map.insert(ext.to_string(), config);
            }
        }
        Self {
            configs: config_map,
        }
    }

    pub fn scan(
        &self,
        root: &Path,
        index: &mut WorkspaceIndex,
        staging: &mut StagingArea,
        lookup: &mut SymbolIndex
    ) {
        let root_abs = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        let root_string = root_abs.to_string_lossy().to_string();
        
        if !index.roots.contains(&root_string) {
            index.roots.push(root_string);
        }

        let mut seen_paths = HashSet::new();
        let walker = WalkBuilder::new(&root_abs).hidden(false).git_ignore(true).build();

        for result in walker {
            match result {
                Ok(entry) => {
                    if !entry.path().is_file() { continue; }
                    let path = entry.path();
                    
                    // 1. Manifest Scanning
                    let manifest_res = scan_manifests(path);
                    if let Some(pkg_name) = manifest_res.package_name {
                        if let Some(parent_dir) = path.parent() {
                            let dir_key = utils::to_index_path(parent_dir);
                            lookup.package_path_map.insert(pkg_name, dir_key);
                        }
                    }
                    if !manifest_res.externals.is_empty() { lookup.external_packages.extend(manifest_res.externals); }
                    if !manifest_res.aliases.is_empty() { lookup.import_mappings.extend(manifest_res.aliases); }

                    // 2. Language Detection
                    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
                    let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    
                    // Note: .cloned() creates a copy of the reference (&'static T), which is cheap
                    let config = self.configs.get(ext)
                        .or_else(|| self.configs.get(filename))
                        .or_else(|| if filename.starts_with('.') { self.configs.get(&filename[1..]) } else { None })
                        .cloned();

                    // 3. File Processing
                    if let Some(config) = config {
                        if let Ok(content) = fs::read_to_string(path) {
                            let hash = blake3::hash(content.as_bytes());
                            let hash_bytes: [u8; 32] = hash.into();
                            let path_key = utils::to_index_path(path);
                            seen_paths.insert(path_key.clone());

                            let is_test = match path.strip_prefix(&root_abs) {
                                Ok(rel) => utils::is_test_path(rel),
                                Err(_) => {
                                    let fname = path.file_name().map(Path::new).unwrap_or(path);
                                    utils::is_test_path(fname)
                                }
                            };

                            let needs_update = match index.files.get(&path_key) {
                                Some(node) => node.hash != hash_bytes,
                                None => true,
                            };
                            
                            if needs_update {
                                self.update_file(
                                    &path_key, path, &content, hash_bytes, config, is_test, 
                                    index, staging, lookup
                                );
                            }
                        }
                    }
                }
                Err(err) => eprintln!("Error walking directory: {}", err),
            }
        }

        // 4. Cleanup Removed Files
        let to_remove: Vec<String> = index.files.keys()
            .filter(|path_key| !seen_paths.contains(*path_key))
            .cloned()
            .collect();
            
        for path_key in to_remove { 
            self.remove_file(&path_key, index, staging, lookup); 
        }
    }

    fn remove_file(
        &self, 
        path_key: &str, 
        index: &mut WorkspaceIndex, 
        staging: &mut StagingArea, 
        lookup: &mut SymbolIndex
    ) {
        if let Some(node) = index.files.remove(path_key) {
            self.clear_file_symbols(node.id, index, staging, lookup);
            index.file_dependencies.remove(&node.id);
            lookup.implicit_routes.retain(|_, sym_id| {
                index.symbols.get(sym_id).map_or(false, |s| s.file_id != node.id)
            });
        }
    }

    fn clear_file_symbols(
        &self, 
        file_id: FileId, 
        index: &mut WorkspaceIndex, 
        staging: &mut StagingArea, 
        lookup: &mut SymbolIndex
    ) {
        let ids_to_remove: Vec<SymbolId> = index.symbols.values()
            .filter(|s| s.file_id == file_id).map(|s| s.id).collect();

        for &sym_id in &ids_to_remove {
            if let Some(sym) = index.symbols.remove(&sym_id) {
                if let Some(id_list) = lookup.symbol_map.get_mut(&sym.name) {
                    id_list.retain(|&id| id != sym_id);
                    if id_list.is_empty() { lookup.symbol_map.remove(&sym.name); }
                }
            }
            
            // Clear Graph
            index.graph.remove(&sym_id);
            
            // Clear Staging
            staging.raw_calls.remove(&sym_id);
            staging.raw_implementations.remove(&sym_id);
            staging.fingerprints.remove(&sym_id);
            staging.container_methods.remove(&sym_id);
            staging.local_variable_types.remove(&sym_id);
            staging.symbol_config_refs.remove(&sym_id);
            staging.raw_type_refs.remove(&sym_id);
            staging.raw_decorators.remove(&sym_id);
            staging.raw_action_dispatches.remove(&sym_id);
            staging.raw_action_handlers.remove(&sym_id);
        }

        // Clear Lookups
        for def_list in lookup.config_definitions.values_mut() {
            def_list.retain(|id| !ids_to_remove.contains(id));
        }
        lookup.config_definitions.retain(|_, v| !v.is_empty());
        lookup.file_imports.remove(&file_id);
        lookup.file_exports.remove(&file_id);
        
        // Clear File-Level Staging
        staging.raw_literals.remove(&file_id);
        staging.raw_middleware_usage.remove(&file_id);
    }

    #[allow(clippy::too_many_arguments)]
    fn update_file(
        &self,
        path_key: &str,
        path_obj: &Path,
        content: &str,
        hash: [u8; 32],
        config: &LanguageConfig,
        is_path_test: bool,
        index: &mut WorkspaceIndex,
        staging: &mut StagingArea,
        lookup: &mut SymbolIndex
    ) {
         let file_id = match index.files.get(path_key) {
            Some(node) => node.id,
            None => { let id = index.next_file_id; index.next_file_id += 1; id }
        };

        self.clear_file_symbols(file_id, index, staging, lookup);
        index.files.insert(path_key.to_string(), FileNode {
            id: file_id, path: path_key.to_string(), hash, is_test: is_path_test,
        });

        if let Ok(analysis) = analyze_source(path_obj, content, config) {
            // Populate Lookups
            if !analysis.imports.is_empty() { lookup.file_imports.insert(file_id, analysis.imports); }
            if !analysis.exports.is_empty() { lookup.file_exports.insert(file_id, analysis.exports); }
            
            // Populate Staging
            if !analysis.literals.is_empty() { staging.raw_literals.insert(file_id, analysis.literals); }
            if !analysis.middleware_usage.is_empty() { staging.raw_middleware_usage.insert(file_id, analysis.middleware_usage); }

            let mut file_symbol_ids = Vec::new();
            for func in analysis.functions {
                let symbol_id = index.next_symbol_id;
                index.next_symbol_id += 1;
                
                let is_inline_test = func.kind != SymbolKind::Module && (
                    func.source_code.contains("it(") || func.name.contains("test") || func.decorators.iter().any(|d| d.contains("test"))
                );

                // Add to Graph (Symbols)
                index.symbols.insert(symbol_id, SymbolNode {
                    id: symbol_id, file_id, parent_id: None,
                    name: func.name.clone(), kind: func.kind,
                    range_start: func.range_start, range_end: func.range_end,
                    doc_comment: func.documentation, return_type: func.return_type,
                    is_test: is_path_test || is_inline_test,
                    is_external: false, external_source: None,
                    decorators: func.decorators.clone(), routes: func.routes.clone(),
                });
                
                file_symbol_ids.push(symbol_id);
                if func.name != "anonymous" { lookup.symbol_map.entry(func.name.clone()).or_default().push(symbol_id); }
                
                // Add to Staging
                if !func.config_keys.is_empty() { staging.symbol_config_refs.insert(symbol_id, func.config_keys); }
                if !func.calls.is_empty() { staging.raw_calls.insert(symbol_id, func.calls); }
                if !func.fingerprints.is_empty() { staging.fingerprints.insert(symbol_id, func.fingerprints); }
                if !func.local_types.is_empty() { staging.local_variable_types.insert(symbol_id, func.local_types); }
                if !func.type_refs.is_empty() { staging.raw_type_refs.insert(symbol_id, func.type_refs); }
                if !func.decorators.is_empty() { staging.raw_decorators.insert(symbol_id, func.decorators); }
                if !func.dispatched_actions.is_empty() { staging.raw_action_dispatches.insert(symbol_id, func.dispatched_actions); }
                if !func.handled_actions.is_empty() { staging.raw_action_handlers.insert(symbol_id, func.handled_actions); }
            }

            // Handle Config Definitions (Lookup)
            let is_data = matches!(config.lang_enum, crate::analysis::language::SupportedLanguage::Yaml | crate::analysis::language::SupportedLanguage::Json | crate::analysis::language::SupportedLanguage::Toml | crate::analysis::language::SupportedLanguage::Dotenv);
            if is_data {
                for &sid in &file_symbol_ids {
                    let name = &index.symbols[&sid].name;
                    lookup.config_definitions.entry(name.clone()).or_default().push(sid);
                }
            }

            // Handle Containers (Graph + Staging)
            let container_ids: Vec<SymbolId> = file_symbol_ids.iter().filter(|&&id| {
                let s = &index.symbols[&id]; 
                s.kind == SymbolKind::Container || s.kind == SymbolKind::Module
            }).cloned().collect();

            for c_id in container_ids {
                let (cs, ce, c_kind) = { let c = &index.symbols[&c_id]; (c.range_start, c.range_end, c.kind) };
                let mut members = HashSet::new();
                for &s_id in &file_symbol_ids {
                    if s_id == c_id { continue; }
                    let is_member = { let s = &index.symbols[&s_id]; s.range_start >= cs && s.range_end <= ce };
                    if is_member {
                        members.insert(index.symbols[&s_id].name.clone());
                        if c_kind != SymbolKind::Module { 
                            if let Some(node) = index.symbols.get_mut(&s_id) { 
                                node.parent_id = Some(c_id); 
                            } 
                            Self::add_edge_internal(index, c_id, s_id, crate::models::EdgeKind::Contains);
                        }
                    }
                }
                if !members.is_empty() { staging.container_methods.insert(c_id, members); }
            }
            
            // Handle Module Scope (Graph)
            let module_id = file_symbol_ids.iter().find(|&&id| index.symbols[&id].kind == SymbolKind::Module).cloned();
            if let Some(mid) = module_id {
                for &id in &file_symbol_ids {
                    if id != mid {
                         if let Some(sym) = index.symbols.get_mut(&id) { 
                             if sym.parent_id.is_none() { 
                                 sym.parent_id = Some(mid); 
                                 Self::add_edge_internal(index, mid, id, crate::models::EdgeKind::Contains); 
                             } 
                         }
                    }
                }
            }

            // Handle Implementations (Staging)
            for (child, parent) in analysis.implementations {
                if let Some(ids) = lookup.symbol_map.get(&child) {
                    if let Some(&cid) = ids.iter().find(|&&id| index.symbols[&id].file_id == file_id) {
                        staging.raw_implementations.entry(cid).or_default().push(parent);
                    }
                }
            }

            // Handle Routes (Lookup)
            if let Some(route) = utils::detect_framework_route(path_obj) {
                 if let Some(mid) = file_symbol_ids.iter().find(|&&id| index.symbols[&id].kind == SymbolKind::Module) {
                     lookup.implicit_routes.insert(route, *mid);
                 }
            }
            for &sid in &file_symbol_ids {
                 if let Some(sym) = index.symbols.get(&sid) {
                     for r in &sym.routes { lookup.implicit_routes.insert(r.clone(), sid); }
                 }
            }
        }
    }
    
    fn add_edge_internal(index: &mut WorkspaceIndex, source: SymbolId, target: SymbolId, kind: crate::models::EdgeKind) {
        if source == target { return; }
        let edges = index.graph.entry(source).or_default();
        for edge in edges.iter() {
            if edge.target_id == target && edge.kind == kind { return; }
        }
        edges.push(crate::models::Edge { target_id: target, kind });
    }
}