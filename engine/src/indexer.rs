use crate::schema::{ WorkspaceIndex, FileNode, SymbolNode, SymbolId, FileId };
use crate::analyzer::analyze_source;
use crate::language::{ get_language_configs, LanguageConfig };
use crate::manifest::scan_manifests;
use std::path::{ Path, PathBuf };
use std::fs::{ self, File };
use std::io::Write;
use std::collections::{ HashMap, HashSet };
use ignore::WalkBuilder;
use blake3;
use memmap2::MmapOptions;
use rkyv::{ to_bytes, check_archived_root };

pub struct Indexer {
    pub index: WorkspaceIndex,
    configs: HashMap<String, &'static LanguageConfig>,
    /// Transient cache used during resolution to avoid redundant barrel walks.
    resolution_cache: HashMap<(FileId, String), Option<SymbolId>>,
}

impl Indexer {
    pub fn new() -> Self {
        let mut config_map: HashMap<String, &'static LanguageConfig> = HashMap::new();
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

    /// Normalizes paths and strips Windows UNC prefixes (\\?\) to ensure
    /// consistency between indexed keys and CLI input.
    fn to_index_path(path: &Path) -> String {
        let abs_path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

        let path_str = abs_path.to_string_lossy().to_string();

        // Strip Windows UNC prefix if it exists
        if path_str.starts_with(r"\\?\") {
            path_str[4..].to_string()
        } else {
            path_str
        }
    }

    /// Detects if a file path corresponds to a framework route (e.g. Next.js pages/api)
    /// Returns the route string (e.g. "/api/user") if detected.
    fn detect_framework_route(path: &Path) -> Option<String> {
        // Normalize path separators to forward slashes for matching
        let path_str = path.to_string_lossy().replace('\\', "/");
        
        // 1. Next.js Pages Router (pages/api/...)
        if let Some(idx) = path_str.find("/pages/api/") {
            let relative = &path_str[idx + "/pages".len()..]; // e.g. "/api/user.ts"
            // Remove extension logic
            if let Some(dot_idx) = relative.rfind('.') {
                 let route = &relative[..dot_idx];
                 // Handle index routes: /api/users/index -> /api/users
                 if route.ends_with("/index") {
                     return Some(route[..route.len() - "/index".len()].to_string());
                 }
                 return Some(route.to_string());
            }
        }

        // 2. Next.js App Router (app/api/.../route.ts)
        // file: .../app/api/auth/route.ts -> route: /api/auth
        if path_str.ends_with("/route.ts") || path_str.ends_with("/route.js") {
            if let Some(app_idx) = path_str.find("/app/") {
                if let Some(route_idx) = path_str.rfind("/route.") {
                     // skip "/app" (length 4) and grab until "/route"
                     let relative = &path_str[app_idx + 4..route_idx]; 
                     return Some(relative.to_string());
                }
            }
        }

        None
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let bytes = to_bytes::<_, 4096>(&self.index).map_err(|e|
            anyhow::anyhow!("Serialization failed: {}", e)
        )?;
        let mut file = File::create(path)?;
        file.write_all(&bytes)?;
        Ok(())
    }

    pub fn load_from_file(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }
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
            
            // Clean up implicit routes for this file
            self.index.implicit_routes.retain(|_, v| *v != node.id);
        }
    }

    fn clear_file_symbols(&mut self, file_id: FileId) {
        let ids_to_remove: Vec<SymbolId> = self.index.symbols
            .values()
            .filter(|s| s.file_id == file_id)
            .map(|s| s.id)
            .collect();

        for &sym_id in &ids_to_remove {
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
            self.index.local_variable_types.remove(&sym_id);
            self.index.inheritance.remove(&sym_id);
            self.index.symbol_config_refs.remove(&sym_id);
            self.index.raw_type_refs.remove(&sym_id);
            self.index.resolved_type_refs.remove(&sym_id);
            self.index.raw_decorators.remove(&sym_id);
        }

        // Clean up config definitions mapping
        for def_list in self.index.config_definitions.values_mut() {
            def_list.retain(|id| !ids_to_remove.contains(id));
        }
        self.index.config_definitions.retain(|_, v| !v.is_empty());

        self.index.file_imports.remove(&file_id);
        self.index.file_exports.remove(&file_id);
        self.index.raw_literals.remove(&file_id);
    }

    pub fn scan(&mut self, root: &Path) {
        let root_abs = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        let mut seen_paths = HashSet::new();

        // 1. Configure the walker to respect gitignore
        let walker = WalkBuilder::new(&root_abs)
            .hidden(false) // Still scan hidden files like .env
            .git_ignore(true)
            .build();

        for result in walker {
            match result {
                Ok(entry) => {
                    if !entry.path().is_file() {
                        continue;
                    }
                    let path = entry.path();

                    // 2. Opportunistic Manifest Scanning
                    let new_externals = scan_manifests(path);
                    if !new_externals.is_empty() {
                        self.index.external_packages.extend(new_externals);
                    }

                    let ext = path
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("");
                    let filename = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("");

                    // Logic to handle hidden files like .env correctly
                    let config = self.configs
                        .get(ext)
                        .or_else(|| self.configs.get(filename))
                        .or_else(|| 
                            if filename.starts_with('.') {
                                self.configs.get(&filename[1..])
                            } else {
                                None
                            }
                        );

                    if let Some(config) = config {
                        if let Ok(content) = fs::read_to_string(path) {
                            let hash = blake3::hash(content.as_bytes());
                            let hash_bytes: [u8; 32] = hash.into();
                            let path_key = Self::to_index_path(path);

                            seen_paths.insert(path_key.clone());

                            let is_test = match path.strip_prefix(&root_abs) {
                                Ok(rel) => Self::is_test_path(rel),
                                Err(_) => {
                                    let fname = path.file_name().map(Path::new).unwrap_or(path);
                                    Self::is_test_path(fname)
                                }
                            };

                            let needs_update = match self.index.files.get(&path_key) {
                                Some(node) => node.hash != hash_bytes,
                                None => true,
                            };
                            if needs_update {
                                self.update_file(
                                    &path_key,
                                    path,
                                    &content,
                                    hash_bytes,
                                    config,
                                    is_test
                                );
                            }
                        }
                    }
                }
                Err(err) => eprintln!("Error walking directory: {}", err),
            }
        }

        // Cleanup removed files
        let to_remove: Vec<String> = self.index.files
            .keys()
            .filter(|path_key| !seen_paths.contains(*path_key))
            .cloned()
            .collect();

        for path_key in to_remove {
            self.remove_file(&path_key);
        }
    }

    fn is_test_path(path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        let normalized = path_str.replace('\\', "/").to_lowercase();
        let path_obj = Path::new(&normalized);

        // 1. Expanded folder-based detection
        let has_test_folder = path_obj.components().any(|c| {
            let s = c.as_os_str().to_string_lossy();
            matches!(
                s.as_ref(),
                "test" |
                    "tests" |
                    "__tests__" |
                    "spec" |
                    "specs" |
                    "integration-test" |
                    "fixtures" |
                    "__fixtures__" |
                    "mocks" |
                    "__mocks__" |
                    "stubs"
            )
        });

        if has_test_folder {
            return true;
        }

        // 2. Check filename patterns
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        if
            filename.ends_with(".test.ts") ||
            filename.ends_with(".test.tsx") ||
            filename.ends_with(".spec.ts") ||
            filename.ends_with(".spec.tsx") ||
            filename.ends_with(".fixture.ts") ||
            filename.ends_with(".mock.ts") ||
            filename.ends_with(".test.js") ||
            filename.ends_with(".test.jsx") ||
            filename.ends_with(".spec.js") ||
            filename.ends_with(".spec.jsx")
        {
            return true;
        }

        if
            filename.ends_with("_test.rs") ||
            filename.ends_with("_spec.rs") ||
            filename == "test.rs"
        {
            return true;
        }

        if filename.starts_with("test_") || filename.ends_with("_test.py") {
            return true;
        }

        if filename.ends_with("test.java") || filename.ends_with("tests.java") {
            return true;
        }

        if
            (filename.contains("mock") || filename.contains("fixture")) &&
            (filename.ends_with(".ts") ||
                filename.ends_with(".js") ||
                filename.ends_with(".rs") ||
                filename.ends_with(".py"))
        {
            return true;
        }

        false
    }

    fn update_file(
        &mut self,
        path_key: &str,
        path_obj: &Path,
        content: &str,
        hash: [u8; 32],
        config: &LanguageConfig,
        is_path_test: bool
    ) {
        let file_id = match self.index.files.get(path_key) {
            Some(node) => node.id,
            None => {
                let id = self.index.next_file_id;
                self.index.next_file_id += 1;
                id
            }
        };

        self.clear_file_symbols(file_id);
        self.index.files.insert(path_key.to_string(), FileNode {
            id: file_id,
            path: path_key.to_string(),
            hash,
            is_test: is_path_test,
        });

        // --- Implicit Route Detection ---
        if let Some(route) = Self::detect_framework_route(path_obj) {
            self.index.implicit_routes.insert(route, file_id);
        }

        match analyze_source(path_obj, content, config) {
            Ok(analysis) => {
                if !analysis.imports.is_empty() {
                    self.index.file_imports.insert(file_id, analysis.imports);
                }
                if !analysis.exports.is_empty() {
                    self.index.file_exports.insert(file_id, analysis.exports);
                }
                if !analysis.literals.is_empty() {
                    self.index.raw_literals.insert(file_id, analysis.literals);
                }

                let mut file_symbol_ids = Vec::new();

                for func in analysis.functions {
                    let symbol_id = self.index.next_symbol_id;
                    self.index.next_symbol_id += 1;

                    let name_lower = func.name.to_lowercase();
                    let code_trim = func.source_code.trim_start();
                    let is_module = func.name.starts_with("(module)");

                    let kind = if is_module {
                        "module".to_string()
                    } else if
                        func.source_code.contains("class ") ||
                        func.source_code.contains("interface ")
                    {
                        "container".to_string()
                    } else {
                        "function".to_string()
                    };

                    // Test Detection Logic...
                    let is_js_test_block =
                        code_trim.starts_with("it(") ||
                        code_trim.starts_with("it.") ||
                        code_trim.starts_with("test(") ||
                        code_trim.starts_with("test.") ||
                        code_trim.starts_with("describe(") ||
                        code_trim.starts_with("describe.") ||
                        code_trim.starts_with("suite(") ||
                        code_trim.starts_with("context(") ||
                        code_trim.starts_with("beforeEach(") ||
                        code_trim.starts_with("afterEach(");

                    let is_test_named =
                        name_lower.starts_with("test_") ||
                        name_lower.ends_with("_test") ||
                        name_lower == "test" ||
                        name_lower.contains("mock") ||
                        name_lower.contains("fixture");

                    let has_test_decorator =
                        func.source_code.contains("#[test]") ||
                        func.source_code.contains("@Test") ||
                        func.source_code.contains("@fixture");

                    let is_inline_test =
                        !is_module && (is_js_test_block || is_test_named || has_test_decorator);

                    self.index.symbols.insert(symbol_id, SymbolNode {
                        id: symbol_id,
                        file_id,
                        parent_id: None,
                        name: func.name.clone(),
                        kind: kind.clone(),
                        range_start: func.range_start,
                        range_end: func.range_end,
                        doc_comment: func.documentation,
                        return_type: func.return_type,
                        is_test: is_path_test || is_inline_test,
                        is_external: false,
                        external_source: None,
                        decorators: func.decorators.clone(),
                    });

                    file_symbol_ids.push(symbol_id);

                    if func.name != "anonymous" {
                        self.index.symbol_map.entry(func.name.clone()).or_default().push(symbol_id);
                    }

                    if !func.config_keys.is_empty() {
                        self.index.symbol_config_refs.insert(symbol_id, func.config_keys);
                    }

                    if !func.calls.is_empty() {
                        self.index.raw_calls.insert(symbol_id, func.calls);
                    }
                    if !func.fingerprints.is_empty() {
                        self.index.fingerprints.insert(symbol_id, func.fingerprints);
                    }

                    if !func.local_types.is_empty() {
                        self.index.local_variable_types.insert(symbol_id, func.local_types);
                    }

                    if !func.type_refs.is_empty() {
                        self.index.raw_type_refs.insert(symbol_id, func.type_refs);
                    }
                    
                    if !func.decorators.is_empty() {
                        self.index.raw_decorators.insert(symbol_id, func.decorators);
                    }
                }

                // Config Data Linking
                let is_data_file = matches!(
                    config.lang_enum,
                    crate::language::SupportedLanguage::Yaml |
                        crate::language::SupportedLanguage::Json |
                        crate::language::SupportedLanguage::Toml |
                        crate::language::SupportedLanguage::Dotenv
                );
                if is_data_file {
                    for &sid in &file_symbol_ids {
                        let name = &self.index.symbols[&sid].name;
                        self.index.config_definitions.entry(name.clone()).or_default().push(sid);
                    }
                }

                // Container/Member Linking
                let container_ids: Vec<SymbolId> = file_symbol_ids
                    .iter()
                    .filter(|&&id| {
                        let s = &self.index.symbols[&id];
                        s.kind == "container" || s.kind == "module"
                    })
                    .cloned()
                    .collect();

                for c_id in container_ids {
                    let (cs, ce, c_kind) = {
                        let c = &self.index.symbols[&c_id];
                        (c.range_start, c.range_end, c.kind.clone())
                    };

                    let mut members = HashSet::new();
                    for &s_id in &file_symbol_ids {
                        if s_id == c_id {
                            continue;
                        }
                        let is_member = {
                            let s = &self.index.symbols[&s_id];
                            s.range_start >= cs && s.range_end <= ce
                        };

                        if is_member {
                            members.insert(self.index.symbols[&s_id].name.clone());

                            if c_kind != "module" {
                                if let Some(node) = self.index.symbols.get_mut(&s_id) {
                                    node.parent_id = Some(c_id);
                                }
                            }
                        }
                    }
                    if !members.is_empty() {
                        self.index.container_methods.insert(c_id, members);
                    }
                }

                // Link Orphans
                let module_id = file_symbol_ids
                    .iter()
                    .find(|&&id| { self.index.symbols[&id].kind == "module" })
                    .cloned();

                if let Some(mid) = module_id {
                    for &id in &file_symbol_ids {
                        if id == mid {
                            continue;
                        }

                        if let Some(sym) = self.index.symbols.get_mut(&id) {
                            if sym.parent_id.is_none() {
                                sym.parent_id = Some(mid);
                            }
                        }
                    }
                }

                for (child, parent) in analysis.implementations {
                    if let Some(ids) = self.index.symbol_map.get(&child) {
                        if
                            let Some(&cid) = ids
                                .iter()
                                .find(|&&id| self.index.symbols[&id].file_id == file_id)
                        {
                            self.index.raw_implementations.entry(cid).or_default().push(parent);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Error analyzing file {:?}: {}", path_obj, e);
            }
        }
    }

    pub fn resolve_references(&mut self) {
        self.index.resolved_calls.clear();
        self.index.resolved_type_refs.clear();
        self.index.inheritance.clear();
        self.index.file_dependencies.clear();

        self.resolution_cache.clear();
        self.resolve_external_imports();
        self.resolve_decorators();
        self.resolve_implicit_routes(); // Implicit route resolution
        self.resolve_namespace_imports();
        self.resolve_literal_dependencies();
        self.resolve_shared_literals();
        self.resolve_type_sniffing();
        self.resolve_fingerprints();
        self.resolve_implicit_connections();
        self.resolve_function_calls_with_fallback();
        self.resolve_config_links();
        self.resolve_type_references();
        self.resolve_database_references();
        self.resolve_file_dependencies();
    }

    fn resolve_external_imports(&mut self) {
        let mut new_symbols = Vec::new();

        for (_file_id, imports) in &self.index.file_imports {
            for imp in imports {
                if
                    !imp.source.starts_with("./") &&
                    !imp.source.starts_with("../") &&
                    !imp.source.starts_with("/")
                {
                    let pkg_name = imp.source.clone();
                    let sym_name = imp.alias.clone().unwrap_or(imp.name.clone());

                    let stub_id = if let Some(ids) = self.index.symbol_map.get(&sym_name) {
                        ids.iter()
                            .find(|&&id| {
                                let s = &self.index.symbols[&id];
                                s.is_external &&
                                    s.external_source.as_deref() == Some(pkg_name.as_str())
                            })
                            .cloned()
                    } else {
                        None
                    };

                    if stub_id.is_none() {
                        let new_id = self.index.next_symbol_id;
                        self.index.next_symbol_id += 1;

                        new_symbols.push(SymbolNode {
                            id: new_id,
                            file_id: 0,
                            parent_id: None,
                            name: sym_name.clone(),
                            kind: "external".to_string(),
                            range_start: 0,
                            range_end: 0,
                            doc_comment: Some(
                                format!("External import from package `{}`", pkg_name)
                            ),
                            return_type: None,
                            is_test: false,
                            is_external: true,
                            external_source: Some(pkg_name.clone()),
                            decorators: Vec::new(),
                        });
                    }
                }
            }
        }

        for sym in new_symbols {
            self.index.symbol_map.entry(sym.name.clone()).or_default().push(sym.id);
            self.index.symbols.insert(sym.id, sym);
        }
    }

    fn resolve_decorators(&mut self) {
        let entries: Vec<(SymbolId, Vec<String>)> = self.index.raw_decorators
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();

        for (caller_id, dec_names) in entries {
            let caller_file_id = self.index.symbols[&caller_id].file_id;

            for dec_name in dec_names {
                // Clean the name further if needed (e.g. remove "()")
                let clean = dec_name.split('(').next().unwrap_or(&dec_name).trim();

                // Treat decorator as a dependency (like a function call)
                if let Some(target_id) = self.resolve_single_call(caller_file_id, clean) {
                    self.index.resolved_calls.entry(caller_id).or_default().push(target_id);
                } else if let Some(candidates) = self.index.symbol_map.get(clean) {
                    let mut guesses = candidates.clone();
                    self.index.resolved_calls
                        .entry(caller_id)
                        .or_default()
                        .append(&mut guesses);
                }
            }
            if let Some(resolved) = self.index.resolved_calls.get_mut(&caller_id) {
                resolved.sort();
                resolved.dedup();
            }
        }
    }

    fn resolve_implicit_routes(&mut self) {
        let mut new_links = Vec::new();

        // Iterate over every string literal found in code (e.g. fetch('/api/user'))
        for (src_file_id, literals) in &self.index.raw_literals {
            for lit in literals {
                // Clean quotes
                let clean_lit = lit.trim_matches(|c| c == '"' || c == '\'' || c == '`');
                
                // Direct Match: frontend uses "/api/user" and we have that implicit route registered
                if let Some(&target_file_id) = self.index.implicit_routes.get(clean_lit) {
                    // Avoid self-referential links if literal appears in defining file (rare for routes)
                    if *src_file_id != target_file_id {
                        new_links.push((*src_file_id, target_file_id));
                    }
                }
            }
        }

        // Create Dependencies
        for (src, tgt) in new_links {
            // Link File Dependency
            let deps = self.index.file_dependencies.entry(src).or_default();
            if !deps.contains(&tgt) {
                deps.push(tgt);
            }

            // Link Module Symbols (so semantic search traverses it)
            // We link the module symbol of the source file to the module symbol of the target file
            let src_mod = self.index.symbols.values().find(|s| s.file_id == src && s.kind == "module").map(|s| s.id);
            let tgt_mod = self.index.symbols.values().find(|s| s.file_id == tgt && s.kind == "module").map(|s| s.id);

            if let (Some(s), Some(t)) = (src_mod, tgt_mod) {
                let calls = self.index.resolved_calls.entry(s).or_default();
                if !calls.contains(&t) {
                    calls.push(t);
                }
            }
        }
    }

    fn resolve_namespace_imports(&mut self) {
        let file_mod_map: HashMap<FileId, String> = self.index.symbols
            .values()
            .filter(|s| s.kind == "module")
            .map(|s| (s.file_id, s.name.clone()))
            .collect();

        let file_ids: Vec<FileId> = self.index.file_imports.keys().cloned().collect();
        for fid in file_ids {
            let mod_sym_id = self.index.symbols
                .values()
                .find(|s| s.file_id == fid && s.kind == "module")
                .map(|s| s.id);

            if let Some(scope_id) = mod_sym_id {
                let imports = self.index.file_imports.get(&fid).cloned().unwrap_or_default();

                for imp in imports {
                    if imp.name == "*" {
                        if let Some(alias) = &imp.alias {
                            if let Some(target_fid) = self.resolve_import_path(fid, &imp.source) {
                                if let Some(target_mod_name) = file_mod_map.get(&target_fid) {
                                    self.index.local_variable_types
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

    fn resolve_config_links(&mut self) {
        let mut new_links = Vec::new();
        for (&sym_id, used_keys) in &self.index.symbol_config_refs {
            for key in used_keys {
                if let Some(def_ids) = self.index.config_definitions.get(key) {
                    for &target_sid in def_ids {
                        new_links.push((sym_id, target_sid));
                    }
                }
            }
        }
        for (caller, target) in new_links {
            self.index.resolved_calls.entry(caller).or_default().push(target);
        }
        for calls in self.index.resolved_calls.values_mut() {
            calls.sort();
            calls.dedup();
        }
    }

    fn resolve_symbol_across_barrels(
        &mut self,
        target_file_id: FileId,
        symbol_name: &str,
        visited: &mut HashSet<FileId>
    ) -> Option<SymbolId> {
        if visited.contains(&target_file_id) {
            return None;
        }
        visited.insert(target_file_id);
        if
            let Some(&cached_res) = self.resolution_cache.get(
                &(target_file_id, symbol_name.to_string())
            )
        {
            return cached_res;
        }

        let mut result = None;
        if let Some(ids) = self.index.symbol_map.get(symbol_name) {
            if
                let Some(&id) = ids
                    .iter()
                    .find(|&&id| self.index.symbols[&id].file_id == target_file_id)
            {
                result = Some(id);
            }
        }
        if result.is_none() {
            if let Some(exports) = self.index.file_exports.get(&target_file_id).cloned() {
                for exp in exports.iter().filter(|e| e.name.as_deref() == Some(symbol_name)) {
                    if
                        let Some(next_file_id) = self.resolve_import_path(
                            target_file_id,
                            &exp.source
                        )
                    {
                        result = self.resolve_symbol_across_barrels(
                            next_file_id,
                            symbol_name,
                            visited
                        );
                        if result.is_some() {
                            break;
                        }
                    }
                }
                if result.is_none() {
                    for exp in exports.iter().filter(|e| e.name.is_none()) {
                        if
                            let Some(next_file_id) = self.resolve_import_path(
                                target_file_id,
                                &exp.source
                            )
                        {
                            result = self.resolve_symbol_across_barrels(
                                next_file_id,
                                symbol_name,
                                visited
                            );
                            if result.is_some() {
                                break;
                            }
                        }
                    }
                }
            }
        }
        self.resolution_cache.insert((target_file_id, symbol_name.to_string()), result);
        result
    }

    fn resolve_function_calls_with_fallback(&mut self) {
        let entries: Vec<(SymbolId, Vec<String>)> = self.index.raw_calls
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();
        for (caller_id, called_names) in entries {
            let caller_file_id = self.index.symbols[&caller_id].file_id;
            for name in called_names {
                let already_resolved = if
                    let Some(resolved) = self.index.resolved_calls.get(&caller_id)
                {
                    resolved.iter().any(|&rid| self.index.symbols[&rid].name == name)
                } else {
                    false
                };
                if !already_resolved {
                    if let Some(tid) = self.resolve_single_call(caller_file_id, &name) {
                        self.index.resolved_calls.entry(caller_id).or_default().push(tid);
                    } else if let Some(candidates) = self.index.symbol_map.get(&name) {
                        let mut guesses = candidates.clone();
                        self.index.resolved_calls
                            .entry(caller_id)
                            .or_default()
                            .append(&mut guesses);
                    }
                }
            }
            if let Some(resolved) = self.index.resolved_calls.get_mut(&caller_id) {
                resolved.sort();
                resolved.dedup();
            }
        }
    }

    fn resolve_type_references(&mut self) {
        let entries: Vec<(SymbolId, Vec<String>)> = self.index.raw_type_refs
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();

        for (caller_id, type_names) in entries {
            let caller_file_id = self.index.symbols[&caller_id].file_id;

            for type_name in type_names {
                // Reuse logic: imports -> local definitions -> external stubs
                if let Some(target_id) = self.resolve_single_call(caller_file_id, &type_name) {
                    self.index.resolved_type_refs.entry(caller_id).or_default().push(target_id);
                } else if let Some(candidates) = self.index.symbol_map.get(&type_name) {
                    // Fallback: Link to any symbol with this name if we can't trace the exact import
                    // This creates loose coupling which is better than no coupling for type impact
                    let mut guesses = candidates.clone();
                    self.index.resolved_type_refs
                        .entry(caller_id)
                        .or_default()
                        .append(&mut guesses);
                }
            }

            if let Some(refs) = self.index.resolved_type_refs.get_mut(&caller_id) {
                refs.sort();
                refs.dedup();
            }
        }
    }

    fn resolve_type_sniffing(&mut self) {
        let mut new_links = Vec::new();
        for (&caller_id, receiver_map) in &self.index.fingerprints {
            for (receiver, methods) in receiver_map {
                let mut type_hint = None;
                let mut curr_scope = Some(caller_id);

                while let Some(sid) = curr_scope {
                    if let Some(vars) = self.index.local_variable_types.get(&sid) {
                        if let Some(h) = vars.get(receiver) {
                            type_hint = Some(h);
                            break;
                        }
                    }
                    curr_scope = self.index.symbols.get(&sid).and_then(|s| s.parent_id);
                }

                if let Some(hint) = type_hint {
                    let mut resolved_type = None;
                    if hint.starts_with("returns:") {
                        if let Some(targets) = self.index.symbol_map.get(&hint[8..]) {
                            resolved_type = self.index.symbols
                                .get(&targets[0])
                                .and_then(|s| s.return_type.clone());
                        }
                    } else {
                        resolved_type = Some(hint.clone());
                    }

                    if let Some(tn) = resolved_type {
                        let clean = tn.split('<').next().unwrap().to_string();
                        if let Some(type_ids) = self.index.symbol_map.get(&clean) {
                            for &tid in type_ids {
                                new_links.push((caller_id, tid));
                                if let Some(mems) = self.index.container_methods.get(&tid) {
                                    for m in methods {
                                        if mems.contains(m) {
                                            if let Some(mids) = self.index.symbol_map.get(m) {
                                                new_links.extend(
                                                    mids
                                                        .iter()
                                                        .filter(
                                                            |&&mid|
                                                                self.index.symbols
                                                                    [&mid].parent_id == Some(tid)
                                                        )
                                                        .map(|&mid| (caller_id, mid))
                                                );
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
        for (c, t) in new_links {
            self.index.resolved_calls.entry(c).or_default().push(t);
        }
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

    fn resolve_single_call(&mut self, file_id: FileId, name: &str) -> Option<SymbolId> {
        // 1. Check imports explicitly
        if let Some(imps) = self.index.file_imports.get(&file_id).cloned() {
            for imp in imps {
                if imp.alias.as_ref().unwrap_or(&imp.name) == name {
                    // A. Try Local Resolution
                    if let Some(tfid) = self.resolve_import_path(file_id, &imp.source) {
                        let mut visited = HashSet::new();
                        if
                            let Some(found) = self.resolve_symbol_across_barrels(
                                tfid,
                                &imp.name,
                                &mut visited
                            )
                        {
                            return Some(found);
                        }
                    }

                    // B. Try External Resolution
                    if let Some(candidates) = self.index.symbol_map.get(&imp.name) {
                        for &cid in candidates {
                            let s = &self.index.symbols[&cid];
                            if
                                s.is_external &&
                                s.external_source.as_deref() == Some(imp.source.as_str())
                            {
                                return Some(cid);
                            }
                        }
                    }
                }
            }
        }

        // 2. Check local file definitions
        if let Some(ids) = self.index.symbol_map.get(name) {
            if let Some(&id) = ids.iter().find(|&&id| self.index.symbols[&id].file_id == file_id) {
                return Some(id);
            }
        }

        None
    }

    fn resolve_import_path(&self, from_id: FileId, source: &str) -> Option<FileId> {
        let from_path_str = &self.index.files.values().find(|f| f.id == from_id)?.path;
        let from_path = Path::new(from_path_str);
        let parent = from_path.parent()?;

        let base = parent.join(source);
        if let Some(id) = self.index.files.get(&Self::to_index_path(&base)).map(|n| n.id) {
            return Some(id);
        }

        let exts = ["ts", "js", "tsx", "rs", "py", "json", "sh"];
        let check = |p: PathBuf| self.index.files.get(&Self::to_index_path(&p)).map(|n| n.id);

        for e in exts {
            if let Some(id) = check(base.with_extension(e)) {
                return Some(id);
            }
            if let Some(id) = check(base.join(format!("index.{}", e))) {
                return Some(id);
            }
        }
        None
    }

    fn resolve_database_references(&mut self) {
        // 1. Identify "Schema Symbols" (Tables/Models)
        let mut schema_map: HashMap<String, SymbolId> = HashMap::new();

        for (id, sym) in &self.index.symbols {
            let file_opt = self.index.files.values().find(|f| f.id == sym.file_id);
            if let Some(file) = file_opt {
                let is_schema_file = file.path.ends_with(".sql") || file.path.ends_with(".prisma");
                if is_schema_file {
                    schema_map.insert(sym.name.clone(), *id);
                }
            }
        }

        let mut new_links = Vec::new();

        // 2. Scan Code Literals (SQL Strings)
        // We iterate over every file's string literals.
        for (file_id, literals) in &self.index.raw_literals {
            // Find module symbol for this file to attach the link to
            let module_sym_id = self.index.symbols
                .values()
                .find(|s| s.file_id == *file_id && s.kind == "module")
                .map(|s| s.id);

            if let Some(mod_id) = module_sym_id {
                for lit in literals {
                    let clean_lit = lit.trim_matches(|c| c == '"' || c == '\'' || c == '`');

                    // Helper logic inline: split by non-alphanumeric to find table names
                    let words: Vec<&str> = clean_lit
                        .split(|c: char| !c.is_alphanumeric() && c != '_')
                        .collect();

                    for (table_name, &table_sym_id) in &schema_map {
                        // Check if the table name appears as a distinct word in the literal
                        if words.iter().any(|&w| w == table_name) {
                            new_links.push((mod_id, table_sym_id));
                        }
                    }
                }
            }
        }

        // 3. Scan Fingerprints (ORM Access like prisma.Order)
        for (func_id, prints) in &self.index.fingerprints {
            for (receiver, _) in prints {
                for (table_name, &table_sym_id) in &schema_map {
                    // Check for "Order" or ".Order" in "prisma.Order"
                    if receiver == table_name || receiver.ends_with(&format!(".{}", table_name)) {
                        new_links.push((*func_id, table_sym_id));
                    }
                }
            }
        }

        // 4. Commit Links
        for (code_sym, table_sym) in new_links {
            self.index.resolved_calls.entry(code_sym).or_default().push(table_sym);
        }
    }

    fn resolve_file_dependencies(&mut self) {
        let mut file_to_module_sym: HashMap<FileId, SymbolId> = HashMap::new();
        for sym in self.index.symbols.values() {
            if sym.kind == "module" {
                file_to_module_sym.insert(sym.file_id, sym.id);
            }
        }

        let fids: Vec<FileId> = self.index.file_imports.keys().cloned().collect();

        for fid in fids {
            let mut deps = HashSet::new();
            let src_module_id = file_to_module_sym.get(&fid).cloned();

            if let Some(imports) = self.index.file_imports.get(&fid) {
                for imp in imports {
                    if let Some(target_fid) = self.resolve_import_path(fid, &imp.source) {
                        deps.insert(target_fid);

                        if
                            let (Some(src_id), Some(tgt_id)) = (
                                src_module_id,
                                file_to_module_sym.get(&target_fid),
                            )
                        {
                            self.index.resolved_calls.entry(src_id).or_default().push(*tgt_id);
                        }
                    }
                }
            }
            if !deps.is_empty() {
                self.index.file_dependencies.insert(fid, deps.into_iter().collect());
            }
        }

        for calls in self.index.resolved_calls.values_mut() {
            calls.sort();
            calls.dedup();
        }
    }

    fn resolve_literal_dependencies(&mut self) {
        let mut potential_links: Vec<(FileId, String)> = Vec::new();

        for (&file_id, literals) in &self.index.raw_literals {
            for lit in literals {
                if
                    (lit.contains('/') || lit.contains('.')) &&
                    !lit.contains(' ') &&
                    !lit.contains('\n') &&
                    lit.len() > 3
                {
                    potential_links.push((file_id, lit.clone()));
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

                    let src_mod = self.index.symbols
                        .values()
                        .find(|s| s.file_id == src_id && s.kind == "module")
                        .map(|s| s.id);
                    let tgt_mod = self.index.symbols
                        .values()
                        .find(|s| s.file_id == target_id && s.kind == "module")
                        .map(|s| s.id);

                    if let (Some(s), Some(t)) = (src_mod, tgt_mod) {
                        let calls = self.index.resolved_calls.entry(s).or_default();
                        if !calls.contains(&t) {
                            calls.push(t);
                        }
                    }
                }
            }
        }
    }

    fn resolve_shared_literals(&mut self) {
        let mut literal_map: HashMap<String, Vec<FileId>> = HashMap::new();

        // 1. Build the Reverse Index
        for (&file_id, literals) in &self.index.raw_literals {
            for lit in literals {
                let is_route = lit.starts_with('/');
                let is_long_identifier =
                    lit.len() > 10 &&
                    !lit.contains(' ') &&
                    (lit.contains('_') || lit.contains('-') || lit.contains('.'));

                if (is_route || is_long_identifier) && lit.len() > 3 {
                    literal_map.entry(lit.clone()).or_default().push(file_id);
                }
            }
        }

        // 2. Create Edges for matches
        for (_lit, file_ids) in literal_map {
            if file_ids.len() < 2 {
                continue;
            }

            for i in 0..file_ids.len() {
                for j in i + 1..file_ids.len() {
                    let id_a = file_ids[i];
                    let id_b = file_ids[j];

                    if id_a == id_b {
                        continue;
                    }

                    let deps_a = self.index.file_dependencies.entry(id_a).or_default();
                    if !deps_a.contains(&id_b) {
                        deps_a.push(id_b);
                    }

                    let deps_b = self.index.file_dependencies.entry(id_b).or_default();
                    if !deps_b.contains(&id_a) {
                        deps_b.push(id_a);
                    }

                    // Link Modules
                    let mod_a = self.index.symbols
                        .values()
                        .find(|s| s.file_id == id_a && s.kind == "module")
                        .map(|s| s.id);
                    let mod_b = self.index.symbols
                        .values()
                        .find(|s| s.file_id == id_b && s.kind == "module")
                        .map(|s| s.id);

                    if let (Some(ma), Some(mb)) = (mod_a, mod_b) {
                        let calls_a = self.index.resolved_calls.entry(ma).or_default();
                        if !calls_a.contains(&mb) {
                            calls_a.push(mb);
                        }

                        let calls_b = self.index.resolved_calls.entry(mb).or_default();
                        if !calls_b.contains(&ma) {
                            calls_b.push(ma);
                        }
                    }
                }
            }
        }
    }

    pub fn get_impacted_files(&self, target_path: &Path) -> Vec<String> {
        let target_key = Self::to_index_path(target_path);
        let target_id = match self.index.files.get(&target_key) {
            Some(node) => node.id,
            None => {
                return vec![];
            }
        };
        let mut impacted_paths = Vec::new();
        for (&source_file_id, dependencies) in &self.index.file_dependencies {
            if dependencies.contains(&target_id) {
                if
                    let Some(source_node) = self.index.files
                        .values()
                        .find(|f| f.id == source_file_id)
                {
                    impacted_paths.push(source_node.path.clone());
                }
            }
        }
        impacted_paths
    }

    fn resolve_implicit_connections(&mut self) {
        self.index.inheritance.clear();
        for (cid, parents) in self.index.raw_implementations.clone() {
            for p in parents {
                if let Some(pids) = self.index.symbol_map.get(&p) {
                    for &pid in pids {
                        self.index.inheritance.entry(pid).or_default().push(cid);
                    }
                }
            }
        }
    }
}