use std::collections::{HashMap, HashSet};
use std::path::Path;
use crate::resolution::Indexer;
use crate::models::{FileId, SymbolId, SymbolKind};
use crate::topic::matches_topic;

impl Indexer {
    pub(crate) fn resolve_state_management(&mut self) {
        let mut action_map: HashMap<String, Vec<SymbolId>> = HashMap::new();
        for (handler_id, actions) in &self.index.raw_action_handlers {
            for action in actions {
                action_map.entry(action.clone()).or_default().push(*handler_id);
            }
        }

        let mut new_links = Vec::new();
        for (dispatch_id, actions) in &self.index.raw_action_dispatches {
            for action in actions {
                if let Some(handler_ids) = action_map.get(action) {
                    for &target_id in handler_ids {
                        if *dispatch_id != target_id { new_links.push((*dispatch_id, target_id)); }
                    }
                }
            }
        }

        for (src, tgt) in new_links {
            let calls = self.index.resolved_calls.entry(src).or_default();
            if !calls.contains(&tgt) { calls.push(tgt); }
        }
    }

    pub(crate) fn resolve_pubsub_wildcards(&mut self) {
        let mut patterns: Vec<(FileId, String)> = Vec::new();
        let mut candidates: Vec<(FileId, String)> = Vec::new();
        
        let config_file_ids: HashSet<FileId> = self.index.files.iter()
            .filter(|(path, _)| { let p = path.to_lowercase(); p.ends_with("tsconfig.json") || p.ends_with("package.json") })
            .map(|(_, node)| node.id).collect();

        for (&file_id, literals) in &self.index.raw_literals {
            if config_file_ids.contains(&file_id) { continue; }
            for lit in literals {
                if lit.len() < 3 || (!lit.contains('.') && !lit.contains('/') && !lit.contains(':')) { continue; }
                let clean = lit.trim_matches(|c| c == '"' || c == '\'' || c == '`').to_string();
                
                if clean.contains('*') || clean.contains('#') || clean.contains('>') {
                    patterns.push((file_id, clean));
                } else {
                    candidates.push((file_id, clean));
                }
            }
        }

        for (pat_file, pat_str) in &patterns {
            for (cand_file, cand_str) in &candidates {
                if pat_file == cand_file { continue; }
                if matches_topic(pat_str, cand_str) {
                    let deps_a = self.index.file_dependencies.entry(*pat_file).or_default();
                    if !deps_a.contains(cand_file) { deps_a.push(*cand_file); }
                    let deps_b = self.index.file_dependencies.entry(*cand_file).or_default();
                    if !deps_b.contains(pat_file) { deps_b.push(*pat_file); }
                    
                    self.link_modules(*pat_file, *cand_file);
                }
            }
        }
    }

    pub(crate) fn resolve_magic_proxies(&mut self) {
        let mut new_links = Vec::new();
        for (&caller_id, receiver_map) in &self.index.fingerprints {
            for (receiver_var, methods) in receiver_map {
                let mut type_class_id = None;
                // Reuse scope walking
                let mut curr_scope = Some(caller_id);
                while let Some(sid) = curr_scope {
                    if let Some(vars) = self.index.local_variable_types.get(&sid) {
                        if let Some(type_name) = vars.get(receiver_var) {
                             let clean = type_name.split('<').next().unwrap().trim();
                             if let Some(ids) = self.index.symbol_map.get(clean) {
                                 type_class_id = ids.iter().find(|&&id| {
                                     let s = &self.index.symbols[&id];
                                     s.kind == SymbolKind::Container
                                 }).cloned();
                             }
                            break;
                        }
                    }
                    curr_scope = self.index.symbols.get(&sid).and_then(|s| s.parent_id);
                }

                if let Some(class_id) = type_class_id {
                    // Logic inline to avoid pub method exposure for now
                     let file_id = self.index.symbols[&class_id].file_id;
                     let file_path = &self.index.files.values().find(|f| f.id == file_id).unwrap().path;
                     let ext = Path::new(file_path).extension().and_then(|s| s.to_str()).unwrap_or("");
                     
                     if let Some(config) = self.configs.get(ext) {
                        if !config.magic_methods.is_empty() {
                            let explicit_methods = self.index.container_methods.get(&class_id);
                            for method_name in methods {
                                let is_explicit = explicit_methods.map_or(false, |s| s.contains(method_name));
                                if !is_explicit {
                                    if let Some(class_members) = self.index.container_methods.get(&class_id) {
                                        for &magic_name in config.magic_methods {
                                            if class_members.contains(magic_name) {
                                                if let Some(candidates) = self.index.symbol_map.get(magic_name) {
                                                    for &magic_id in candidates {
                                                        if self.index.symbols[&magic_id].parent_id == Some(class_id) {
                                                            new_links.push((caller_id, magic_id));
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
                }
            }
        }
        for (src, tgt) in new_links {
            let calls = self.index.resolved_calls.entry(src).or_default();
            if !calls.contains(&tgt) { calls.push(tgt); }
        }
    }

    pub(crate) fn resolve_decorators(&mut self) {
        let entries: Vec<_> = self.index.raw_decorators.iter().map(|(k, v)| (*k, v.clone())).collect();
        for (caller_id, dec_names) in entries {
            let caller_file_id = self.index.symbols[&caller_id].file_id;
            for dec_name in dec_names {
                let clean = dec_name.split('(').next().unwrap_or(&dec_name).trim();
                if let Some(target_id) = self.resolve_single_call(caller_file_id, clean) {
                    self.index.resolved_calls.entry(caller_id).or_default().push(target_id);
                } else if let Some(candidates) = self.index.symbol_map.get(clean) {
                    let mut guesses = candidates.clone();
                    self.index.resolved_calls.entry(caller_id).or_default().append(&mut guesses);
                }
            }
            if let Some(resolved) = self.index.resolved_calls.get_mut(&caller_id) {
                resolved.sort();
                resolved.dedup();
            }
        }
    }
}