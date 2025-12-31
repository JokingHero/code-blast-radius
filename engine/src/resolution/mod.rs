pub mod persistence;
pub mod utils;
pub mod resolvers;

use crate::manifest::scan_manifests;
use crate::models::{FileId, FileNode, SymbolId, SymbolKind, SymbolNode, WorkspaceIndex};
use crate::analysis::analyze_source;
use crate::analysis::language::{get_language_configs, LanguageConfig};
use std::path::Path;
use std::fs;
use std::collections::{HashMap, HashSet};
use ignore::WalkBuilder;
use blake3;

pub struct Indexer {
    pub index: WorkspaceIndex,
    configs: HashMap<String, &'static LanguageConfig>,
    resolution_cache: HashMap<(FileId, String), Option<SymbolId>>,
}

impl Indexer {
    pub fn new() -> Self {
        let mut config_map = HashMap::new();
        for config in get_language_configs() {
            for ext in config.file_extensions {
                config_map.insert(ext.to_string(), config);
            }
        }
        Self {
            index: WorkspaceIndex::default(),
            configs: config_map,
            resolution_cache: HashMap::new(),
        }
    }

    pub fn resolve_references(&mut self) {
        self.index.resolved_calls.clear();
        self.index.resolved_type_refs.clear();
        self.index.inheritance.clear();
        self.index.file_dependencies.clear();
        self.resolution_cache.clear();

        // 1. Core imports and basic structure
        self.resolve_external_imports();
        self.resolve_decorators();
        self.resolve_implicit_routes();
        self.resolve_namespace_imports();

        // 2. Data and Literals
        self.resolve_literal_dependencies();
        self.resolve_shared_literals();
        self.resolve_pubsub_wildcards();
        
        // 3. Inference & Magic
        self.resolve_type_sniffing();
        self.resolve_magic_proxies();
        self.resolve_fingerprints();
        self.resolve_implicit_connections();
        
        // 4. Frameworks & State
        self.resolve_dependency_injection();
        self.resolve_function_calls_with_fallback(); // Renamed in standard.rs to resolve_function_calls, need alias or rename there
        self.resolve_config_links();
        self.resolve_type_references();
        self.resolve_database_references();
        self.resolve_file_dependencies();
        self.resolve_state_management();
        self.resolve_middleware_injection();
        self.resolve_iac_links();
    }
    
    // Alias to match existing name in tests/internal usage if any
    fn resolve_function_calls_with_fallback(&mut self) {
        self.resolve_function_calls();
    }

    // --- Scanning Logic (Kept here as it drives the process) ---

    pub fn scan(&mut self, root: &Path) {
        let root_abs = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        let root_string = root_abs.to_string_lossy().to_string();
        if !self.index.roots.contains(&root_string) {
            self.index.roots.push(root_string);
        }

        let mut seen_paths = HashSet::new();
        let walker = WalkBuilder::new(&root_abs).hidden(false).git_ignore(true).build();

        for result in walker {
            match result {
                Ok(entry) => {
                    if !entry.path().is_file() { continue; }
                    let path = entry.path();

                    // Manifests
                    let manifest_res = scan_manifests(path);
                    if let Some(pkg_name) = manifest_res.package_name {
                        if let Some(parent_dir) = path.parent() {
                            let dir_key = utils::to_index_path(parent_dir);
                            self.index.package_path_map.insert(pkg_name, dir_key);
                        }
                    }
                    if !manifest_res.externals.is_empty() { self.index.external_packages.extend(manifest_res.externals); }
                    if !manifest_res.aliases.is_empty() { self.index.import_mappings.extend(manifest_res.aliases); }

                    // Source Files
                    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
                    let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    
                    let config = self.configs.get(ext)
                        .or_else(|| self.configs.get(filename))
                        .or_else(|| if filename.starts_with('.') { self.configs.get(&filename[1..]) } else { None });

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

                            let needs_update = match self.index.files.get(&path_key) {
                                Some(node) => node.hash != hash_bytes,
                                None => true,
                            };
                            if needs_update {
                                self.update_file(&path_key, path, &content, hash_bytes, config, is_test);
                            }
                        }
                    }
                }
                Err(err) => eprintln!("Error walking directory: {}", err),
            }
        }

        let to_remove: Vec<String> = self.index.files.keys()
            .filter(|path_key| !seen_paths.contains(*path_key))
            .cloned().collect();
        for path_key in to_remove { self.remove_file(&path_key); }
    }

    fn remove_file(&mut self, path_key: &str) {
        if let Some(node) = self.index.files.remove(path_key) {
            self.clear_file_symbols(node.id);
            self.index.file_dependencies.remove(&node.id);
            self.index.implicit_routes.retain(|_, sym_id| {
                self.index.symbols.get(sym_id).map_or(false, |s| s.file_id != node.id)
            });
        }
    }

    fn clear_file_symbols(&mut self, file_id: FileId) {
        let ids_to_remove: Vec<SymbolId> = self.index.symbols.values()
            .filter(|s| s.file_id == file_id).map(|s| s.id).collect();

        for &sym_id in &ids_to_remove {
            if let Some(sym) = self.index.symbols.remove(&sym_id) {
                if let Some(id_list) = self.index.symbol_map.get_mut(&sym.name) {
                    id_list.retain(|&id| id != sym_id);
                    if id_list.is_empty() { self.index.symbol_map.remove(&sym.name); }
                }
            }
            // Cleanup maps
            self.index.raw_calls.remove(&sym_id);
            self.index.raw_implementations.remove(&sym_id);
            self.index.resolved_calls.remove(&sym_id);
            self.index.fingerprints.remove(&sym_id);
            self.index.container_methods.remove(&sym_id);
            self.index.local_variable_types.remove(&sym_id);
            self.index.inheritance.remove(&sym_id);
            self.index.symbol_config_refs.remove(&sym_id);
            self.index.raw_type_refs.remove(&sym_id);
            self.index.resolved_type_refs.remove(&sym_id);
            self.index.raw_decorators.remove(&sym_id);
        }
        
        for def_list in self.index.config_definitions.values_mut() {
            def_list.retain(|id| !ids_to_remove.contains(id));
        }
        self.index.config_definitions.retain(|_, v| !v.is_empty());
        self.index.file_imports.remove(&file_id);
        self.index.file_exports.remove(&file_id);
        self.index.raw_literals.remove(&file_id);
    }

    fn update_file(&mut self, path_key: &str, path_obj: &Path, content: &str, hash: [u8; 32], config: &LanguageConfig, is_path_test: bool) {
        let file_id = match self.index.files.get(path_key) {
            Some(node) => node.id,
            None => { let id = self.index.next_file_id; self.index.next_file_id += 1; id }
        };

        self.clear_file_symbols(file_id);
        self.index.files.insert(path_key.to_string(), FileNode {
            id: file_id, path: path_key.to_string(), hash, is_test: is_path_test,
        });

        if let Ok(analysis) = analyze_source(path_obj, content, config) {
            if !analysis.imports.is_empty() { self.index.file_imports.insert(file_id, analysis.imports); }
            if !analysis.exports.is_empty() { self.index.file_exports.insert(file_id, analysis.exports); }
            if !analysis.literals.is_empty() { self.index.raw_literals.insert(file_id, analysis.literals); }
            if !analysis.middleware_usage.is_empty() { self.index.raw_middleware_usage.insert(file_id, analysis.middleware_usage); }

            let mut file_symbol_ids = Vec::new();
            for func in analysis.functions {
                let symbol_id = self.index.next_symbol_id;
                self.index.next_symbol_id += 1;
                
                let is_inline_test = func.kind != SymbolKind::Module && (
                    func.source_code.contains("it(") || func.name.contains("test") || func.decorators.iter().any(|d| d.contains("test"))
                );

                self.index.symbols.insert(symbol_id, SymbolNode {
                    id: symbol_id, file_id, parent_id: None,
                    name: func.name.clone(), kind: func.kind,
                    range_start: func.range_start, range_end: func.range_end,
                    doc_comment: func.documentation, return_type: func.return_type,
                    is_test: is_path_test || is_inline_test,
                    is_external: false, external_source: None,
                    decorators: func.decorators.clone(), routes: func.routes.clone(),
                });
                
                file_symbol_ids.push(symbol_id);
                if func.name != "anonymous" { self.index.symbol_map.entry(func.name.clone()).or_default().push(symbol_id); }
                
                // Populate maps
                if !func.config_keys.is_empty() { self.index.symbol_config_refs.insert(symbol_id, func.config_keys); }
                if !func.calls.is_empty() { self.index.raw_calls.insert(symbol_id, func.calls); }
                if !func.fingerprints.is_empty() { self.index.fingerprints.insert(symbol_id, func.fingerprints); }
                if !func.local_types.is_empty() { self.index.local_variable_types.insert(symbol_id, func.local_types); }
                if !func.type_refs.is_empty() { self.index.raw_type_refs.insert(symbol_id, func.type_refs); }
                if !func.decorators.is_empty() { self.index.raw_decorators.insert(symbol_id, func.decorators); }
                if !func.dispatched_actions.is_empty() { self.index.raw_action_dispatches.insert(symbol_id, func.dispatched_actions); }
                if !func.handled_actions.is_empty() { self.index.raw_action_handlers.insert(symbol_id, func.handled_actions); }
            }

            // Post-analysis linking (Containers, Config defs, Implicit routes)
            // ... (Logic from original update_file for container/member linking kept here or moved to helpers if huge)
            // For brevity, assuming the Container/Member linking logic stays here or is extracted similarly.
            // Copied standard logic:
            
            let is_data = matches!(config.lang_enum, crate::analysis::language::SupportedLanguage::Yaml | crate::analysis::language::SupportedLanguage::Json | crate::analysis::language::SupportedLanguage::Toml | crate::analysis::language::SupportedLanguage::Dotenv);
            if is_data {
                for &sid in &file_symbol_ids {
                    let name = &self.index.symbols[&sid].name;
                    self.index.config_definitions.entry(name.clone()).or_default().push(sid);
                }
            }

            let container_ids: Vec<SymbolId> = file_symbol_ids.iter().filter(|&&id| {
                let s = &self.index.symbols[&id]; 
                s.kind == SymbolKind::Container || s.kind == SymbolKind::Module
            }).cloned().collect();

            for c_id in container_ids {
                let (cs, ce, c_kind) = { let c = &self.index.symbols[&c_id]; (c.range_start, c.range_end, c.kind) };
                let mut members = HashSet::new();
                for &s_id in &file_symbol_ids {
                    if s_id == c_id { continue; }
                    let is_member = { let s = &self.index.symbols[&s_id]; s.range_start >= cs && s.range_end <= ce };
                    if is_member {
                        members.insert(self.index.symbols[&s_id].name.clone());
                        if c_kind != SymbolKind::Module { 
                            if let Some(node) = self.index.symbols.get_mut(&s_id) { 
                                node.parent_id = Some(c_id); 
                            } 
                        }
                    }
                }
                if !members.is_empty() { self.index.container_methods.insert(c_id, members); }
            }
            
            // Orphans to module
            let module_id = file_symbol_ids.iter().find(|&&id| self.index.symbols[&id].kind == SymbolKind::Module).cloned();
            if let Some(mid) = module_id {
                for &id in &file_symbol_ids {
                    if id != mid {
                         if let Some(sym) = self.index.symbols.get_mut(&id) { if sym.parent_id.is_none() { sym.parent_id = Some(mid); } }
                    }
                }
            }

            for (child, parent) in analysis.implementations {
                if let Some(ids) = self.index.symbol_map.get(&child) {
                    if let Some(&cid) = ids.iter().find(|&&id| self.index.symbols[&id].file_id == file_id) {
                        self.index.raw_implementations.entry(cid).or_default().push(parent);
                    }
                }
            }

            if let Some(route) = utils::detect_framework_route(path_obj) {
                 if let Some(mid) = file_symbol_ids.iter().find(|&&id| self.index.symbols[&id].kind == SymbolKind::Module) {
                     self.index.implicit_routes.insert(route, *mid);
                 }
            }
            for &sid in &file_symbol_ids {
                 if let Some(sym) = self.index.symbols.get(&sid) {
                     for r in &sym.routes { self.index.implicit_routes.insert(r.clone(), sid); }
                 }
            }
        }
    }

    pub fn get_impacted_files(&self, target_path: &Path) -> Vec<String> {
        let target_key = utils::to_index_path(target_path);
        let target_id = match self.index.files.get(&target_key) {
            Some(node) => node.id, None => return vec![],
        };
        let mut impacted_paths = Vec::new();
        for (&source_file_id, dependencies) in &self.index.file_dependencies {
            if dependencies.contains(&target_id) {
                if let Some(source_node) = self.index.files.values().find(|f| f.id == source_file_id) {
                    impacted_paths.push(source_node.path.clone());
                }
            }
        }
        impacted_paths
    }
}