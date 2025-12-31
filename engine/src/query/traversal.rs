use std::collections::{HashSet, VecDeque, HashMap};
use crate::models::{Edge, EdgeKind, SymbolId, SymbolKind, WorkspaceIndex};

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
            let is_external = current_sym.map_or(false, |s| s.is_external);
            let is_module = current_sym.map_or(false, |s| s.kind == SymbolKind::Module);

            // 1. Go Downstream (Source -> Target)
            if let Some(edges) = self.index.graph.get(&current) {
                for edge in edges {
                    // LOGIC CHECK: Sibling Pollution
                    // If we are at a File (Module), we generally do NOT want to grab every 
                    // single function in that file unless there is a specific call/reference.
                    // We skip structural `Contains` edges going down from a Module.
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
            // SAFETY: Do not traverse upstream from External symbols (e.g. don't index all of React just because I use it)
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
            // Logic & Flow
            EdgeKind::Calls => true, 
            EdgeKind::Dispatches => true, 
            EdgeKind::Constructs => true,
            
            // Type System
            EdgeKind::TypeReference => true,
            EdgeKind::Inherits => true,
            EdgeKind::Implements => true, 
            
            // Structure & DI
            EdgeKind::Contains => true, // (Filtered conditionally for Modules above)
            EdgeKind::Injects => true,
            EdgeKind::Configures => true, 
            
            // Meta
            EdgeKind::Defines => false, // File -> Module (Keep 1:1)
            EdgeKind::Imports => false, // Don't traverse file imports deeply downstream
            EdgeKind::Handles => false, // Reducer handles Action (Usually upstream logic)
            EdgeKind::Related => true,
        }
    }

    fn should_follow_upstream(&self, kind: EdgeKind) -> bool {
        match kind {
            // "Who calls me?"
            EdgeKind::Calls => true,     
            EdgeKind::Constructs => true,

            // "Who implements/inherits me?"
            EdgeKind::Inherits => true,  
            EdgeKind::Implements => true, 
            
            // "Who uses me as a type?"
            EdgeKind::TypeReference => true,
            
            // "Who contains me?" (Module/Class)
            EdgeKind::Contains => true,
            
            // "Who injects me?"
            EdgeKind::Injects => true,

            // "Who dispatches this action?" (If I am the handler)
            // "Who handles this action?" (If I am the dispatcher - though usually handled by downstream)
            EdgeKind::Dispatches => true, 
            EdgeKind::Handles => true,   
            
            // Config
            EdgeKind::Configures => true, 

            // Explicitly ignored
            EdgeKind::Defines => false,
            EdgeKind::Imports => false, // Don't traverse upstream imports (Impact Analysis handles this separately)
            EdgeKind::Related => true,
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

pub fn find_related_symbols(indexer: &crate::resolution::Indexer, target_name: &str) -> Option<Vec<SymbolId>> {
    let targets = indexer.index.symbol_map.get(target_name)?;
    if targets.is_empty() {
        return None;
    }

    let walker = GraphWalker::new(&indexer.index, &indexer.reverse_graph);
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