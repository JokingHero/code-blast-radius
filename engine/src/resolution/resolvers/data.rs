use std::collections::{ HashMap, HashSet };
use crate::resolution::Indexer;
use crate::models::{ FileId, SymbolId, SymbolKind, EdgeKind };
impl Indexer {
    pub(crate) fn resolve_database_references(&mut self) {
        // 1. Build Schema Map
        let mut schema_map: HashMap<String, SymbolId> = HashMap::new();
        // We can iterate directly because we aren't mutating self inside this loop
        for (id, sym) in &self.index.symbols {
            let file_opt = self.index.files.values().find(|f| f.id == sym.file_id);
            if let Some(file) = file_opt {
                if file.path.ends_with(".sql") || file.path.ends_with(".prisma") {
                    schema_map.insert(sym.name.clone(), *id);
                }
            }
        }
        let mut new_links = Vec::new();

        // 2. Resolve via Literals
        // Snapshot: Collect data to avoid holding borrow on self.staging.raw_literals
        let literals_snapshot: Vec<(FileId, Vec<String>)> = self.staging.raw_literals
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();

        for (file_id, literals) in literals_snapshot {
            let module_sym_id = self.index.symbols
                .values()
                .find(|s| s.file_id == file_id && s.kind == SymbolKind::Module)
                .map(|s| s.id);

            if let Some(mod_id) = module_sym_id {
                for lit in literals {
                    let clean_lit = lit.trim_matches(|c| c == '"' || c == '\'' || c == '`');
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

        // 3. Resolve via Fingerprints
        // We iterate fingerprints from staging
        for (func_id, prints) in &self.staging.fingerprints {
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

        // 4. Apply Mutations
        for (src, tgt) in new_links {
            self.add_edge(src, tgt, EdgeKind::TypeReference);
        }
    }

    pub(crate) fn resolve_config_links(&mut self) {
        let mut new_links = Vec::new();
        // Collect edges to add from staging
        for (&sym_id, used_keys) in &self.staging.symbol_config_refs {
            for key in used_keys {
                if let Some(def_ids) = self.lookup.config_definitions.get(key) {
                    for &target_sid in def_ids {
                        new_links.push((sym_id, target_sid));
                    }
                }
            }
        }
        // Apply edges
        for (caller, target) in new_links {
            self.add_edge(caller, target, EdgeKind::Configures);
        }
    }

    pub(crate) fn resolve_file_dependencies(&mut self) {
        let mut file_to_module_sym: HashMap<FileId, SymbolId> = HashMap::new();
        for sym in self.index.symbols.values() {
            if sym.kind == SymbolKind::Module {
                file_to_module_sym.insert(sym.file_id, sym.id);
            }
        }

        let fids: Vec<FileId> = self.lookup.file_imports.keys().cloned().collect();

        for fid in fids {
            let mut deps = HashSet::new();
            let src_module_id = file_to_module_sym.get(&fid).cloned();

            // FIX: Clone imports to end borrow on self.index.lookup
            let imports = self.lookup.file_imports.get(&fid).cloned().unwrap_or_default();

            for imp in imports {
                // Now we can call resolve_import_path (immut) and add_edge (mut)
                if let Some(target_fid) = self.resolve_import_path(fid, &imp.source) {
                    deps.insert(target_fid);
                    if
                        let (Some(src_id), Some(tgt_id)) = (
                            src_module_id,
                            file_to_module_sym.get(&target_fid),
                        )
                    {
                        self.add_edge(src_id, *tgt_id, EdgeKind::Imports);
                    }
                }
            }
            if !deps.is_empty() {
                self.index.file_dependencies.insert(fid, deps.into_iter().collect());
            }
        }
    }

    pub(crate) fn resolve_namespace_imports(&mut self) {
        let file_mod_map: HashMap<FileId, String> = self.index.symbols
            .values()
            .filter(|s| s.kind == SymbolKind::Module)
            .map(|s| (s.file_id, s.name.clone()))
            .collect();

        let file_ids: Vec<FileId> = self.lookup.file_imports.keys().cloned().collect();

        for fid in file_ids {
            let mod_sym_id = self.index.symbols
                .values()
                .find(|s| s.file_id == fid && s.kind == SymbolKind::Module)
                .map(|s| s.id);

            if let Some(scope_id) = mod_sym_id {
                // FIX: Clone imports to avoid borrow conflict
                let imports = self.lookup.file_imports.get(&fid).cloned().unwrap_or_default();

                for imp in imports {
                    if imp.name == "*" {
                        if let Some(alias) = &imp.alias {
                            if let Some(target_fid) = self.resolve_import_path(fid, &imp.source) {
                                if let Some(target_mod_name) = file_mod_map.get(&target_fid) {
                                    self.staging.local_variable_types
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

    pub(crate) fn resolve_literal_dependencies(&mut self) {
        let mut potential_links: Vec<(FileId, String)> = Vec::new();
        let config_file_ids: HashSet<FileId> = self.index.files
            .iter()
            .filter(|(path, _)| {
                let p = path.to_lowercase();
                p.ends_with("json") || p.ends_with(".env")
            })
            .map(|(_, node)| node.id)
            .collect();

        // FIX: Snapshot staging.raw_literals to break borrow
        let literals_snapshot: Vec<(FileId, Vec<String>)> = self.staging.raw_literals
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();

        for (file_id, literals) in literals_snapshot {
            if config_file_ids.contains(&file_id) {
                continue;
            }
            for lit in literals {
                if
                    (lit.contains('/') || lit.contains('.')) &&
                    !lit.contains(' ') &&
                    !lit.contains('\n') &&
                    lit.len() > 3
                {
                    potential_links.push((file_id, lit));
                }
            }
        }

        for (src_id, literal) in potential_links {
            if let Some(target_id) = self.resolve_import_path(src_id, &literal) {
                if src_id != target_id {
                    let deps = self.index.file_dependencies.entry(src_id).or_default();
                    if !deps.contains(&target_id) {
                        deps.push(target_id);
                    }
                    self.link_modules(src_id, target_id);
                }
            }
        }
    }

    pub(crate) fn resolve_shared_literals(&mut self) {
        let mut literal_map: HashMap<String, Vec<FileId>> = HashMap::new();
        let config_file_ids: HashSet<FileId> = self.index.files
            .iter()
            .filter(|(path, _)| {
                let p = path.to_lowercase();
                p.ends_with("json") || p.ends_with(".env")
            })
            .map(|(_, node)| node.id)
            .collect();

        // FIX: Snapshot staging.raw_literals
        let literals_snapshot: Vec<(FileId, Vec<String>)> = self.staging.raw_literals
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();

        for (file_id, literals) in literals_snapshot {
            if config_file_ids.contains(&file_id) {
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
                    literal_map.entry(lit).or_default().push(file_id);
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

                    let deps_a = self.index.file_dependencies.entry(id_a).or_default();
                    if !deps_a.contains(&id_b) {
                        deps_a.push(id_b);
                    }
                    let deps_b = self.index.file_dependencies.entry(id_b).or_default();
                    if !deps_b.contains(&id_a) {
                        deps_b.push(id_a);
                    }

                    self.link_modules(id_a, id_b);
                }
            }
        }
    }

    pub(crate) fn resolve_iac_links(&mut self) {
        let mut new_file_links = Vec::new();
        let mut env_var_definitions: HashMap<String, Vec<FileId>> = HashMap::new();

        // FIX: Snapshot staging.raw_literals
        let literals_snapshot: Vec<(FileId, Vec<String>)> = self.staging.raw_literals
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();

        for (file_id, literals) in literals_snapshot {
            for lit in literals {
                let parts = lit.split(|c: char| !c.is_alphanumeric() && c != '_');
                for part in parts {
                    if
                        part.len() > 3 &&
                        part.chars().all(|c| c.is_uppercase() || c.is_numeric() || c == '_') &&
                        part.contains('_')
                    {
                        let entry = env_var_definitions.entry(part.to_string()).or_default();
                        if !entry.contains(&file_id) {
                            entry.push(file_id);
                        }
                    }
                }
            }
        }

        // Snapshot staging.symbol_config_refs to iterate safely
        let config_refs_snapshot: Vec<(SymbolId, Vec<String>)> = self.staging.symbol_config_refs
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();

        for (sym_id, config_keys) in config_refs_snapshot {
            // Need to look up symbol to get file_id, which borrows self.index.symbols
            let sym_file_id = self.index.symbols.get(&sym_id).map(|s| s.file_id);

            if let Some(fid) = sym_file_id {
                for key in config_keys {
                    if let Some(def_files) = env_var_definitions.get(&key) {
                        for &def_file_id in def_files {
                            if fid != def_file_id {
                                new_file_links.push((fid, def_file_id));
                            }
                        }
                    }
                }
            }
        }

        // AWS Heuristic
        // Snapshot lookup.file_imports
        let imports_snapshot: Vec<
            (FileId, Vec<crate::models::ImportNode>)
        > = self.lookup.file_imports
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();

        let mut aws_s3_users = Vec::new();
        for (file_id, imports) in imports_snapshot {
            for imp in imports {
                if imp.source.contains("aws-sdk") || imp.source.contains("boto3") {
                    aws_s3_users.push(file_id);
                }
            }
        }

        let mut s3_definers = Vec::new();
        for sym in self.index.symbols.values() {
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
            let deps = self.index.file_dependencies.entry(src).or_default();
            if !deps.contains(&tgt) {
                deps.push(tgt);
            }
            // Implicit link module -> module
            self.link_modules(src, tgt);
        }
    }
}