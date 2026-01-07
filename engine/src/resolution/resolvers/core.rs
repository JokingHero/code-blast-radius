use std::collections::{HashSet, HashMap};
use std::path::Path;
use crate::models::{FileId, SymbolId, WorkspaceIndex, SymbolIndex};
pub type ResolutionCache = HashMap<(FileId, String), Option<SymbolId>>;

pub fn resolve_symbol_across_barrels(
    index: &WorkspaceIndex,
    lookup: &SymbolIndex,
    path_map: &HashMap<std::path::PathBuf, FileId>,
    id_map: &HashMap<FileId, std::path::PathBuf>,
    active_roots: &Vec<std::path::PathBuf>,
    cache: &mut ResolutionCache,
    target_file_id: FileId,
    symbol_name: &str,
    visited: &mut HashSet<FileId>
) -> Option<SymbolId> {
    if visited.contains(&target_file_id) { return None; }
    visited.insert(target_file_id);
    
    let cache_key = (target_file_id, symbol_name.to_string());
    if let Some(&cached_res) = cache.get(&cache_key) {
        return cached_res;
    }

    let mut result = None;
    
    // 1. Check direct definition
    if let Some(ids) = lookup.symbol_map.get(symbol_name) {
        if let Some(&id) = ids.iter().find(|&&id| index.symbols[&id].file_id == target_file_id) {
            result = Some(id);
        }
    }

    // 2. Check re-exports
    if result.is_none() {
        if let Some(exports) = lookup.file_exports.get(&target_file_id).cloned() {
            // Named re-exports
            for exp in exports.iter().filter(|e| e.name.as_deref() == Some(symbol_name)) {
                if let Some(next_file_id) = resolve_import_path(index, lookup, path_map, id_map, active_roots, target_file_id, &exp.source) {
                    result = resolve_symbol_across_barrels(index, lookup, path_map, id_map, active_roots, cache, next_file_id, symbol_name, visited);
                    if result.is_some() { break; }
                }
            }
            // Star re-exports
            if result.is_none() {
                for exp in exports.iter().filter(|e| e.name.is_none()) {
                    if let Some(next_file_id) = resolve_import_path(index, lookup, path_map, id_map, active_roots, target_file_id, &exp.source) {
                        result = resolve_symbol_across_barrels(index, lookup, path_map, id_map, active_roots, cache, next_file_id, symbol_name, visited);
                        if result.is_some() { break; }
                    }
                }
            }
        }
    }
    
    cache.insert(cache_key, result);
    result
}

pub fn resolve_single_call(
    index: &WorkspaceIndex,
    lookup: &SymbolIndex,
    path_map: &HashMap<std::path::PathBuf, FileId>, 
    id_map: &HashMap<FileId, std::path::PathBuf>,
    active_roots: &Vec<std::path::PathBuf>,
    cache: &mut ResolutionCache,
    file_id: FileId,
    name: &str
) -> Option<SymbolId> {
    // 1. Check imports explicitly
    if let Some(imps) = lookup.file_imports.get(&file_id) {
        for imp in imps {
            if imp.alias.as_ref().unwrap_or(&imp.name) == name {
                // A. Try Local Resolution
                if let Some(target_file_id) = resolve_import_path(index, lookup, path_map, id_map, active_roots, file_id, &imp.source) {
                    let mut visited = HashSet::new();
                    if let Some(found) = resolve_symbol_across_barrels(index, lookup, path_map, id_map, active_roots, cache, target_file_id, &imp.name, &mut visited) {
                        return Some(found);
                    }
                }

                // B. Try External Resolution
                if let Some(candidates) = lookup.symbol_map.get(&imp.name) {
                    for &candidate_id in candidates {
                        let symbol = &index.symbols[&candidate_id];
                        if symbol.is_external && symbol.external_source.as_deref() == Some(imp.source.as_str()) {
                            return Some(candidate_id);
                        }
                    }
                }
            }
        }
    }

    // 2. Check local file definitions
    if let Some(ids) = lookup.symbol_map.get(name) {
        if let Some(&id) = ids.iter().find(|&&id| index.symbols[&id].file_id == file_id) {
            return Some(id);
        }
    }

    None
}

pub fn resolve_import_path(
    _index: &WorkspaceIndex,
    lookup: &SymbolIndex,
    path_map: &HashMap<std::path::PathBuf, FileId>,
    id_map: &HashMap<FileId, std::path::PathBuf>,
    active_roots: &Vec<std::path::PathBuf>,
    from_id: FileId,
    source: &str
) -> Option<FileId> {
    // We need to find the absolute path of the 'from' file to resolve relative imports.
    // Use O(1) lookup via id_map
    let from_path = id_map.get(&from_id)?;

    // 1. Relative Imports
    if source.starts_with("./") || source.starts_with("../") {
        let parent = from_path.parent()?;
        let base = parent.join(source);
        return check_path_variants(path_map, &base);
    }

    // 2. Rust Crate Alias
    if source.starts_with("crate::") {
        let relative_part = source.replace("crate::", "src/").replace("::", "/");
        
        // Attempt to anchor this relative path against all active roots
        for root in active_roots {
            let candidate = root.join(&relative_part);
            if let Some(id) = check_path_variants(path_map, &candidate) {
                return Some(id);
            }
        }
        
        // Fallback: Check without anchoring (unlikely to work for relative paths in path_map)
        return check_path_variants(path_map, Path::new(&relative_part));
    }

    // 3. Monorepo / Workspace Packages
    for (pkg_name, pkg_root_str) in &lookup.package_path_map {
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
            if let Some(id) = check_path_variants(path_map, &target_path) {
                return Some(id);
            }
        }
    }

    // 4. Aliases (tsconfig, etc)
    for (alias_key, alias_target) in &lookup.import_mappings {
        if source.starts_with(alias_key) {
            let relative_part = source.replace(alias_key, alias_target);
            
            // Optimization: Instead of iterating path_map, check against every root
            for root in active_roots {
                let candidate = root.join(&relative_part);
                if let Some(id) = check_path_variants(path_map, &candidate) {
                    return Some(id);
                }
            }
            continue;
        }
    }

    // 5. Absolute / Root-Relative
    if let Some(id) = check_path_variants(path_map, Path::new(source)) {
        return Some(id);
    }

    // 6. Fuzzy Suffix Match (The O(N) Fallback)
    if !lookup.external_packages.contains(&source.split('/').next().unwrap_or("").to_string()) {
        let matches: Vec<FileId> = path_map.iter()
            .filter(|(path, _)| {
                let p = path.to_string_lossy().replace('\\', "/");
                p.contains(source) && 
                (p.ends_with(&format!("/{}.ts", source)) || 
                p.ends_with(&format!("/{}.js", source)) ||
                p.ends_with(&format!("/{}.rs", source)) ||
                p.ends_with(&format!("/{}.py", source)) ||
                p.ends_with(&format!("/{}", source)))
            })
            .map(|(_, &id)| id)
            .collect();

        if matches.len() == 1 {
            return Some(matches[0]);
        }
    }

    None
}

fn check_path_variants(path_map: &HashMap<std::path::PathBuf, FileId>, base: &Path) -> Option<FileId> {
    let exts = ["ts", "js", "tsx", "jsx", "rs", "py", "json", "sh", "java"];
    
    let check = |candidate: &Path| -> Option<FileId> {
        // Try direct lookup first (Fastest)
        if let Some(&id) = path_map.get(candidate) {
            return Some(id);
        }
        
        // Try canonicalizing if path exists on disk (Slow IO, but handles .. and symlinks)
        if let Ok(canon) = std::fs::canonicalize(candidate) {
             if let Some(&id) = path_map.get(&canon) {
                return Some(id);
            }
        }
        None
    };

    let mut candidates = Vec::new();
    candidates.push(base.to_path_buf());

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