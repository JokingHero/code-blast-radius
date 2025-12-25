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
    /// Transient cache used during resolution to avoid redundant barrel walks.
    /// Key: (FileId where symbol is requested, Symbol Name)
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
        let mut s = Self::new();
        s.index = index;
        Ok(s)
    }

    fn remove_file(&mut self, path_key: &str) {
        if let Some(node) = self.index.files.remove(path_key) {
            self.clear_file_symbols(node.id);
            self.index.file_dependencies.remove(&node.id);
        }
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
                    if id_list.is_empty() { self.index.symbol_map.remove(&sym.name); }
                }
            }
            self.index.raw_calls.remove(&sym_id);
            self.index.raw_implementations.remove(&sym_id);
            self.index.resolved_calls.remove(&sym_id);
            self.index.fingerprints.remove(&sym_id);
            self.index.container_methods.remove(&sym_id);
            self.index.local_variable_types.remove(&sym_id);
            self.index.inheritance.remove(&sym_id);
        }
        self.index.file_imports.remove(&file_id);
        self.index.file_exports.remove(&file_id);
        self.index.raw_literals.remove(&file_id);
    }

    pub fn scan(&mut self, root: &Path) {
        let root_abs = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        let mut seen_paths = HashSet::new();

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

        let to_remove: Vec<String> = self.index.files.keys()
            .filter(|path_key| {
                let p = Path::new(path_key);
                p.starts_with(&root_abs) && !seen_paths.contains(*path_key)
            })
            .cloned()
            .collect();

        for path_key in to_remove {
            self.remove_file(&path_key);
        }
    }

    fn update_file(&mut self, path_key: &str, path_obj: &Path, content: &str, hash: [u8; 32], config: &LanguageConfig) {
        let existing_id = self.index.files.get(path_key).map(|node| node.id);
        let file_id = existing_id.unwrap_or_else(|| self.index.files.len() as u32);

        self.clear_file_symbols(file_id);
        self.index.files.insert(path_key.to_string(), FileNode { id: file_id, path: path_key.to_string(), hash });

        if let Ok(analysis) = analyze_source(path_obj, content, config) {
            if !analysis.imports.is_empty() { self.index.file_imports.insert(file_id, analysis.imports); }
            if !analysis.exports.is_empty() { self.index.file_exports.insert(file_id, analysis.exports); }
            if !analysis.literals.is_empty() { self.index.raw_literals.insert(file_id, analysis.literals); }

            let mut file_symbol_ids = Vec::new();
            for func in analysis.functions {
                let symbol_id = self.index.symbols.keys().max().map_or(0, |k| k + 1);
                let is_container = func.source_code.contains("class ") || func.source_code.contains("interface ");

                self.index.symbols.insert(symbol_id, SymbolNode {
                    id: symbol_id, file_id, parent_id: None,
                    name: func.name.clone(), kind: if is_container { "container" } else { "function" }.to_string(), 
                    range_start: func.range_start, range_end: func.range_end,
                    doc_comment: func.documentation,
                    return_type: func.return_type,
                });
                
                file_symbol_ids.push(symbol_id);
                self.index.symbol_map.entry(func.name.clone()).or_default().push(symbol_id);
                if !func.calls.is_empty() { self.index.raw_calls.insert(symbol_id, func.calls); }
                if !func.fingerprints.is_empty() { self.index.fingerprints.insert(symbol_id, func.fingerprints); }
                
                let mut vars = func.local_types.clone();
                for (v, f) in func.local_assigns { vars.insert(v, format!("returns:{}", f)); }
                if !vars.is_empty() { self.index.local_variable_types.insert(symbol_id, vars); }
            }

            let container_ids: Vec<SymbolId> = file_symbol_ids.iter()
                .filter(|&&id| self.index.symbols.get(&id).map_or(false, |s| s.kind == "container"))
                .cloned()
                .collect();

            for c_id in container_ids {
                let (cs, ce) = {
                    let c = &self.index.symbols[&c_id];
                    (c.range_start, c.range_end)
                };
                
                let mut members = HashSet::new();
                for &s_id in &file_symbol_ids {
                    if s_id == c_id { continue; }
                    let is_member = {
                        let s = &self.index.symbols[&s_id];
                        s.range_start >= cs && s.range_end <= ce
                    };
                    if is_member {
                        members.insert(self.index.symbols[&s_id].name.clone());
                        if let Some(node) = self.index.symbols.get_mut(&s_id) {
                            node.parent_id = Some(c_id);
                        }
                    }
                }
                if !members.is_empty() { self.index.container_methods.insert(c_id, members); }
            }

            for (child, parent) in analysis.implementations {
                if let Some(ids) = self.index.symbol_map.get(&child) {
                    if let Some(&cid) = ids.iter().find(|&&id| self.index.symbols[&id].file_id == file_id) {
                        self.index.raw_implementations.entry(cid).or_default().push(parent);
                    }
                }
            }
        }
    }
    
    pub fn resolve_references(&mut self) {
        self.index.resolved_calls.clear();
        self.index.inheritance.clear();
        self.resolution_cache.clear();

        self.resolve_type_sniffing();
        self.resolve_fingerprints();
        self.resolve_implicit_connections();
        self.resolve_function_calls_with_fallback();
        self.resolve_bare_imports();
    }

    fn resolve_symbol_across_barrels(
        &mut self, 
        target_file_id: FileId, 
        symbol_name: &str, 
        visited: &mut HashSet<FileId>
    ) -> Option<SymbolId> {
        // 1. Cycle Detection
        if visited.contains(&target_file_id) { return None; }
        visited.insert(target_file_id);

        // 2. Cache Lookup
        if let Some(&cached_res) = self.resolution_cache.get(&(target_file_id, symbol_name.to_string())) {
            return cached_res;
        }

        let mut result = None;

        // 3. Local Definition Check
        if let Some(ids) = self.index.symbol_map.get(symbol_name) {
            if let Some(&id) = ids.iter().find(|&&id| self.index.symbols[&id].file_id == target_file_id) {
                result = Some(id);
            }
        }

        // 4. Re-export Walk
        if result.is_none() {
            if let Some(exports) = self.index.file_exports.get(&target_file_id).cloned() {
                // Named exports first
                for exp in exports.iter().filter(|e| e.name.as_deref() == Some(symbol_name)) {
                    if let Some(next_file_id) = self.resolve_import_path(target_file_id, &exp.source) {
                        result = self.resolve_symbol_across_barrels(next_file_id, symbol_name, visited);
                        if result.is_some() { break; }
                    }
                }

                // Wildcard exports second
                if result.is_none() {
                    for exp in exports.iter().filter(|e| e.name.is_none()) {
                        if let Some(next_file_id) = self.resolve_import_path(target_file_id, &exp.source) {
                            result = self.resolve_symbol_across_barrels(next_file_id, symbol_name, visited);
                            if result.is_some() { break; }
                        }
                    }
                }
            }
        }

        // 5. Update Cache
        self.resolution_cache.insert((target_file_id, symbol_name.to_string()), result);
        result
    }

    fn resolve_function_calls_with_fallback(&mut self) {
        // Clone entries to decouple the immutable borrow of self.index for the loop
        // from the mutable borrow of self for resolve_single_call
        let entries: Vec<(SymbolId, Vec<String>)> = self.index.raw_calls.iter()
            .map(|(k, v)| (*k, v.clone())).collect();

        for (caller_id, called_names) in entries {
            let caller_file_id = self.index.symbols[&caller_id].file_id;
            
            for name in called_names {
                let already_resolved = if let Some(resolved) = self.index.resolved_calls.get(&caller_id) {
                    resolved.iter().any(|&rid| self.index.symbols[&rid].name == name)
                } else { false };

                if !already_resolved {
                    if let Some(tid) = self.resolve_single_call(caller_file_id, &name) {
                        self.index.resolved_calls.entry(caller_id).or_default().push(tid);
                    } 
                    else if let Some(candidates) = self.index.symbol_map.get(&name) {
                        let mut guesses = candidates.clone();
                        self.index.resolved_calls.entry(caller_id).or_default().append(&mut guesses);
                    }
                }
            }
            if let Some(resolved) = self.index.resolved_calls.get_mut(&caller_id) {
                resolved.sort();
                resolved.dedup();
            }
        }
    }

    fn resolve_type_sniffing(&mut self) {
        let mut new_links = Vec::new();
        for (&caller_id, receiver_map) in &self.index.fingerprints {
            for (receiver, methods) in receiver_map {
                let type_hint = self.index.local_variable_types.get(&caller_id).and_then(|v| v.get(receiver));
                if let Some(hint) = type_hint {
                    let mut resolved_type = None;
                    if hint.starts_with("returns:") {
                        if let Some(targets) = self.index.symbol_map.get(&hint[8..]) {
                            resolved_type = self.index.symbols.get(&targets[0]).and_then(|s| s.return_type.clone());
                        }
                    } else { resolved_type = Some(hint.clone()); }

                    if let Some(tn) = resolved_type {
                        let clean = tn.split('<').next().unwrap().to_string();
                        if let Some(type_ids) = self.index.symbol_map.get(&clean) {
                            for &tid in type_ids {
                                new_links.push((caller_id, tid));
                                if let Some(mems) = self.index.container_methods.get(&tid) {
                                    for m in methods {
                                        if mems.contains(m) {
                                            if let Some(mids) = self.index.symbol_map.get(m) {
                                                new_links.extend(mids.iter().filter(|&&mid| self.index.symbols[&mid].parent_id == Some(tid)).map(|&mid| (caller_id, mid)));
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
        for (c, t) in new_links { self.index.resolved_calls.entry(c).or_default().push(t); }
    }

    fn resolve_fingerprints(&mut self) {
        let mut links = Vec::new();
        for (&cid, fprints) in &self.index.fingerprints {
            for (_, meths) in fprints {
                for (&cont_id, cont_meths) in &self.index.container_methods {
                    if meths.iter().all(|m| cont_meths.contains(m)) {
                        links.push((cid, cont_id));
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

    fn resolve_bare_imports(&mut self) {
        let ids: Vec<FileId> = self.index.file_imports.keys().cloned().collect();
        for fid in ids {
            for imp in self.index.file_imports[&fid].clone() {
                if imp.name.is_empty() {
                    if let Some(tid) = self.resolve_import_path(fid, &imp.source) {
                        self.index.file_dependencies.entry(fid).or_default().push(tid);
                    }
                }
            }
        }
    }

    fn resolve_single_call(&mut self, file_id: FileId, name: &str) -> Option<SymbolId> {
        if let Some(ids) = self.index.symbol_map.get(name) {
            if let Some(&id) = ids.iter().find(|&&id| self.index.symbols[&id].file_id == file_id) { return Some(id); }
        }

        if let Some(imps) = self.index.file_imports.get(&file_id).cloned() {
            for imp in imps {
                if imp.name == name {
                    if let Some(tfid) = self.resolve_import_path(file_id, &imp.source) {
                        let mut visited = HashSet::new();
                        return self.resolve_symbol_across_barrels(tfid, name, &mut visited);
                    }
                }
            }
        }
        None
    }

    fn resolve_import_path(&self, from_id: FileId, source: &str) -> Option<FileId> {
        let from_path_str = &self.index.files.values().find(|f| f.id == from_id)?.path;
        let from_path = Path::new(from_path_str);
        let parent = from_path.parent()?;
        let exts = ["ts", "js", "tsx", "rs", "py"];
        
        let base = parent.join(source);
        let check = |p: PathBuf| self.index.files.get(&p.to_string_lossy().to_string()).map(|n| n.id);
        
        if let Some(id) = check(base.clone()) { return Some(id); }
        for e in exts { 
            if let Some(id) = check(base.with_extension(e)) { return Some(id); } 
            if let Some(id) = check(base.join(format!("index.{}", e))) { return Some(id); }
        }
        None
    }

    fn resolve_implicit_connections(&mut self) {
        self.index.inheritance.clear();
        for (cid, parents) in self.index.raw_implementations.clone() {
            for p in parents {
                if let Some(pids) = self.index.symbol_map.get(&p) {
                    for &pid in pids { self.index.inheritance.entry(pid).or_default().push(cid); }
                }
            }
        }
    }
}