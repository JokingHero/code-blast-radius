use crate::schema::{WorkspaceIndex, FileNode, SymbolNode, SymbolId, FileId};
use crate::analyzer::analyze_source;
use crate::language::{get_language_configs, LanguageConfig};

use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::Write;
use std::collections::{HashMap, HashSet};
use walkdir::WalkDir;
use blake3;
use memmap2::MmapOptions;
use rkyv::{to_bytes, check_archived_root};

pub struct Indexer {
    pub index: WorkspaceIndex,
    configs: HashMap<String, &'static LanguageConfig>, 
}

impl Indexer {
    pub fn new() -> Self {
        let mut config_map = HashMap::new();
        for config in get_language_configs() {
            for ext in config.file_extensions {
                config_map.insert(ext.to_string(), config);
            }
        }
        Self { index: WorkspaceIndex::default(), configs: config_map }
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let bytes = to_bytes::<_, 4096>(&self.index)
            .map_err(|e| anyhow::anyhow!("Serialization failed: {}", e))?;
        let mut file = File::create(path)?;
        file.write_all(&bytes)?;
        Ok(())
    }

    pub fn load_from_file(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() { return Ok(Self::new()); }
        let file = File::open(path)?;
        let mmap = unsafe { MmapOptions::new().map(&file)? };
        if let Err(e) = check_archived_root::<WorkspaceIndex>(&mmap[..]) {
            eprintln!("Index corrupted: {}", e);
            return Ok(Self::new());
        }
        let index: WorkspaceIndex = unsafe { 
            rkyv::from_bytes_unchecked(&mmap[..]).map_err(|e| anyhow::anyhow!(e))? 
        };
        let mut config_map = HashMap::new();
        for config in get_language_configs() {
            for ext in config.file_extensions {
                config_map.insert(ext.to_string(), config);
            }
        }
        Ok(Self { index, configs: config_map })
    }

    fn clear_file_symbols(&mut self, file_id: FileId) {
        let ids_to_remove: Vec<SymbolId> = self.index.symbols.values()
            .filter(|s| s.file_id == file_id)
            .map(|s| s.id)
            .collect();

        for sym_id in ids_to_remove {
            if let Some(sym) = self.index.symbols.remove(&sym_id) {
                if let Some(id_list) = self.index.symbol_map.get_mut(&sym.name) {
                    id_list.retain(|&id| id != sym_id);
                    if id_list.is_empty() {
                        self.index.symbol_map.remove(&sym.name);
                    }
                }
            }
            self.index.raw_calls.remove(&sym_id);
            self.index.raw_implementations.remove(&sym_id);
            self.index.resolved_calls.remove(&sym_id);
            self.index.fingerprints.remove(&sym_id);
            self.index.container_methods.remove(&sym_id);
        }
        self.index.file_imports.remove(&file_id);
        self.index.raw_literals.remove(&file_id);
    }

    pub fn scan_workspace(&mut self, roots: &[PathBuf]) {
        let mut seen_paths = HashSet::new();
        for root in roots {
            let root_abs = fs::canonicalize(root).unwrap_or_else(|_| root.clone());
            for entry in WalkDir::new(&root_abs).into_iter().filter_map(|e| e.ok()).filter(|e| e.path().is_file()) {
                let path = entry.path();
                let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
                if let Some(config) = self.configs.get(ext) {
                    if let Ok(content) = fs::read_to_string(path) {
                        let hash = blake3::hash(content.as_bytes());
                        let hash_bytes: [u8; 32] = hash.into();
                        let path_key = path.to_string_lossy().to_string();
                        seen_paths.insert(path_key.clone());
                        let needs_update = match self.index.files.get(&path_key) {
                            Some(node) => node.hash != hash_bytes,
                            None => true, 
                        };
                        if needs_update {
                            self.update_file(&path_key, path, &content, hash_bytes, config);
                        }
                    }
                }
            }
        }
        let all_known_paths: Vec<String> = self.index.files.keys().cloned().collect();
        for path in all_known_paths {
            if !seen_paths.contains(&path) {
                let id_to_remove = self.index.files.get(&path).map(|node| node.id);
                if let Some(id) = id_to_remove {
                    self.clear_file_symbols(id); 
                    self.index.files.remove(&path);
                }
            }
        }
    }

    pub fn scan(&mut self, root: &Path) {
        self.scan_workspace(&[root.to_path_buf()]);
    }

    fn update_file(&mut self, path_key: &str, path_obj: &Path, content: &str, hash: [u8; 32], config: &LanguageConfig) {
        let existing_id = self.index.files.get(path_key).map(|node| node.id);
        let file_id = if let Some(id) = existing_id {
            self.clear_file_symbols(id); 
            id
        } else {
            self.index.files.values().map(|f| f.id).max().map(|id| id + 1).unwrap_or(0)
        };

        self.index.files.insert(path_key.to_string(), FileNode { id: file_id, path: path_key.to_string(), hash });

        if let Ok(analysis) = analyze_source(path_obj, content, config) {
            if !analysis.imports.is_empty() { self.index.file_imports.insert(file_id, analysis.imports); }
            if !analysis.literals.is_empty() { self.index.raw_literals.insert(file_id, analysis.literals); }

            let mut file_symbol_ids = Vec::new();
            for func in analysis.functions {
                let symbol_id = self.index.symbols.keys().max().map(|k| k + 1).unwrap_or(0);
                
                // IMPROVED: Check for container keywords near the start of the source
                let prefix = func.source_code.chars().take(50).collect::<String>();
                let is_container = prefix.contains("class ") || 
                                   prefix.contains("interface ") || 
                                   prefix.contains("trait ") || 
                                   prefix.contains("enum ");

                let kind = if is_container { "container" } else { "function" };

                self.index.symbols.insert(symbol_id, SymbolNode {
                    id: symbol_id, file_id, parent_id: None,
                    name: func.name.clone(), kind: kind.to_string(), 
                    range_start: func.range_start, range_end: func.range_end,
                    doc_comment: func.documentation,
                });
                
                file_symbol_ids.push(symbol_id);
                self.index.symbol_map.entry(func.name.clone()).or_default().push(symbol_id);
                if !func.calls.is_empty() { self.index.raw_calls.insert(symbol_id, func.calls); }
                if !func.fingerprints.is_empty() { self.index.fingerprints.insert(symbol_id, func.fingerprints); }
            }

            // Link members to containers
            let containers: Vec<SymbolId> = file_symbol_ids.iter()
                .filter(|&&id| self.index.symbols.get(&id).unwrap().kind == "container")
                .cloned().collect();

            for c_id in containers {
                let (c_start, c_end) = {
                    let c = self.index.symbols.get(&c_id).unwrap();
                    (c.range_start, c.range_end)
                };
                let mut members = HashSet::new();
                for &s_id in &file_symbol_ids {
                    if s_id == c_id { continue; }
                    let s_data = self.index.symbols.get(&s_id).unwrap();
                    if s_data.range_start >= c_start && s_data.range_end <= c_end {
                        members.insert(s_data.name.clone());
                        // Update the node's parent_id
                        if let Some(node) = self.index.symbols.get_mut(&s_id) {
                            node.parent_id = Some(c_id);
                        }
                    }
                }
                if !members.is_empty() { self.index.container_methods.insert(c_id, members); }
            }

            for (child_name, parent_name) in analysis.implementations {
                if let Some(ids) = self.index.symbol_map.get(&child_name) {
                    if let Some(&child_id) = ids.iter().find(|&&id| self.index.symbols.get(&id).map(|s| s.file_id) == Some(file_id)) {
                        self.index.raw_implementations.entry(child_id).or_default().push(parent_name);
                    }
                }
            }
        }
    }

    pub fn resolve_references(&mut self) {
        // Order matters: Standard calls first, then specific fingerprints
        self.resolve_function_calls();
        self.resolve_fingerprints();
        self.resolve_bare_imports();
        self.resolve_implicit_connections();
    }

    fn resolve_fingerprints(&mut self) {
        let mut fingerprint_links = Vec::new();

        for (&caller_id, receiver_map) in &self.index.fingerprints {
            for (_, used_methods) in receiver_map {
                if used_methods.is_empty() { continue; }

                for (&container_id, container_methods) in &self.index.container_methods {
                    // Check if container satisfies the duck-type fingerprint
                    if used_methods.iter().all(|m| container_methods.contains(m)) {
                        // Link 1: Link the caller to the Container (Class/Interface)
                        fingerprint_links.push((caller_id, container_id));

                        // Link 2: Link the caller to the specific Methods used
                        for method_name in used_methods {
                            if let Some(ids) = self.index.symbol_map.get(method_name) {
                                for &m_id in ids {
                                    if let Some(sym) = self.index.symbols.get(&m_id) {
                                        // Only link if this method actually belongs to this container
                                        if sym.parent_id == Some(container_id) {
                                            fingerprint_links.push((caller_id, m_id));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        for (caller, target) in fingerprint_links {
            let entry = self.index.resolved_calls.entry(caller).or_default();
            if !entry.contains(&target) {
                entry.push(target);
            }
        }
    }

    fn resolve_function_calls(&mut self) {
        self.index.resolved_calls.clear();
        let entries: Vec<(SymbolId, Vec<String>)> = self.index.raw_calls.iter().map(|(k, v)| (*k, v.clone())).collect();

        for (caller_id, called_names) in entries {
            let caller_sym = self.index.symbols.get(&caller_id).unwrap();
            let caller_file_id = caller_sym.file_id;
            let mut resolved_targets = Vec::new();

            // Fetch the fingerprints for this caller once
            let caller_fingerprints = self.index.fingerprints.get(&caller_id);

            for func_name in called_names {
                // If this name appears as a property/method call in a fingerprint,
                // we DO NOT fall back to global search (to avoid polymorphism pollution).
                let is_method_call = caller_fingerprints.map_or(false, |f| {
                    f.values().any(|methods| methods.contains(&func_name))
                });

                if let Some(target_id) = self.resolve_single_call(caller_file_id, &func_name) {
                    resolved_targets.push(target_id);
                } else if !is_method_call {
                    // Global fallback only for plain function calls
                    if let Some(candidates) = self.index.symbol_map.get(&func_name) {
                        resolved_targets.extend(candidates.iter().cloned());
                    }
                }
            }
            resolved_targets.sort(); resolved_targets.dedup();
            if !resolved_targets.is_empty() { self.index.resolved_calls.insert(caller_id, resolved_targets); }
        }
    }

    fn resolve_bare_imports(&mut self) {
        let file_ids: Vec<FileId> = self.index.file_imports.keys().cloned().collect();
        for file_id in file_ids {
            let imports = self.index.file_imports.get(&file_id).unwrap().clone();
            for import in imports {
                if import.name.is_empty() {
                    if let Some(target_file_id) = self.resolve_import_path(file_id, &import.source) {
                        self.index.file_dependencies.entry(file_id).or_default().push(target_file_id);
                    }
                }
            }
        }
    }

    fn resolve_single_call(&self, file_id: FileId, func_name: &str) -> Option<SymbolId> {
        if let Some(candidates) = self.index.symbol_map.get(func_name) {
            for &id in candidates {
                if let Some(sym) = self.index.symbols.get(&id) {
                    if sym.file_id == file_id { return Some(id); }
                }
            }
        }
        if let Some(imports) = self.index.file_imports.get(&file_id) {
            for import in imports {
                if import.name == func_name {
                    if let Some(target_file_id) = self.resolve_import_path(file_id, &import.source) {
                        if let Some(candidates) = self.index.symbol_map.get(func_name) {
                            for &id in candidates {
                                if let Some(sym) = self.index.symbols.get(&id) {
                                    if sym.file_id == target_file_id { return Some(id); }
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn resolve_import_path(&self, from_file_id: FileId, import_source: &str) -> Option<FileId> {
        let from_file_node = self.index.files.values().find(|f| f.id == from_file_id)?;
        let from_path = Path::new(&from_file_node.path);
        let parent_dir = from_path.parent()?;
        let extensions = ["ts", "js", "tsx", "jsx", "rs", "py", "java", "sh"];
        let candidate_base = parent_dir.join(import_source);
        let check_path = |p: PathBuf| -> Option<FileId> {
             if let Ok(abs) = fs::canonicalize(&p) { return self.index.files.get(&abs.to_string_lossy().to_string()).map(|n| n.id); }
             self.index.files.get(&p.to_string_lossy().to_string()).map(|n| n.id)
        };
        if let Some(id) = check_path(candidate_base.clone()) { return Some(id); }
        for ext in extensions { if let Some(id) = check_path(candidate_base.with_extension(ext)) { return Some(id); } }
        let index_base = candidate_base.join("index");
        for ext in extensions { if let Some(id) = check_path(index_base.with_extension(ext)) { return Some(id); } }
        None
    }

    fn resolve_implicit_connections(&mut self) {
        self.index.file_dependencies.clear(); self.index.inheritance.clear();
        let raw_impls: Vec<(SymbolId, Vec<String>)> = self.index.raw_implementations.iter().map(|(k, v)| (*k, v.clone())).collect();
        for (child_id, parent_names) in raw_impls {
            for parent_name in parent_names {
                if let Some(parent_ids) = self.index.symbol_map.get(&parent_name) {
                    for &parent_id in parent_ids { self.index.inheritance.entry(parent_id).or_default().push(child_id); }
                }
            }
        }
        let mut global_string_map: HashMap<String, Vec<FileId>> = HashMap::new();
        let entries: Vec<(FileId, Vec<String>)> = self.index.raw_literals.iter().map(|(k, v)| (*k, v.clone())).collect();
        for (src_id, literals) in entries {
            for lit in literals {
                if let Some(target_id) = self.resolve_import_path(src_id, &lit) {
                    if src_id != target_id { self.index.file_dependencies.entry(src_id).or_default().push(target_id); }
                }
                let is_path = lit.contains('/') || lit.contains('.');
                let is_constant = lit.len() > 5 && lit.chars().all(|c| c.is_uppercase() || c == '_');
                if is_path || is_constant { global_string_map.entry(lit.clone()).or_default().push(src_id); }
            }
        }
        for (_, file_ids) in global_string_map {
            if file_ids.len() < 2 || file_ids.len() > 50 { continue; }
            for &f1 in &file_ids { for &f2 in &file_ids { if f1 != f2 { self.index.file_dependencies.entry(f1).or_default().push(f2); } } }
        }
        for deps in self.index.file_dependencies.values_mut() { deps.sort(); deps.dedup(); }
        for children in self.index.inheritance.values_mut() { children.sort(); children.dedup(); }
    }
}