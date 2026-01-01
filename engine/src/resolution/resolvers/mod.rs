pub mod core;
pub mod standard;
pub mod inference;
pub mod frameworks;
pub mod state;
pub mod data;
pub mod constants;

use crate::models::{Edge, EdgeKind, SymbolId, WorkspaceIndex, SymbolKind};

// Shared helper to avoid code duplication in resolvers
pub fn add_edge(index: &mut WorkspaceIndex, source: SymbolId, target: SymbolId, kind: EdgeKind) {
    if source == target { return; }
    let edges = index.graph.entry(source).or_default();
    for edge in edges.iter() {
        if edge.target_id == target && edge.kind == kind { return; }
    }
    edges.push(Edge { target_id: target, kind });
}

// Shared helper for linking modules
pub fn link_modules(index: &mut WorkspaceIndex, file_a: u32, file_b: u32) {
    let mod_a = index.symbols.values().find(|s| s.file_id == file_a && s.kind == SymbolKind::Module).map(|s| s.id);
    let mod_b = index.symbols.values().find(|s| s.file_id == file_b && s.kind == SymbolKind::Module).map(|s| s.id);
    if let (Some(ma), Some(mb)) = (mod_a, mod_b) {
        add_edge(index, ma, mb, EdgeKind::Imports);
        add_edge(index, mb, ma, EdgeKind::Imports);
    }
}