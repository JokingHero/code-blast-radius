pub mod core;
pub mod standard;
pub mod inference;
pub mod frameworks;
pub mod state;
pub mod data;

use crate::{models::SymbolKind, resolution::Indexer};

impl Indexer {
    // Shared helper for linking modules in file dependencies
    pub(crate) fn link_modules(&mut self, file_a: u32, file_b: u32) {
        let mod_a = self.index.symbols.values().find(|s| s.file_id == file_a && s.kind == SymbolKind::Module).map(|s| s.id);
        let mod_b = self.index.symbols.values().find(|s| s.file_id == file_b && s.kind == SymbolKind::Module).map(|s| s.id);
        if let (Some(ma), Some(mb)) = (mod_a, mod_b) {
            let calls_a = self.index.resolved_calls.entry(ma).or_default();
            if !calls_a.contains(&mb) { calls_a.push(mb); }
            let calls_b = self.index.resolved_calls.entry(mb).or_default();
            if !calls_b.contains(&ma) { calls_b.push(ma); }
        }
    }
}