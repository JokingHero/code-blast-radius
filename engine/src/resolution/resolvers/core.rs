use std::collections::HashSet;
use std::path::Path;
use crate::resolution::{Indexer, utils};
use crate::models::{FileId, SymbolId};

impl Indexer {
    /// Recursively walks barrel files (re-exports) to find the actual definition.
    pub(crate) fn resolve_symbol_across_barrels(
        &mut self,
        target_file_id: FileId,
        symbol_name: &str,
        visited: &mut HashSet<FileId>
    ) -> Option<SymbolId> {
        if visited.contains(&target_file_id) { return None; }
        visited.insert(target_file_id);
        
        let cache_key = (target_file_id, symbol_name.to_string());
        if let Some(&cached_res) = self.resolution_cache.get(&cache_key) {
            return cached_res;
        }

        let mut result = None;
        
        // 1. Check direct definition via lookup
        if let Some(ids) = self.index.lookup.symbol_map.get(symbol_name) {
            if let Some(&id) = ids.iter().find(|&&id| self.index.symbols[&id].file_id == target_file_id) {
                result = Some(id);
            }
        }

        // 2. Check re-exports via lookup
        if result.is_none() {
            if let Some(exports) = self.index.lookup.file_exports.get(&target_file_id).cloned() {
                // Named re-exports: export { Foo } from './bar'
                for exp in exports.iter().filter(|e| e.name.as_deref() == Some(symbol_name)) {
                    if let Some(next_file_id) = self.resolve_import_path(target_file_id, &exp.source) {
                        result = self.resolve_symbol_across_barrels(next_file_id, symbol_name, visited);
                        if result.is_some() { break; }
                    }
                }
                // Star re-exports: export * from './bar'
                if result.is_none() {
                    for exp in exports.iter().filter(|e| e.name.is_none()) {
                        if let Some(next_file_id) = self.resolve_import_path(target_file_id, &exp.source) {
                            result = self.resolve_symbol_across_barrels(next_file_id, symbol_name, visited);
                            if result.is_some() { break; }
                        }
                    }
                }
            }
        }
        
        self.resolution_cache.insert(cache_key, result);
        result
    }

    /// The core glue: resolves a symbol name in the context of a file
    pub(crate) fn resolve_single_call(&mut self, file_id: FileId, name: &str) -> Option<SymbolId> {
        // 1. Check imports explicitly via lookup
        if let Some(imps) = self.index.lookup.file_imports.get(&file_id).cloned() {
            for imp in imps {
                if imp.alias.as_ref().unwrap_or(&imp.name) == name {
                    // A. Try Local Resolution
                    if let Some(tfid) = self.resolve_import_path(file_id, &imp.source) {
                        let mut visited = HashSet::new();
                        if let Some(found) = self.resolve_symbol_across_barrels(tfid, &imp.name, &mut visited) {
                            return Some(found);
                        }
                    }

                    // B. Try External Resolution
                    if let Some(candidates) = self.index.lookup.symbol_map.get(&imp.name) {
                        for &cid in candidates {
                            let s = &self.index.symbols[&cid];
                            if s.is_external && s.external_source.as_deref() == Some(imp.source.as_str()) {
                                return Some(cid);
                            }
                        }
                    }
                }
            }
        }

        // 2. Check local file definitions via lookup
        if let Some(ids) = self.index.lookup.symbol_map.get(name) {
            if let Some(&id) = ids.iter().find(|&&id| self.index.symbols[&id].file_id == file_id) {
                return Some(id);
            }
        }

        None
    }

    pub(crate) fn resolve_import_path(&self, from_id: FileId, source: &str) -> Option<FileId> {
        let from_path_str = &self.index.files.values().find(|f| f.id == from_id)?.path;
        let from_path = Path::new(from_path_str);
        
        // 1. Relative Imports
        if source.starts_with("./") || source.starts_with("../") {
            let parent = from_path.parent()?;
            let base = parent.join(source);
            return self.check_path_variants(&base);
        }

        // 2. Rust Crate Alias
        if source.starts_with("crate::") {
            let relative = source.replace("crate::", "src/").replace("::", "/");
            return self.check_path_variants(Path::new(&relative));
        }

        // 3. Monorepo / Workspace Packages
        for (pkg_name, pkg_root_str) in &self.index.lookup.package_path_map {
            let is_exact = source == pkg_name;
            let is_subpath = source.starts_with(pkg_name) && source.as_bytes().get(pkg_name.len()) == Some(&b'/');

            if is_exact || is_subpath {
                let pkg_root = Path::new(pkg_root_str);
                let target_path = if is_exact {
                    pkg_root.to_path_buf()
                } else {
                    let suffix = &source[pkg_name.len() + 1..];
                    pkg_root.join(suffix)
                };
                if let Some(id) = self.check_path_variants(&target_path) {
                    return Some(id);
                }
            }
        }

        // 4. Aliases (tsconfig, etc)
        for (alias_key, alias_target) in &self.index.lookup.import_mappings {
            if source.starts_with(alias_key) {
                let replaced = source.replace(alias_key, alias_target);
                if let Some(id) = self.check_path_variants(Path::new(&replaced)) {
                    return Some(id);
                }
            }
        }

        // 5. Absolute / Root-Relative
        if let Some(id) = self.check_path_variants(Path::new(source)) {
            return Some(id);
        }

        // 6. Fuzzy Suffix Match
        if !self.index.lookup.external_packages.contains(&source.split('/').next().unwrap_or("").to_string()) {
            let matches: Vec<FileId> = self.index.files.values()
                .filter(|f| {
                    let p = f.path.replace('\\', "/");
                    p.contains(source) && 
                    (p.ends_with(&format!("/{}.ts", source)) || 
                     p.ends_with(&format!("/{}.js", source)) ||
                     p.ends_with(&format!("/{}.rs", source)) ||
                     p.ends_with(&format!("/{}.py", source)) ||
                     p.ends_with(&format!("/{}", source)))
                })
                .map(|f| f.id)
                .collect();

            if matches.len() == 1 {
                return Some(matches[0]);
            }
        }

        None
    }

    fn check_path_variants(&self, base: &Path) -> Option<FileId> {
        let exts = ["ts", "js", "tsx", "jsx", "rs", "py", "json", "sh", "java"];
        
        let check = |candidate: &Path| -> Option<FileId> {
            let key = utils::to_index_path(candidate);
            if let Some(node) = self.index.files.get(&key) {
                return Some(node.id);
            }
            None
        };

        let mut candidates = Vec::new();
        candidates.push(base.to_path_buf());
        
        if base.is_relative() {
            for root in &self.index.roots {
                candidates.push(Path::new(root).join(base));
            }
        }

        for path in candidates {
            if let Some(id) = check(&path) { return Some(id); }
            for e in &exts {
                if let Some(id) = check(&path.with_extension(e)) { return Some(id); }
            }
            for e in &exts {
                if let Some(id) = check(&path.join(format!("index.{}", e))) { return Some(id); }
            }
            if let Some(id) = check(&path.join("mod.rs")) { return Some(id); }
        }

        None
    }
}