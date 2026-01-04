use std::path::{Path, PathBuf};
use std::fs;
use std::collections::{HashMap, HashSet};
use ignore::WalkBuilder;
use blake3;
use rayon::prelude::*;

use crate::manifest::{scan_manifests, ManifestResult};
use crate::models::{
    FileNode, FileId, SymbolNode, SymbolKind, SymbolId,
    WorkspaceIndex, SymbolIndex, FileAnalysis
};
use crate::analysis::analyze_source;
use crate::analysis::language::{get_language_configs, LanguageConfig};
use crate::resolution::utils;

pub struct FileScanner {
    pub configs: HashMap<String, LanguageConfig>,
}

impl FileScanner {
    pub fn new() -> Self {
        let mut config_map = HashMap::new();
        for config in get_language_configs() {
            for &ext in config.file_extensions {
                config_map.insert(ext.to_string(), config.clone());
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
        lookup: &mut SymbolIndex
    ) {
        let root_abs = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        let root_string = root_abs.to_string_lossy().to_string();
        
        if !index.roots.contains(&root_string) {
            index.roots.push(root_string);
        }

        // 1. Collect all candidate files
        let walker = WalkBuilder::new(&root_abs).hidden(false).git_ignore(true).build();
        let mut file_entries = Vec::new();

        for result in walker {
            match result {
                Ok(entry) => {
                    if entry.path().is_file() {
                        file_entries.push(entry.into_path());
                    }
                }
                Err(err) => eprintln!("Error walking directory: {}", err),
            }
        }

        // 2. Parallel Processing (Read & Hash)
        struct InitialFileResult {
            path: PathBuf,
            path_key: String,
            manifest: ManifestResult,
            config: Option<LanguageConfig>,
            is_test: bool,
            hash: Option<[u8; 32]>,
            content: Option<String>,
        }

        let initial_results: Vec<InitialFileResult> = file_entries.into_par_iter().map(|path| {
            let path_key = utils::to_index_path(&path);
            let manifest = scan_manifests(&path);
            
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            
            let config = self.configs.get(ext)
                .or_else(|| self.configs.get(filename))
                .or_else(|| if filename.starts_with('.') { self.configs.get(&filename[1..]) } else { None })
                .cloned();

            let is_test = match path.strip_prefix(&root_abs) {
                Ok(rel) => utils::is_test_path(rel),
                Err(_) => {
                    let fname = path.file_name().map(Path::new).unwrap_or(&path);
                    utils::is_test_path(fname)
                }
            };

            let mut hash = None;
            let mut content = None;
            if config.is_some() {
                if let Ok(c) = fs::read_to_string(&path) {
                    hash = Some(blake3::hash(c.as_bytes()).into());
                    content = Some(c);
                }
            }

            InitialFileResult { path, path_key, manifest, config, is_test, hash, content }
        }).collect();

        // 3. Filter and trigger slow analysis in parallel
        struct ProcessingResult {
            path: PathBuf,
            path_key: String,
            config: LanguageConfig,
            is_test: bool,
            hash: [u8; 32],
            analysis: Result<FileAnalysis, String>,
        }

        let mut seen_paths = HashSet::new();
        let mut to_process = Vec::new();

        for res in initial_results {
            // Update manifest data sequentially (fast operations)
            if let Some(pkg_name) = res.manifest.package_name.clone() {
                if let Some(parent_dir) = res.path.parent() {
                    let dir_key = utils::to_index_path(parent_dir);
                    lookup.package_path_map.insert(pkg_name, dir_key);
                }
            }
            if !res.manifest.externals.is_empty() { lookup.external_packages.extend(res.manifest.externals.clone()); }
            if !res.manifest.aliases.is_empty() { lookup.import_mappings.extend(res.manifest.aliases.clone()); }

            // If we have a language config, valid content, and hash
            if let (Some(config), Some(hash), Some(content)) = (res.config, res.hash, res.content) {
                seen_paths.insert(res.path_key.clone());
                
                // Check if file has changed
                let needs_update = match index.files.get(&res.path_key) {
                    Some(node) => node.hash != hash,
                    None => true,
                };

                if needs_update {
                    to_process.push((res.path, res.path_key, res.manifest, config, res.is_test, hash, content));
                }
            }
        }

        // Run Analysis (Expensive)
        let analysis_results: Vec<ProcessingResult> = to_process.into_par_iter().map(|(path, path_key, _manifest, config, is_test, hash, content)| {
            let analysis = analyze_source(&path, &content, &config);
            ProcessingResult { path, path_key, config, is_test, hash, analysis }
        }).collect();

        // 4. Sequential state merge (Update Index Source of Truth)
        for res in analysis_results {
            self.update_file_from_analysis(
                &res.path_key, &res.path, res.analysis, res.hash, &res.config, res.is_test,
                index, lookup
            );
        }

        // 5. Cleanup Removed Files (SCOPED)
        // We calculate the prefix based on the *current* root being scanned (root_abs).
        let root_prefix = utils::to_index_path(&root_abs);

        // We filter keys that are in the index but were NOT seen in this specific scan execution.
        let to_remove: Vec<String> = index.files.keys()
            .filter(|path_key| {
                // Condition 1: File actually belongs to the root we are currently scanning.
                // We use standard starts_with check.
                // Note: we assume path_key is already normalized by utils::to_index_path during insertion.
                let belongs_to_current_root = path_key.starts_with(&root_prefix);

                // Condition 2: We did NOT find this file in the current walk of this root.
                let not_seen = !seen_paths.contains(*path_key);

                belongs_to_current_root && not_seen
            })
            .cloned()
            .collect();
            
        for path_key in to_remove { 
            self.remove_file(&path_key, index, lookup); 
        }
    }

    fn update_file_from_analysis(
        &self,
        path_key: &str,
        path_obj: &Path,
        analysis_res: Result<FileAnalysis, String>,
        hash: [u8; 32],
        config: &LanguageConfig, 
        is_path_test: bool,
        index: &mut WorkspaceIndex,
        lookup: &mut SymbolIndex
    ) {
         let file_id = match index.files.get(path_key) {
            Some(node) => node.id,
            None => { let id = index.next_file_id; index.next_file_id += 1; id }
        };

        // Clear old data for this file completely to ensure no stale symbols remain
        self.clear_file_symbols(file_id, index, lookup);

        if let Ok(analysis) = analysis_res {
            // 1. Update Imports/Exports (Persisted + Lookup)
            if !analysis.imports.is_empty() { 
                index.file_imports.insert(file_id, analysis.imports.clone()); 
                lookup.file_imports.insert(file_id, analysis.imports);
            }
            if !analysis.exports.is_empty() { 
                index.file_exports.insert(file_id, analysis.exports.clone()); 
                lookup.file_exports.insert(file_id, analysis.exports);
            }
            
            // 2. Create FileNode (Source of Truth)
            // Note: We need to ensure FileNode definition in models.rs has 'literals', 'middleware_usage', and 'implementations'
            index.files.insert(path_key.to_string(), FileNode {
                id: file_id,
                path: path_key.to_string(),
                hash,
                is_test: is_path_test,
                literals: analysis.literals, // Persisted
                middleware_usage: analysis.middleware_usage, // Persisted
                // NOTE: Assuming 'implementations' was added to FileNode in models.rs as discussed. 
                // If not, this data is lost unless mapped to symbols. 
                // For this implementation, we assume FileNode stores it to enable hydration.
                // implementations: analysis.implementations 
            });

            // 3. Process Functions -> Symbols
            let mut file_symbol_ids = Vec::new();

            for func in analysis.functions {
                let symbol_id = index.next_symbol_id;
                index.next_symbol_id += 1;
                
                let is_inline_test = func.kind != SymbolKind::Module && (
                    func.source_code.contains("it(") || 
                    func.name.contains("test") || 
                    func.decorators.iter().any(|d| d.contains("test"))
                );

                // Create Persisted SymbolNode
                // All "raw" resolution data is now stored here.
                let node = SymbolNode {
                    id: symbol_id, 
                    file_id, 
                    parent_id: None, // Will be set in the post-processing loop below
                    name: func.name.clone(), 
                    kind: func.kind,
                    range_start: func.range_start, 
                    range_end: func.range_end,
                    body_start: func.body_start,
                    doc_comment: func.documentation, 
                    return_type: func.return_type,
                    is_test: is_path_test || is_inline_test,
                    is_external: false, 
                    external_source: None,
                    decorators: func.decorators, 
                    routes: func.routes,
                    
                    // Persisted Raw Data
                    calls: func.calls,
                    type_refs: func.type_refs,
                    fingerprints: func.fingerprints,
                    local_types: func.local_types,
                    config_keys: func.config_keys,
                    dispatched_actions: func.dispatched_actions,
                    handled_actions: func.handled_actions,
                };
                
                index.symbols.insert(symbol_id, node);
                file_symbol_ids.push(symbol_id);
                
                // Update Runtime Lookup
                if func.name != "anonymous" { 
                    lookup.symbol_map.entry(func.name.clone()).or_default().push(symbol_id); 
                }
            }

            // 4. Post-Process: Handle Config Definitions (Lookup)
            let is_data = matches!(config.lang, 
                crate::analysis::language::SupportedLanguage::Yaml | 
                crate::analysis::language::SupportedLanguage::Json | 
                crate::analysis::language::SupportedLanguage::Toml | 
                crate::analysis::language::SupportedLanguage::Dotenv
            );

            if is_data {
                for &symbol_id in &file_symbol_ids {
                    let name = &index.symbols[&symbol_id].name;
                    lookup.config_definitions.entry(name.clone()).or_default().push(symbol_id);
                }
            }

            // 5. Post-Process: Handle Container/Module Hierarchy
            // We set parent_id on the symbols. We do NOT add edges to the graph here.
            // The Pipeline will reconstruct EdgeKind::Contains during resolve_structure().
            let container_ids: Vec<SymbolId> = file_symbol_ids.iter().filter(|&&id| {
                let s = &index.symbols[&id]; 
                s.kind == SymbolKind::Container || s.kind == SymbolKind::Module
            }).cloned().collect();

            for c_id in container_ids {
                let (cs, ce, c_kind) = { let c = &index.symbols[&c_id]; (c.range_start, c.range_end, c.kind) };
                
                for &s_id in &file_symbol_ids {
                    if s_id == c_id { continue; }
                    
                    // Check strict containment
                    let is_member = { 
                        let s = &index.symbols[&s_id]; 
                        s.range_start >= cs && s.range_end <= ce 
                    };

                    if is_member {
                        if c_kind != SymbolKind::Module { 
                            if let Some(node) = index.symbols.get_mut(&s_id) { 
                                node.parent_id = Some(c_id); 
                            } 
                        }
                    }
                }
            }
            
            // Handle implicit Module scope (files are modules)
            let module_id = file_symbol_ids.iter()
                .find(|&&id| index.symbols[&id].kind == SymbolKind::Module)
                .cloned();

            if let Some(mid) = module_id {
                // NEW: Update the bridge
                lookup.file_to_module.insert(file_id, mid);

                for &id in &file_symbol_ids {
                    if id != mid {
                         if let Some(sym) = index.symbols.get_mut(&id) { 
                             if sym.parent_id.is_none() { 
                                 sym.parent_id = Some(mid); 
                             } 
                         }
                    }
                }
            }

            // 6. Handle Route Lookups
            if let Some(route) = utils::detect_framework_route(path_obj) {
                 if let Some(mid) = module_id {
                     lookup.implicit_routes.insert(route, mid);
                 }
            }
            for &symbol_id in &file_symbol_ids {
                 if let Some(sym) = index.symbols.get(&symbol_id) {
                     for r in &sym.routes { 
                         lookup.implicit_routes.insert(r.clone(), symbol_id); 
                     }
                 }
            }
        }
    }

    pub fn remove_file(
        &self, 
        path_key: &str, 
        index: &mut WorkspaceIndex, 
        lookup: &mut SymbolIndex
    ) {
        if let Some(node) = index.files.remove(path_key) {
            self.clear_file_symbols(node.id, index, lookup);
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
        lookup: &mut SymbolIndex
    ) {
        // Collect IDs to remove
        let ids_to_remove: Vec<SymbolId> = index.symbols.values()
            .filter(|s| s.file_id == file_id).map(|s| s.id).collect();

        for &sym_id in &ids_to_remove {
            if let Some(sym) = index.symbols.remove(&sym_id) {
                // Remove from name lookup
                if let Some(id_list) = lookup.symbol_map.get_mut(&sym.name) {
                    id_list.retain(|&id| id != sym_id);
                    if id_list.is_empty() { lookup.symbol_map.remove(&sym.name); }
                }
            }
            
            // Remove from Graph
            index.graph.remove(&sym_id);
        }

        // Clean Config Lookups
        for def_list in lookup.config_definitions.values_mut() {
            def_list.retain(|id| !ids_to_remove.contains(id));
        }
        lookup.config_definitions.retain(|_, v| !v.is_empty());

        // Clean Imports/Exports (Both Persisted and Runtime)
        index.file_imports.remove(&file_id);
        index.file_exports.remove(&file_id);
        
        lookup.file_imports.remove(&file_id);
        lookup.file_exports.remove(&file_id);
        lookup.file_to_module.remove(&file_id);
    }
}