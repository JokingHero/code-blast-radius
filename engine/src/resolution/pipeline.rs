use std::collections::HashMap;

use crate::models::{
    FileId, SymbolId, WorkspaceIndex, StagingArea, SymbolIndex, EdgeKind
};
use crate::analysis::language::LanguageConfig;
use crate::resolution::resolvers;
use crate::resolution::resolvers::add_edge;

pub struct ResolutionPipeline {
    // Internal state needed only during resolution
    cache: HashMap<(FileId, String), Option<SymbolId>>,
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
        configs: &HashMap<String, &'static LanguageConfig>
    ) {
        // Reset Graph State
        index.graph.clear();
        index.file_dependencies.clear();
        self.cache.clear();

        // 0. Restore Structure
        self.resolve_structure(index);

        // 1. Core imports and basic structure
        resolvers::standard::resolve_external_imports(index, lookup);
        resolvers::state::resolve_decorators(index, staging, lookup, &mut self.cache);
        resolvers::frameworks::resolve_implicit_routes(index, staging, lookup);
        resolvers::data::resolve_namespace_imports(index, staging, lookup);

        // 2. Data and Literals
        resolvers::data::resolve_literal_dependencies(index, staging, lookup);
        resolvers::data::resolve_shared_literals(index, staging);
        resolvers::state::resolve_pubsub_wildcards(index, staging);

        // 3. Inference & Magic
        resolvers::inference::resolve_type_sniffing(index, staging, lookup);
        resolvers::state::resolve_magic_proxies(index, staging, lookup, configs);
        resolvers::inference::resolve_fingerprints(index, staging, lookup);
        resolvers::inference::resolve_implicit_connections(index, staging, lookup);

        // 4. Frameworks & State
        resolvers::frameworks::resolve_dependency_injection(index, staging, lookup, &mut self.cache, configs);
        resolvers::standard::resolve_function_calls(index, staging, lookup, &mut self.cache);
        resolvers::data::resolve_config_links(index, staging, lookup);
        resolvers::standard::resolve_type_references(index, staging, lookup, &mut self.cache);
        resolvers::data::resolve_database_references(index, staging);
        resolvers::data::resolve_file_dependencies(index, lookup); 
        resolvers::state::resolve_state_management(index, staging);
        resolvers::frameworks::resolve_middleware_injection(index, staging, lookup, &mut self.cache);
        resolvers::data::resolve_iac_links(index, staging, lookup);
    }

    fn resolve_structure(&self, index: &mut WorkspaceIndex) {
        let mut edges_to_add = Vec::new();
        for sym in index.symbols.values() {
            if let Some(pid) = sym.parent_id {
                edges_to_add.push((pid, sym.id));
            }
        }
        for (p, c) in edges_to_add {
            add_edge(index, p, c, EdgeKind::Contains);
        }
    }
}