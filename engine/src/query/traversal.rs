use std::collections::{HashSet, VecDeque, HashMap};
use crate::models::{Edge, EdgeKind, SymbolId, SymbolKind, WorkspaceIndex, SymbolIndex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SliceDirection {
    Upstream,
    Downstream,
    Both,
}

pub struct GraphWalker<'a> {
    index: &'a WorkspaceIndex,
    reverse_graph: &'a HashMap<SymbolId, Vec<Edge>>,
}

impl<'a> GraphWalker<'a> {
    pub fn new(index: &'a WorkspaceIndex, reverse_graph: &'a HashMap<SymbolId, Vec<Edge>>) -> Self {
        Self { index, reverse_graph }
    }

    pub fn walk_deep(&self, start_ids: &[SymbolId]) -> Vec<SymbolId> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut results = Vec::new();

        for &id in start_ids {
            visited.insert(id);
            queue.push_back(id);
            results.push(id);
        }

        while let Some(current) = queue.pop_front() {
            let current_sym = self.index.symbols.get(&current);
            let is_module = current_sym.map_or(false, |s| s.kind == SymbolKind::Module);
            
            // 1. Go Downstream (Source -> Target)
            if let Some(edges) = self.index.graph.get(&current) {
                for edge in edges {
                    // GATEKEEPER: Stop "File Dump" Explosion.
                    // If we are at a File (Module), do NOT walk down to list all its children (Contains).
                    // We only want to traverse explicit dependencies (Calls, Imports, etc).
                    // We DO allow traversing down from Classes (Containers) to their methods.
                    if is_module && edge.kind == EdgeKind::Contains {
                        continue;
                    }

                    if self.should_follow_downstream(edge.kind) && visited.insert(edge.target_id) {
                        results.push(edge.target_id);
                        queue.push_back(edge.target_id);
                    }
                }
            }

            // 2. Go Upstream (Target -> Source)
            // SAFETY: Do not traverse upstream from External symbols
            let is_external = current_sym.map_or(false, |s| s.is_external);
            if !is_external {
                if let Some(edges) = self.reverse_graph.get(&current) {
                    for edge in edges {
                        if self.should_follow_upstream(edge.kind) && visited.insert(edge.target_id) {
                            results.push(edge.target_id);
                            queue.push_back(edge.target_id);
                        }
                    }
                }
            }
        }
        
        // Sort for deterministic output
        results.sort_by(|a, b| {
            let sym_a = self.index.symbols.get(a).unwrap();
            let sym_b = self.index.symbols.get(b).unwrap();
            sym_a.file_id.cmp(&sym_b.file_id).then(sym_a.range_start.cmp(&sym_b.range_start))
        });

        results
    }

    fn should_follow_downstream(&self, kind: EdgeKind) -> bool {
        match kind {
            EdgeKind::Calls | EdgeKind::Dispatches | EdgeKind::Constructs => true,
            EdgeKind::TypeReference | EdgeKind::Inherits | EdgeKind::Implements => true,
            EdgeKind::Contains | EdgeKind::Injects | EdgeKind::Configures | EdgeKind::Related => true,
            
            EdgeKind::Defines | EdgeKind::Imports | EdgeKind::Handles => false,
        }
    }

    fn should_follow_upstream(&self, kind: EdgeKind) -> bool {
        match kind {
            EdgeKind::Calls | EdgeKind::Constructs => true,
            EdgeKind::Inherits | EdgeKind::Implements | EdgeKind::TypeReference => true,
            EdgeKind::Injects | EdgeKind::Dispatches | EdgeKind::Handles | EdgeKind::Configures | EdgeKind::Related => true,
            
            // ENABLED: Allow finding the parent File/Module of a Function.
            // Essential for providing file context (fixes collision_test).
            EdgeKind::Contains => true,

            // DISABLED: Prevents leaking across shared files in Monorepos.
            // If enabled, SharedFile -> Upstream -> UnrelatedFile.
            EdgeKind::Defines | EdgeKind::Imports => false,
        }
    }
}

pub fn find_call_chain_ids(
    _index: &WorkspaceIndex,
    _target_name: &str,
    _direction: SliceDirection
) -> Option<Vec<SymbolId>> {
    None 
}

/// Decoupled from Indexer: takes only the data structures needed.
pub fn find_related_symbols(
    index: &WorkspaceIndex,
    lookup: &SymbolIndex,
    reverse_graph: &HashMap<SymbolId, Vec<Edge>>,
    target_name: &str
) -> Option<Vec<SymbolId>> {
    let targets = lookup.symbol_map.get(target_name)?;
    if targets.is_empty() {
        return None;
    }

    let walker = GraphWalker::new(index, reverse_graph);
    Some(walker.walk_deep(targets))
}

pub fn generate_context_from_ids(
    index: &WorkspaceIndex,
    chain: &[SymbolId],
    include_docs: bool,
    exclude_tests: bool
) -> String {
    if chain.is_empty() {
        return String::from("// No context found.");
    }

    let filtered_chain: Vec<SymbolId> = if exclude_tests {
        chain
            .iter()
            .filter(|&&id| { index.symbols.get(&id).map_or(true, |s| !s.is_test) })
            .cloned()
            .collect()
    } else {
        chain.to_vec()
    };

    if filtered_chain.is_empty() {
        return String::from("// All relevant context was filtered out (test exclusion active).");
    }

    let mut context = String::new();

    let primary_id = filtered_chain.first().unwrap();
    let primary_name = index.symbols
        .get(primary_id)
        .map(|s| s.name.as_str())
        .unwrap_or("Unknown");

    context.push_str(&format!("// Context for search: `{}`\n", primary_name));

    let names: Vec<String> = filtered_chain
        .iter()
        .filter_map(|id| index.symbols.get(id).map(|s| s.name.clone()))
        .collect();
    context.push_str(&format!("// Resolved Symbols: {}\n", names.join(", ")));

    if exclude_tests {
        context.push_str(
            "// Note: Test files and functions have been excluded from this output.\n"
        );
    }
    context.push('\n');

    let mut seen_files = HashSet::new();

    for &sym_id in &filtered_chain {
        if let Some(sym) = index.symbols.get(&sym_id) {
            if sym.is_external {
                context.push_str("// ==========================================================\n");
                context.push_str(
                    &format!(
                        "// External Library: {}\n",
                        sym.external_source.as_deref().unwrap_or("Unknown")
                    )
                );
                context.push_str("// ==========================================================\n");
                context.push_str(&format!("// Symbol: {}\n", sym.name));

                if let Some(docs) = &sym.doc_comment {
                    context.push_str(&format!("// {}\n", docs));
                }
                context.push_str("\n\n");
                continue;
            }

            if let Some(file_node) = index.files.values().find(|f| f.id == sym.file_id) {
                if !seen_files.contains(&file_node.id) {
                    context.push_str(
                        "// ==========================================================\n"
                    );
                    context.push_str(&format!("// File: {}\n", file_node.path));
                    if file_node.is_test {
                        context.push_str("// (Test File)\n");
                    }
                    context.push_str(
                        "// ==========================================================\n"
                    );
                    seen_files.insert(file_node.id);
                }

                if include_docs {
                    if let Some(docs) = &sym.doc_comment {
                        context.push_str(docs);
                        context.push('\n');
                    }
                }

                if let Ok(content) = std::fs::read_to_string(&file_node.path) {
                    if sym.range_end <= content.len() {
                        let text = String::from_utf8_lossy(
                            &content.as_bytes()[sym.range_start..sym.range_end]
                        );
                        context.push_str(&text);
                    } else {
                        context.push_str(
                            "// Error: Source range out of bounds for this file version"
                        );
                    }
                } else {
                    context.push_str("// Error: Could not read source file from disk");
                }
                context.push_str("\n\n");
            }
        }
    }

    context
}