use std::collections::{HashMap, HashSet};
use std::path::Path;
use crate::models::{FileId, SymbolId, SymbolKind, EdgeKind, WorkspaceIndex, StagingArea, SymbolIndex};
use crate::resolution::resolvers::{core, add_edge, link_modules, constants};
use crate::topic::matches_topic;
use crate::analysis::language::LanguageConfig;

pub fn resolve_state_management(index: &mut WorkspaceIndex, staging: &StagingArea) {
    let mut action_map: HashMap<String, Vec<SymbolId>> = HashMap::new();
    for (handler_id, actions) in &staging.raw_action_handlers {
        for action in actions {
            action_map.entry(action.clone()).or_default().push(*handler_id);
        }
    }

    let mut new_links = Vec::new();
    for (dispatch_id, actions) in &staging.raw_action_dispatches {
        for action in actions {
            if let Some(handler_ids) = action_map.get(action) {
                for &target_id in handler_ids {
                    if *dispatch_id != target_id { new_links.push((*dispatch_id, target_id)); }
                }
            }
        }
    }

    for (src, tgt) in new_links {
        add_edge(index, src, tgt, EdgeKind::Dispatches);
        add_edge(index, src, tgt, EdgeKind::Calls);
    }
}

pub fn resolve_pubsub_wildcards(
    index: &mut WorkspaceIndex,
    staging: &StagingArea,
    configs: &HashMap<String, LanguageConfig>
) {
    let mut patterns: Vec<(FileId, String)> = Vec::new();
    let mut candidates: Vec<(FileId, String)> = Vec::new();
    let config_file_ids: HashSet<FileId> = index.files.iter()
        .filter(|(path, _)| {
            let p = path.to_lowercase();
            let ext = Path::new(&p).extension().and_then(|s| s.to_str()).unwrap_or("");
            if let Some(config) = configs.get(ext) {
                config.heuristics.project_config_files.iter().any(|&f| p.ends_with(f))
            } else {
                p.ends_with("tsconfig.json") || p.ends_with("package.json")
            }
        })
        .map(|(_, node)| node.id).collect();

    for (file_id, literals) in &staging.raw_literals {
        if config_file_ids.contains(file_id) { continue; }
        for lit in literals {
            if lit.len() < 3 || !lit.contains(constants::PATH_SEPARATORS) { continue; }
            let clean = lit.trim_matches(constants::QUOTE_CHARS).to_string();
            if clean.contains(constants::WILDCARD_CHARS) {
                patterns.push((*file_id, clean));
            } else {
                candidates.push((*file_id, clean));
            }
        }
    }

    for (pat_file, pat_str) in &patterns {
        for (cand_file, cand_str) in &candidates {
            if pat_file == cand_file { continue; }
            if matches_topic(pat_str, cand_str) {
                let deps = index.file_dependencies.entry(*cand_file).or_default();
                if !deps.contains(pat_file) { deps.push(*pat_file); }
                link_modules(index, *pat_file, *cand_file);
            }
        }
    }
}

pub fn resolve_magic_proxies(
    index: &mut WorkspaceIndex,
    staging: &StagingArea,
    lookup: &SymbolIndex,
    configs: &HashMap<String, LanguageConfig>
) {
    let mut new_links = Vec::new();

    for (caller_id, receiver_map) in &staging.fingerprints {
        for (receiver_var, methods) in receiver_map {
            let mut type_class_id = None;
            let mut curr_scope = Some(*caller_id);
            
            // 1. Scope Walk (Find the variable type)
            while let Some(sid) = curr_scope {
                if let Some(vars) = staging.local_variable_types.get(&sid) {
                    if let Some(type_name) = vars.get(receiver_var) {
                         let clean = type_name.split('<').next().unwrap().trim();
                         if let Some(ids) = lookup.symbol_map.get(clean) {
                             type_class_id = ids.iter().find(|&&id| {
                                 let s = &index.symbols[&id];
                                 s.kind == SymbolKind::Container
                             }).cloned();
                         }
                        break;
                    }
                }
                curr_scope = index.symbols.get(&sid).and_then(|s| s.parent_id);
            }

            // 2. Check Magic Methods
            if let Some(class_id) = type_class_id {
                 let file_id = index.symbols[&class_id].file_id;
                 let file_path = &index.files.values().find(|f| f.id == file_id).unwrap().path;
                 let ext = Path::new(file_path).extension().and_then(|s| s.to_str()).unwrap_or("");
                 
                 if let Some(config) = configs.get(ext) {
                    // CHANGE 1: Access via .heuristics
                    if !config.heuristics.magic_methods.is_empty() {
                        let has_explicit = if let Some(explicit_methods) = staging.container_methods.get(&class_id) {
                            methods.iter().any(|m| explicit_methods.contains(m))
                        } else { false };

                        if !has_explicit {
                            if let Some(class_members) = staging.container_methods.get(&class_id) {
                                // CHANGE 2: Access via .heuristics
                                for &magic_name in config.heuristics.magic_methods {
                                    if class_members.contains(magic_name) {
                                        if let Some(candidates) = lookup.symbol_map.get(magic_name) {
                                            for &magic_id in candidates {
                                                if index.symbols[&magic_id].parent_id == Some(class_id) {
                                                    new_links.push((*caller_id, magic_id));
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
        add_edge(index, src, tgt, EdgeKind::Calls);
    }
}

pub fn resolve_decorators(
    index: &mut WorkspaceIndex,
    staging: &StagingArea,
    lookup: &SymbolIndex,
    cache: &mut core::ResolutionCache
) {
    for (caller_id, dec_names) in &staging.raw_decorators {
        let caller_file_id = index.symbols[caller_id].file_id;
        for dec_name in dec_names {
            let clean = dec_name.split('(').next().unwrap_or(&dec_name).trim().trim_matches(constants::QUOTE_CHARS);
            if let Some(target_id) = core::resolve_single_call(index, lookup, cache, caller_file_id, clean) {
                add_edge(index, *caller_id, target_id, EdgeKind::Calls); 
            } else if let Some(candidates) = lookup.symbol_map.get(clean) {
                for g in candidates { add_edge(index, *caller_id, *g, EdgeKind::Calls); }
            }
        }
    }
}