use std::path::{ Path, PathBuf };
use std::fs;
use std::collections::{ HashMap, HashSet };
use ignore::WalkBuilder;
use blake3;
use rayon::prelude::*;
use crate::manifest::{ scan_manifests, ManifestResult };
use crate::models::{
    FileNode,
    FileId,
    SymbolNode,
    SymbolKind,
    SymbolId,
    WorkspaceIndex,
    SymbolIndex,
    FileAnalysis,
};
use crate::analysis::analyze_source;
use crate::analysis::language::{ get_language_configs, LanguageConfig };
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
        lookup: &mut SymbolIndex,
        root_id: Option<&str>
    ) {
        let root_abs = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        let active_root_id = root_id.unwrap_or("default_root");

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
            logical_key: String,
            relative_path: String,
            manifest: ManifestResult,
            config: Option<LanguageConfig>,
            is_test: bool,
            hash: Option<[u8; 32]>,
            content: Option<String>,
        }

        let initial_results: Vec<InitialFileResult> = file_entries
            .into_par_iter()
            .map(|path| {
                let relative_path = pathdiff
                    ::diff_paths(&path, &root_abs)
                    .unwrap_or_else(|| path.clone())
                    .to_string_lossy()
                    .to_string()
                    .replace('\\', "/");

                let logical_key = format!("::{}::{}", active_root_id, relative_path);

                let manifest = scan_manifests(&path);

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
                        } )
                    .cloned();

                let is_test = match path.strip_prefix(&root_abs) {
                    Ok(rel) => utils::is_test_path(rel),
                    Err(_) => {
                        let fname = path.file_name().map(Path::new).unwrap_or(&path);
                        utils::is_test_path(fname)
                    }
                };

                // CHANGE: Attempt to read and hash ALL files, not just configured ones.
                // read_to_string will fail for binary files (non-UTF8), effectively filtering them out.
                let mut hash = None;
                let mut content = None;
                if let Ok(c) = fs::read_to_string(&path) {
                    hash = Some(blake3::hash(c.as_bytes()).into());
                    content = Some(c);
                }

                InitialFileResult {
                    path,
                    logical_key,
                    relative_path,
                    manifest,
                    config,
                    is_test,
                    hash,
                    content,
                }
            })
            .collect();

        // 3. Filter and trigger slow analysis in parallel
        struct ProcessingResult {
            path: PathBuf,
            logical_key: String,
            relative_path: String,
            config: Option<LanguageConfig>, // Changed: Now Optional
            is_test: bool,
            hash: [u8; 32],
            analysis: Result<FileAnalysis, String>,
        }

        let mut seen_keys = HashSet::new();
        let mut to_process = Vec::new();

        for res in initial_results {
            if let Some(pkg_name) = res.manifest.package_name.clone() {
                if let Some(parent_dir) = res.path.parent() {
                    let dir_key = utils::to_index_path(parent_dir);
                    lookup.package_path_map.insert(pkg_name, dir_key);
                }
            }
            if !res.manifest.externals.is_empty() {
                lookup.external_packages.extend(res.manifest.externals.clone());
            }
            if !res.manifest.aliases.is_empty() {
                lookup.import_mappings.extend(res.manifest.aliases.clone());
            }

            // CHANGE: Proceed if we have Content/Hash, regardless of Config presence
            if let (Some(hash), Some(content)) = (res.hash, res.content) {
                seen_keys.insert(res.logical_key.clone());

                let needs_update = match index.files.get(&res.logical_key) {
                    Some(node) => node.hash != hash,
                    None => true,
                };

                if needs_update {
                    to_process.push((
                        res.path,
                        res.logical_key,
                        res.relative_path,
                        res.manifest,
                        res.config,
                        res.is_test,
                        hash,
                        content,
                    ));
                }
            }
        }

        // Run Analysis (Expensive)
        let analysis_results: Vec<ProcessingResult> = to_process
            .into_par_iter()
            .map(|(path, logical_key, relative_path, _manifest, config, is_test, hash, content)| {
                // CHANGE: Run analysis only if config exists, otherwise return empty analysis
                let analysis = if let Some(ref c) = config {
                    analyze_source(&path, &content, c)
                } else {
                    Ok(FileAnalysis {
                        functions: vec![],
                        imports: vec![],
                        exports: vec![],
                        literals: vec![],
                        implementations: vec![],
                        global_vars: HashMap::new(),
                        middleware_usage: vec![],
                        defined_routes: vec![],
                    })
                };

                ProcessingResult {
                    path,
                    logical_key,
                    relative_path,
                    config,
                    is_test,
                    hash,
                    analysis,
                }
            })
            .collect();

        // 4. Sequential state merge (Update Index Source of Truth)
        for res in analysis_results {
            self.update_file_from_analysis(
                &res.logical_key,
                &res.relative_path,
                active_root_id,
                &res.path,
                res.analysis,
                res.hash,
                res.config.as_ref(), // Pass Option reference
                res.is_test,
                index,
                lookup
            );
        }

        // 5. Cleanup Removed Files
        let prefix = format!("::{}::", active_root_id);
        let to_remove: Vec<String> = index.files
            .keys()
            .filter(|key| { key.starts_with(&prefix) && !seen_keys.contains(*key) })
            .cloned()
            .collect();

        for key in to_remove {
            self.remove_file(&key, index, lookup);
        }
    }

    fn update_file_from_analysis(
        &self,
        logical_key: &str,
        relative_path: &str,
        root_id: &str,
        path_obj: &Path,
        analysis_res: Result<FileAnalysis, String>,
        hash: [u8; 32],
        config: Option<&LanguageConfig>, // Changed: Now Optional
        is_path_test: bool,
        index: &mut WorkspaceIndex,
        lookup: &mut SymbolIndex
    ) {
        let file_id = match index.files.get(logical_key) {
            Some(node) => node.id,
            None => {
                let id = index.next_file_id;
                index.next_file_id += 1;
                id
            }
        };

        self.clear_file_symbols(file_id, index, lookup);

        if let Ok(analysis) = analysis_res {
            if !analysis.imports.is_empty() {
                index.file_imports.insert(file_id, analysis.imports.clone());
                lookup.file_imports.insert(file_id, analysis.imports);
            }
            if !analysis.exports.is_empty() {
                index.file_exports.insert(file_id, analysis.exports.clone());
                lookup.file_exports.insert(file_id, analysis.exports);
            }

            index.files.insert(logical_key.to_string(), FileNode {
                id: file_id,
                root_id: root_id.to_string(),
                relative_path: relative_path.to_string(),
                hash,
                is_test: is_path_test,
                literals: analysis.literals,
                middleware_usage: analysis.middleware_usage,
            });

            let mut file_symbol_ids = Vec::new();

            for func in analysis.functions {
                let symbol_id = index.next_symbol_id;
                index.next_symbol_id += 1;

                let is_inline_test =
                    func.kind != SymbolKind::Module &&
                    (func.source_code.contains("it(") ||
                        func.name.contains("test") ||
                        func.decorators.iter().any(|d| d.contains("test")));

                let node = SymbolNode {
                    id: symbol_id,
                    file_id,
                    parent_id: None,
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

                if func.name != "anonymous" {
                    lookup.symbol_map.entry(func.name.clone()).or_default().push(symbol_id);
                }
            }

            // 4. Config Definitions
            if let Some(c) = config {
                let is_data = matches!(
                    c.lang,
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
            }

            // 5. Container/Module Hierarchy
            let container_ids: Vec<SymbolId> = file_symbol_ids
                .iter()
                .filter(|&&id| {
                    let s = &index.symbols[&id];
                    s.kind == SymbolKind::Container || s.kind == SymbolKind::Module
                })
                .cloned()
                .collect();

            for c_id in container_ids {
                let (cs, ce, c_kind) = {
                    let c = &index.symbols[&c_id];
                    (c.range_start, c.range_end, c.kind)
                };

                for &s_id in &file_symbol_ids {
                    if s_id == c_id {
                        continue;
                    }

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

            let module_id = file_symbol_ids
                .iter()
                .find(|&&id| index.symbols[&id].kind == SymbolKind::Module)
                .cloned();

            if let Some(mid) = module_id {
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

            // 6. Route Lookups
            if let Some(route) = utils::detect_framework_route(path_obj) {
                if let Some(mid) = module_id {
                    lookup.implicit_routes.entry(route).or_default().push(mid);
                }
            }
            for &symbol_id in &file_symbol_ids {
                if let Some(sym) = index.symbols.get(&symbol_id) {
                    for r in &sym.routes {
                        lookup.implicit_routes.entry(r.clone()).or_default().push(symbol_id);
                    }
                }
            }
        }
    }

    pub fn remove_file(
        &self,
        logical_key: &str,
        index: &mut WorkspaceIndex,
        lookup: &mut SymbolIndex
    ) {
        if let Some(node) = index.files.remove(logical_key) {
            self.clear_file_symbols(node.id, index, lookup);
            index.file_dependencies.remove(&node.id);

            for ids in lookup.implicit_routes.values_mut() {
                ids.retain(|&sym_id| {
                    index.symbols.get(&sym_id).map_or(false, |s| s.file_id != node.id)
                });
            }
            lookup.implicit_routes.retain(|_, ids| !ids.is_empty());
        }
    }

    fn clear_file_symbols(
        &self,
        file_id: FileId,
        index: &mut WorkspaceIndex,
        lookup: &mut SymbolIndex
    ) {
        let ids_to_remove: Vec<SymbolId> = index.symbols
            .values()
            .filter(|s| s.file_id == file_id)
            .map(|s| s.id)
            .collect();

        for &sym_id in &ids_to_remove {
            if let Some(sym) = index.symbols.remove(&sym_id) {
                if let Some(id_list) = lookup.symbol_map.get_mut(&sym.name) {
                    id_list.retain(|&id| id != sym_id);
                    if id_list.is_empty() {
                        lookup.symbol_map.remove(&sym.name);
                    }
                }
            }
            index.graph.remove(&sym_id);
        }

        for def_list in lookup.config_definitions.values_mut() {
            def_list.retain(|id| !ids_to_remove.contains(id));
        }
        lookup.config_definitions.retain(|_, v| !v.is_empty());

        index.file_imports.remove(&file_id);
        index.file_exports.remove(&file_id);

        lookup.file_imports.remove(&file_id);
        lookup.file_exports.remove(&file_id);
        lookup.file_to_module.remove(&file_id);
    }
}