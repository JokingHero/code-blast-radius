use std::collections::{ HashMap, HashSet };
use std::path::PathBuf;
use crate::models::{
    FileId,
    SymbolId,
    SymbolKind,
    EdgeKind,
    WorkspaceIndex,
    StagingArea,
    SymbolIndex,
};
use crate::resolution::resolvers::{ core, add_edge, link_modules, constants };

pub fn resolve_database_references(index: &mut WorkspaceIndex, staging: &StagingArea) {
    // 1. Build Schema Map
    let mut schema_map: HashMap<String, SymbolId> = HashMap::new();
    for (id, sym) in &index.symbols {
        let file_opt = index.files.values().find(|f| f.id == sym.file_id);
        if let Some(file) = file_opt {
            if file.relative_path.ends_with(".sql") || file.relative_path.ends_with(".prisma") {
                schema_map.insert(sym.name.clone(), *id);
            }
        }
    }
    let mut new_links = Vec::new();

    // 2. Resolve via Literals (No Clone!)
    for (file_id, literals) in &staging.raw_literals {
        let module_sym_id = index.symbols
            .values()
            .find(|s| s.file_id == *file_id && s.kind == SymbolKind::Module)
            .map(|s| s.id);

        if let Some(mod_id) = module_sym_id {
            for lit in literals {
                let clean_lit = lit.trim_matches(constants::QUOTE_CHARS);
                let words: Vec<&str> = clean_lit
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                    .collect();
                for (table_name, &table_sym_id) in &schema_map {
                    if words.iter().any(|&w| w == table_name) {
                        new_links.push((mod_id, table_sym_id));
                    }
                }
            }
        }
    }

    // 3. Resolve via Fingerprints (No Clone!)
    for (func_id, prints) in &staging.fingerprints {
        for (receiver, _) in prints {
            for (table_name, &table_sym_id) in &schema_map {
                let receiver_lower = receiver.to_lowercase();
                let table_lower = table_name.to_lowercase();
                if
                    receiver == table_name ||
                    receiver.ends_with(&format!(".{}", table_name)) ||
                    receiver_lower == table_lower ||
                    receiver_lower.ends_with(&format!(".{}", table_lower))
                {
                    new_links.push((*func_id, table_sym_id));
                }
            }
        }
    }

    for (src, tgt) in new_links {
        add_edge(index, src, tgt, EdgeKind::TypeReference);
    }
}

pub fn resolve_config_links(
    index: &mut WorkspaceIndex,
    staging: &StagingArea,
    lookup: &SymbolIndex
) {
    let mut new_links = Vec::new();
    for (&sym_id, used_keys) in &staging.symbol_config_refs {
        for key in used_keys {
            if let Some(def_ids) = lookup.config_definitions.get(key) {
                for &target_sid in def_ids {
                    new_links.push((sym_id, target_sid));
                }
            }
        }
    }
    for (caller, target) in new_links {
        add_edge(index, caller, target, EdgeKind::Configures);
    }
}

pub fn resolve_file_dependencies(
    index: &mut WorkspaceIndex,
    lookup: &SymbolIndex,
    path_map: &HashMap<PathBuf, FileId>, 
    id_map: &HashMap<FileId, PathBuf>,
    active_roots: &Vec<std::path::PathBuf>,
) {
    let mut file_to_module_sym: HashMap<FileId, SymbolId> = HashMap::new();
    for sym in index.symbols.values() {
        if sym.kind == SymbolKind::Module {
            file_to_module_sym.insert(sym.file_id, sym.id);
        }
    }

    for (file_id, imports) in &lookup.file_imports {
        let mut deps = HashSet::new();
        let src_module_id = file_to_module_sym.get(file_id).cloned();

        for imp in imports {
            // Pass path_map
            if
                let Some(target_fid) = core::resolve_import_path(
                    index,
                    lookup,
                    path_map,
                    id_map,
                    active_roots,
                    *file_id,
                    &imp.source
                )
            {
                deps.insert(target_fid);
                if
                    let (Some(src_id), Some(tgt_id)) = (
                        src_module_id,
                        file_to_module_sym.get(&target_fid),
                    )
                {
                    add_edge(index, src_id, *tgt_id, EdgeKind::Imports);
                }
            }
        }
        if !deps.is_empty() {
            index.file_dependencies.insert(*file_id, deps.into_iter().collect());
        }
    }
}

pub fn resolve_namespace_imports(
    index: &mut WorkspaceIndex,
    staging: &mut StagingArea,
    lookup: &SymbolIndex,
    path_map: &HashMap<PathBuf, FileId>,
    id_map: &HashMap<FileId, PathBuf>,
    active_roots: &Vec<std::path::PathBuf>
) {
    let file_mod_map: HashMap<FileId, String> = index.symbols
        .values()
        .filter(|s| s.kind == SymbolKind::Module)
        .map(|s| (s.file_id, s.name.clone()))
        .collect();
    for (file_id, imports) in &lookup.file_imports {
        let mod_sym_id = index.symbols
            .values()
            .find(|s| s.file_id == *file_id && s.kind == SymbolKind::Module)
            .map(|s| s.id);

        if let Some(scope_id) = mod_sym_id {
            for imp in imports {
                if imp.name == "*" {
                    if let Some(alias) = &imp.alias {
                        if
                            let Some(target_fid) = core::resolve_import_path(
                                index,
                                lookup,
                                path_map,
                                id_map,
                                active_roots,
                                *file_id,
                                &imp.source
                            )
                        {
                            if let Some(target_mod_name) = file_mod_map.get(&target_fid) {
                                staging.local_variable_types
                                    .entry(scope_id)
                                    .or_default()
                                    .insert(alias.clone(), target_mod_name.clone());
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn resolve_literal_dependencies(
    index: &mut WorkspaceIndex,
    staging: &StagingArea,
    lookup: &SymbolIndex,
    path_map: &HashMap<PathBuf, FileId>, 
    id_map: &HashMap<FileId, PathBuf>,
    active_roots: &Vec<std::path::PathBuf>
) {
    let mut potential_links: Vec<(FileId, String)> = Vec::new();
    let config_file_ids: HashSet<FileId> = index.files
        .iter()
        .filter(|(_, node)| {
            let p = node.relative_path.to_lowercase();
            p.ends_with("json") || p.ends_with(".env")
        })
        .map(|(_, node)| node.id)
        .collect();
    for (file_id, literals) in &staging.raw_literals {
        if config_file_ids.contains(file_id) {
            continue;
        }
        for lit in literals {
            if
                (lit.contains('/') || lit.contains('.')) &&
                !lit.contains(' ') &&
                !lit.contains('\n') &&
                lit.len() > 3
            {
                potential_links.push((*file_id, lit.clone()));
            }
        }
    }

    for (src_id, literal) in potential_links {
        if
            let Some(target_id) = core::resolve_import_path(
                index,
                lookup,
                path_map,
                id_map,
                active_roots,
                src_id,
                &literal
            )
        {
            if src_id != target_id {
                let deps = index.file_dependencies.entry(src_id).or_default();
                if !deps.contains(&target_id) {
                    deps.push(target_id);
                }
                link_modules(index, lookup, src_id, target_id);
            }
        }
    }
}

pub fn resolve_shared_literals(
    index: &mut WorkspaceIndex,
    staging: &StagingArea,
    lookup: &SymbolIndex
) {
    let mut literal_map: HashMap<String, Vec<FileId>> = HashMap::new();
    let config_file_ids: HashSet<FileId> = index.files
        .iter()
        .filter(|(_, node)| {
            let p = node.relative_path.to_lowercase();
            p.ends_with("json") || p.ends_with(".env")
        })
        .map(|(_, node)| node.id)
        .collect();

    for (file_id, literals) in &staging.raw_literals {
        if config_file_ids.contains(file_id) {
            continue;
        }
        for lit in literals {
            if lit.starts_with("./") || lit.starts_with("../") || lit.starts_with("@") {
                continue;
            }
            let is_route = lit.starts_with('/');
            let is_long =
                lit.len() > 10 &&
                !lit.contains(' ') &&
                (lit.contains('_') || lit.contains('-') || lit.contains('.'));

            if (is_route || is_long) && lit.len() > 3 {
                literal_map.entry(lit.clone()).or_default().push(*file_id);
            }
        }
    }

    for (_lit, file_ids) in literal_map {
        if file_ids.len() < 2 {
            continue;
        }
        for i in 0..file_ids.len() {
            for j in i + 1..file_ids.len() {
                let id_a = file_ids[i];
                let id_b = file_ids[j];

                let deps_a = index.file_dependencies.entry(id_a).or_default();
                if !deps_a.contains(&id_b) {
                    deps_a.push(id_b);
                }
                let deps_b = index.file_dependencies.entry(id_b).or_default();
                if !deps_b.contains(&id_a) {
                    deps_b.push(id_a);
                }

                link_modules(index, lookup, id_a, id_b);
            }
        }
    }
}

pub fn resolve_iac_links(index: &mut WorkspaceIndex, staging: &StagingArea, lookup: &SymbolIndex) {
    let mut new_file_links = Vec::new();
    let mut env_var_definitions: HashMap<String, Vec<FileId>> = HashMap::new();

    for (file_id, literals) in &staging.raw_literals {
        for lit in literals {
            let parts = lit.split(|c: char| !c.is_alphanumeric() && c != '_');
            for part in parts {
                if
                    part.len() > 3 &&
                    part.chars().all(|c| c.is_uppercase() || c.is_numeric() || c == '_') &&
                    part.contains('_')
                {
                    let entry = env_var_definitions.entry(part.to_string()).or_default();
                    if !entry.contains(file_id) {
                        entry.push(*file_id);
                    }
                }
            }
        }
    }

    for (sym_id, config_keys) in &staging.symbol_config_refs {
        let sym_file_id = index.symbols.get(sym_id).map(|s| s.file_id);

        if let Some(file_id) = sym_file_id {
            for key in config_keys {
                if let Some(def_files) = env_var_definitions.get(key) {
                    for &def_file_id in def_files {
                        if file_id != def_file_id {
                            new_file_links.push((file_id, def_file_id));
                        }
                    }
                }
            }
        }
    }

    let mut aws_s3_users = Vec::new();
    for (file_id, imports) in &lookup.file_imports {
        for imp in imports {
            if imp.source.contains("aws-sdk") || imp.source.contains("boto3") {
                aws_s3_users.push(*file_id);
            }
        }
    }

    let mut s3_definers = Vec::new();
    for sym in index.symbols.values() {
        if sym.kind == SymbolKind::Resource && sym.name.contains("aws_s3_bucket") {
            s3_definers.push(sym.file_id);
        }
    }

    for user_id in &aws_s3_users {
        for def_id in &s3_definers {
            if user_id != def_id {
                new_file_links.push((*user_id, *def_id));
            }
        }
    }

    for (src, tgt) in new_file_links {
        let deps = index.file_dependencies.entry(src).or_default();
        if !deps.contains(&tgt) {
            deps.push(tgt);
        }
        link_modules(index, lookup, src, tgt);
    }
}