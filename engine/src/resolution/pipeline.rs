use std::collections::HashMap;
use std::path::PathBuf;
use crate::models::{ FileId, SymbolId, WorkspaceIndex, StagingArea, SymbolIndex, EdgeKind };
use crate::analysis::language::LanguageConfig;
use crate::resolution::resolvers;
use crate::resolution::resolvers::add_edge;

use crate::resolution::scanner::FileScanner;
use crate::resolution::Indexer;

pub struct ResolutionPipeline {
    cache: HashMap<(FileId, String), Option<SymbolId>>,
}

pub struct Pipeline {
    pub scanner: FileScanner,
    pub resolver: ResolutionPipeline,
}

impl Pipeline {
    pub fn new() -> Self {
        Self {
            scanner: FileScanner::new(),
            resolver: ResolutionPipeline::new(),
        }
    }
    pub fn hydrate_maps(
        &self,
        index: &WorkspaceIndex,
        active_roots: &HashMap<String, PathBuf>
    ) -> (HashMap<PathBuf, FileId>, HashMap<FileId, PathBuf>) {
        let mut path_map = HashMap::new();
        let mut id_map = HashMap::new();

        for file_node in index.files.values() {
            if let Some(root_path) = active_roots.get(&file_node.root_id) {
                let relative_os = if cfg!(windows) {
                    file_node.relative_path.replace('/', "\\")
                } else {
                    file_node.relative_path.clone()
                };
                let absolute_path = root_path.join(&relative_os);
                path_map.insert(absolute_path.clone(), file_node.id);
                id_map.insert(file_node.id, absolute_path);
            }
        }
        (path_map, id_map)
    }

    pub fn hydrate_staging(&self, index: &WorkspaceIndex) -> StagingArea {
        let mut staging = StagingArea::default();

        for file in index.files.values() {
            if !file.literals.is_empty() {
                staging.raw_literals.insert(file.id, file.literals.clone());
            }
            if !file.middleware_usage.is_empty() {
                staging.raw_middleware_usage.insert(file.id, file.middleware_usage.clone());
            }
        }

        for sym in index.symbols.values() {
            if !sym.calls.is_empty() {
                staging.raw_calls.insert(sym.id, sym.calls.clone());
            }
            if !sym.type_refs.is_empty() {
                staging.raw_type_refs.insert(sym.id, sym.type_refs.clone());
            }
            if !sym.config_keys.is_empty() {
                staging.symbol_config_refs.insert(sym.id, sym.config_keys.clone());
            }
            if !sym.decorators.is_empty() {
                staging.raw_decorators.insert(sym.id, sym.decorators.clone());
            }
            if !sym.dispatched_actions.is_empty() {
                staging.raw_action_dispatches.insert(sym.id, sym.dispatched_actions.clone());
            }
            if !sym.handled_actions.is_empty() {
                staging.raw_action_handlers.insert(sym.id, sym.handled_actions.clone());
            }
            if !sym.local_types.is_empty() {
                staging.local_variable_types.insert(sym.id, sym.local_types.clone());
            }
            if !sym.fingerprints.is_empty() {
                staging.fingerprints.insert(sym.id, sym.fingerprints.clone());
            }

            if
                sym.kind == crate::models::SymbolKind::Container ||
                sym.kind == crate::models::SymbolKind::Module
            {
                let children_names: std::collections::HashSet<String> = index.symbols
                    .values()
                    .filter(|s| s.parent_id == Some(sym.id))
                    .map(|s| s.name.clone())
                    .collect();
                
                if !children_names.is_empty() {
                    staging.container_methods.insert(sym.id, children_names);
                }
            }
        }

        staging
    }

    pub fn scan(&self, indexer: &mut Indexer, root_path: &std::path::Path, root_id: Option<&str>) {
        self.scanner.scan(root_path, &mut indexer.index, &mut indexer.lookup, root_id);
    }

    pub fn resolve(&mut self, indexer: &mut Indexer, staging: &mut StagingArea, active_roots: &Vec<std::path::PathBuf> ) {
        self.resolver.run(
            &mut indexer.index,
            staging,
            &mut indexer.lookup,
            &indexer.path_map,
            &indexer.id_map,
            active_roots,
            &self.scanner.configs
        );
        indexer.build_reverse_graph();
    }
}

impl ResolutionPipeline {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn run(
        &mut self,
        index: &mut WorkspaceIndex,
        staging: &mut StagingArea,
        lookup: &mut SymbolIndex,
        path_map: &HashMap<PathBuf, FileId>, 
        id_map: &HashMap<FileId, PathBuf>,
        active_roots: &Vec<std::path::PathBuf>,
        configs: &HashMap<String, LanguageConfig>
    ) {
        // Reset Graph State
        index.graph.clear();
        index.file_dependencies.clear();
        self.cache.clear();

        // 0. Restore Structure
        self.resolve_structure(index);

        // 1. Core imports and basic structure
        resolvers::standard::resolve_external_imports(index, lookup);
        resolvers::state::resolve_decorators(index, staging, lookup, path_map, id_map, active_roots, &mut self.cache);
        resolvers::frameworks::resolve_implicit_routes(index, staging, lookup);
        resolvers::data::resolve_namespace_imports(index, staging, lookup, path_map, id_map, active_roots);

        // 2. Data and Literals
        resolvers::data::resolve_literal_dependencies(index, staging, lookup, path_map, id_map, active_roots);
        resolvers::data::resolve_shared_literals(index, staging, lookup);
        resolvers::state::resolve_pubsub_wildcards(index, staging, lookup, configs);

        // 3. Inference & Magic
        resolvers::inference::resolve_type_sniffing(index, staging, lookup);
        resolvers::state::resolve_magic_proxies(index, staging, lookup, configs);
        resolvers::inference::resolve_fingerprints(index, staging, lookup);
        resolvers::inference::resolve_implicit_connections(index, staging, lookup);

        // 4. Frameworks & State
        resolvers::frameworks::resolve_dependency_injection(
            index,
            staging,
            lookup,
            path_map,
            id_map,
            active_roots,
            &mut self.cache,
            configs
        ); // Updated
        resolvers::standard::resolve_function_calls(
            index,
            staging,
            lookup,
            path_map,
            id_map,
            active_roots,
            &mut self.cache
        );
        resolvers::data::resolve_config_links(index, staging, lookup);
        resolvers::standard::resolve_type_references(
            index,
            staging,
            lookup,
            path_map,
            id_map,
            active_roots,
            &mut self.cache
        );
        resolvers::data::resolve_database_references(index, staging);
        resolvers::data::resolve_file_dependencies(index, lookup, path_map, id_map, active_roots);
        resolvers::state::resolve_state_management(index, staging);
        resolvers::frameworks::resolve_middleware_injection(
            index,
            staging,
            lookup,
            path_map,
            id_map,
            active_roots,
            &mut self.cache
        ); 
        resolvers::data::resolve_iac_links(index, staging, lookup);
    }

    fn resolve_structure(&self, index: &mut WorkspaceIndex) {
        let mut edges_to_add = Vec::new();
        for sym in index.symbols.values() {
            if let Some(parent_id) = sym.parent_id {
                edges_to_add.push((parent_id, sym.id));
            }
        }
        for (parent_id, child_id) in edges_to_add {
            add_edge(index, parent_id, child_id, EdgeKind::Contains);
        }
    }
}