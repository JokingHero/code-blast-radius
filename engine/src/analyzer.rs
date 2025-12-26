use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};

use crate::language::{get_language, LanguageConfig};
use crate::schema::{ImportNode, ExportNode, WorkspaceIndex, SymbolId};

#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub name: String,
    pub is_anonymous: bool, // Added to distinguish symbols
    pub range_start: usize,
    pub range_end: usize,
    pub source_code: String,
    pub documentation: Option<String>,
    pub calls: Vec<String>, 
    pub fingerprints: HashMap<String, Vec<String>>,
    pub return_type: Option<String>, 
    pub local_types: HashMap<String, String>, 
    pub local_assigns: HashMap<String, String>, 
    pub config_keys: Vec<String>,
}

pub struct FileAnalysis {
    pub functions: Vec<FunctionInfo>,
    pub imports: Vec<ImportNode>,
    pub exports: Vec<ExportNode>,
    pub literals: Vec<String>,
    pub implementations: Vec<(String, String)>, 
    pub global_vars: HashMap<String, String>, 
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SliceDirection {
    Upstream,
    Downstream,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum TraversalMode {
    Downstream,
    Upstream,
    Both,
}

pub fn analyze_source(
    path: &Path, // We use this now for the module name
    source_code: &str,
    config: &LanguageConfig,
) -> Result<FileAnalysis, String> {
    let code_bytes = source_code.as_bytes();
    let language = get_language(config.lang_enum);

    let mut parser = Parser::new();
    parser.set_language(&language).map_err(|e| e.to_string())?;
    
    if source_code.trim().is_empty() {
        return Ok(FileAnalysis { 
            functions: vec![], imports: vec![], exports: vec![], 
            literals: vec![], implementations: vec![], global_vars: HashMap::new() 
        });
    }

    let tree = parser.parse(source_code, None).ok_or("Failed to parse code")?;
    let root_node = tree.root_node();
    
    // --- STEP 0: CONSTANT PROPAGATION ---
    // We scan for constants first (e.g., const API_URL = "/api/v1") so we can 
    // substitute them in imports or inject them as literals later.
    let mut local_constants: HashMap<String, String> = HashMap::new();
    
    if !config.query_vals.is_empty() {
         if let Ok(q) = Query::new(&language, config.query_vals) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&q, root_node, code_bytes);
            while let Some(m) = matches.next() {
                let mut name = String::new();
                let mut val = String::new();
                
                for cap in m.captures {
                    let text = cap.node.utf8_text(code_bytes).unwrap_or("").to_string();
                    let capture_name = q.capture_names()[cap.index as usize];
                    
                    if capture_name == "val.name" { 
                        name = text; 
                    } else if capture_name == "val.value" { 
                        // Strip quotes immediately to store the semantic value
                        val = text.trim_matches(|c| c == '"' || c == '\'' || c == '`').to_string(); 
                    }
                }
                
                if !name.is_empty() && !val.is_empty() {
                    local_constants.insert(name, val);
                }
            }
        }
    }

    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut literals = Vec::new();
    let mut implementations = Vec::new();
    let mut functions = Vec::new();

    // 1. Imports (Enhanced with Substitution)
    if !config.query_imports.is_empty() {
        if let Ok(q) = Query::new(&language, config.query_imports) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&q, root_node, code_bytes);
            while let Some(m) = matches.next() {
                let mut src = String::new();
                let mut name = String::new();
                let mut alias = None;
                
                for cap in m.captures {
                    let text = cap.node.utf8_text(code_bytes).unwrap_or("").to_string();
                    let capture_name = q.capture_names()[cap.index as usize];
                    
                    if capture_name == "import.source" { 
                        // Logic:
                        // 1. If 'text' matches a known constant (e.g. require(MY_CONST)), use the constant's value.
                        // 2. Otherwise, treat it as a literal and strip quotes.
                        if let Some(resolved) = local_constants.get(&text) {
                            src = resolved.clone();
                        } else {
                            src = text.replace(['"', '\''], ""); 
                        }
                    } else if capture_name == "import.name" { 
                        name = text; 
                    } else if capture_name == "import.alias" {
                        // Handle namespace import
                        name = "*".to_string(); // Magic string for namespace
                        alias = Some(text);
                    }
                }
                if !src.is_empty() { 
                    imports.push(ImportNode { name, source: src, alias }); 
                }
            }
        }
    }

    // 2. Exports
    if !config.query_exports.is_empty() {
        if let Ok(q) = Query::new(&language, config.query_exports) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&q, root_node, code_bytes);
            while let Some(m) = matches.next() {
                let mut src = String::new();
                let mut name = None;
                for cap in m.captures {
                    let text = cap.node.utf8_text(code_bytes).unwrap_or("").to_string();
                    let capture_name = q.capture_names()[cap.index as usize];
                    if capture_name == "export.source" { 
                        // Similar substitution logic for exports could go here, 
                        // generally exports use literals, but substitution is safe.
                        if let Some(resolved) = local_constants.get(&text) {
                            src = resolved.clone();
                        } else {
                            src = text.replace(['"', '\''], ""); 
                        }
                    }
                    else if capture_name == "export.name" { name = Some(text); }
                }
                if !src.is_empty() { exports.push(ExportNode { name, source: src }); }
            }
        }
    }

    // 3. Literals (Enhanced with Injection)
    if !config.query_literals.is_empty() {
        if let Ok(q) = Query::new(&language, config.query_literals) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&q, root_node, code_bytes);
            while let Some(m) = matches.next() {
                for cap in m.captures {
                    let text = cap.node.utf8_text(code_bytes).unwrap_or("").trim_matches(|c| c == '"' || c == '\'' || c == '`').to_string();
                    if text.len() > 1 { literals.push(text); }
                }
            }
        }
    }
    
    // INJECTION: Add the values of constants found in Step 0.
    // This ensures that if a user writes `const API = "/users"`, the indexer
    // sees "/users" as a literal present in this file for linking purposes.
    for val in local_constants.values() {
        if val.len() > 1 {
            literals.push(val.clone());
        }
    }

    // 4. Implementations
    if !config.query_implements.is_empty() {
        if let Ok(q) = Query::new(&language, config.query_implements) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&q, root_node, code_bytes);
            while let Some(m) = matches.next() {
                let mut child = String::new();
                let mut parent = String::new();
                for cap in m.captures {
                    let name = q.capture_names()[cap.index as usize];
                    let text = cap.node.utf8_text(code_bytes).unwrap_or("").to_string();
                    if name == "impl.child" { child = text; } 
                    else if name == "impl.parent" { parent = text; }
                }
                if !child.is_empty() && !parent.is_empty() { implementations.push((child, parent)); }
            }
        }
    }

    // 5. Build Queries
    let defs_query = Query::new(&language, config.query_defs)
        .map_err(|e| format!("Invalid defs query for {:?}: {}", config.lang_enum, e))?;
    let calls_query = Query::new(&language, config.query_calls)
        .map_err(|e| format!("Invalid calls query for {:?}: {}", config.lang_enum, e))?;
    let docs_query = Query::new(&language, config.query_docs)
        .map_err(|e| format!("Invalid docs query for {:?}: {}", config.lang_enum, e))?;
    let config_query = if !config.query_config.is_empty() {
        Some(Query::new(&language, config.query_config)
            .map_err(|e| format!("Invalid config query for {:?}: {}", config.lang_enum, e))?)
    } else {
        None
    };

    // --- Create the Module Symbol ---
    // This represents the file itself and will catch top-level calls/configs
    let module_name = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let mut module_info = FunctionInfo {
        name: format!("(module) {}", module_name),
        is_anonymous: false,
        range_start: root_node.start_byte(),
        range_end: root_node.end_byte(),
        source_code: String::new(), // Optional: Don't duplicate source to save RAM, or copy it if needed
        documentation: None, // We could parse top-level file comments here if desired
        calls: Vec::new(),
        fingerprints: HashMap::new(),
        return_type: None,
        local_types: HashMap::new(),
        local_assigns: HashMap::new(),
        config_keys: Vec::new(),
    };

    // 6. Extract Definitions
    let mut variable_hints = Vec::new(); 
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&defs_query, root_node, code_bytes);

    while let Some(match_) = matches.next() {
        let mut def_node = None;
        let mut name_opt: Option<String> = None;
        let mut return_type = None;
        let mut v_name = None;
        let mut v_type = None;
        let mut v_assign = None;

        for capture in match_.captures {
            let cap_name = defs_query.capture_names()[capture.index as usize];
            let text = capture.node.utf8_text(code_bytes).unwrap_or("");
            
            match cap_name {
                "function.definition" => def_node = Some(capture.node),
                "function.name" => name_opt = Some(text.to_string()),
                "function.return_type" => {
                    return_type = Some(text.trim_start_matches(|c| c == ':' || c == '=' || c == '>').trim().to_string());
                }
                "variable.name" => {
                    v_name = Some(text.to_string());
                    // Try to sniff what variable is assigned to: const x = someFunc()
                    if let Some(parent) = capture.node.parent() {
                        if let Some(val) = parent.child_by_field_name("value") {
                            if val.kind() == "call_expression" {
                                if let Some(f) = val.child_by_field_name("function") {
                                    let fn_name = if f.kind() == "member_expression" {
                                        f.child_by_field_name("property").and_then(|p| p.utf8_text(code_bytes).ok()).unwrap_or("")
                                    } else { f.utf8_text(code_bytes).unwrap_or("") };
                                    if !fn_name.is_empty() { v_assign = Some(fn_name.to_string()); }
                                }
                            }
                        }
                    }
                }
                "variable.type" => {
                    v_type = Some(text.trim_start_matches(':').trim().to_string());
                }
                _ => {}
            }
        }

        if let Some(node) = def_node {
            functions.push(FunctionInfo {
                name: name_opt.clone().unwrap_or_else(|| "anonymous".to_string()),
                is_anonymous: name_opt.is_none(),
                range_start: node.start_byte(),
                range_end: node.end_byte(),
                source_code: node.utf8_text(code_bytes).unwrap_or("").to_string(),
                documentation: None,
                calls: Vec::new(),
                fingerprints: HashMap::new(),
                return_type,
                local_types: HashMap::new(),
                local_assigns: HashMap::new(),
                config_keys: Vec::new(),
            });
        } else if let Some(vn) = v_name {
            // Collecting variable hints to distribute later
            variable_hints.push((match_.captures[0].node.byte_range(), vn, v_type, v_assign));
        }
    }

    // Helper closure to find the "Smallest Container" for a given range
    // Returns index in `functions` or None if it belongs to `module_info`
    let get_owner_index = |start: usize, end: usize, funcs: &[FunctionInfo]| -> Option<usize> {
        let mut best_idx = None;
        let mut smallest_len = usize::MAX;

        for (i, func) in funcs.iter().enumerate() {
            if start >= func.range_start && end <= func.range_end {
                let len = func.range_end - func.range_start;
                if len < smallest_len {
                    smallest_len = len;
                    best_idx = Some(i);
                }
            }
        }
        best_idx
    };

    // 7. Distribute Variable Hints
    for (v_range, v_name, v_type, v_assign) in variable_hints {
        if let Some(idx) = get_owner_index(v_range.start, v_range.end, &functions) {
            let func = &mut functions[idx];
            if let Some(t) = v_type { func.local_types.insert(v_name.clone(), t); }
            if let Some(a) = v_assign { func.local_assigns.insert(v_name.clone(), a); }
        } else {
            // Add to Module
            if let Some(t) = v_type { module_info.local_types.insert(v_name.clone(), t); }
            if let Some(a) = v_assign { module_info.local_assigns.insert(v_name.clone(), a); }
        }
    }

    // 8. Distribute Config Keys
    if let Some(ref q) = config_query {
        let mut cf_cursor = QueryCursor::new();
        let mut cf_matches = cf_cursor.matches(q, root_node, code_bytes);
        while let Some(cfm) = cf_matches.next() {
            for cap in cfm.captures {
                if q.capture_names()[cap.index as usize] == "config.key" {
                    let text = cap.node.utf8_text(code_bytes)
                        .unwrap_or("")
                        .trim_matches(|c| c == '"' || c == '\'' || c == '`')
                        .to_string();
                    
                    if !text.is_empty() {
                        let range = cap.node.byte_range();
                        if let Some(idx) = get_owner_index(range.start, range.end, &functions) {
                            functions[idx].config_keys.push(text);
                        } else {
                            module_info.config_keys.push(text);
                        }
                    }
                }
            }
        }
    }

    // Deduplicate config keys
    for func in &mut functions {
        func.config_keys.sort();
        func.config_keys.dedup();
    }
    module_info.config_keys.sort();
    module_info.config_keys.dedup();

    // 9. Extract and Distribute Calls (Global Pass)
    let mut c_cursor = QueryCursor::new();
    let mut c_matches = c_cursor.matches(&calls_query, root_node, code_bytes);
    
    while let Some(cm) = c_matches.next() {
        let mut m_name = None;
        let mut r_name = None;
        let mut call_range = None;

        for cp in cm.captures {
            let t = cp.node.utf8_text(code_bytes).unwrap_or("").to_string();
            let cap_name = calls_query.capture_names()[cp.index as usize];
            if cap_name == "call.name" { 
                m_name = Some(t); 
                call_range = Some(cp.node.byte_range());
            }
            else if cap_name == "call.receiver" { r_name = Some(t); }
        }

        if let (Some(m), Some(range)) = (m_name, call_range) {
            if let Some(idx) = get_owner_index(range.start, range.end, &functions) {
                let func = &mut functions[idx];
                func.calls.push(m.clone());
                if let Some(r) = r_name {
                    func.fingerprints.entry(r).or_default().push(m);
                }
            } else {
                // Top-Level Call -> Module
                module_info.calls.push(m.clone());
                if let Some(r) = r_name {
                    module_info.fingerprints.entry(r).or_default().push(m);
                }
            }
        }
    }

    // 10. Extract Docs (Only for Defined Functions)
    let mut d_cursor = QueryCursor::new();
    let mut d_matches = d_cursor.matches(&docs_query, root_node, code_bytes);
    while let Some(dm) = d_matches.next() {
        let d_def = dm.captures.iter()
            .find(|c| docs_query.capture_names()[c.index as usize] == "function.definition")
            .map(|c| c.node);
        
        if let Some(d_node) = d_def {
            // Find which function matches this definition node
            for func in &mut functions {
                if func.range_start == d_node.start_byte() {
                    func.documentation = Some(dm.captures.iter()
                        .filter(|c| docs_query.capture_names()[c.index as usize] == "function.docs")
                        .map(|c| c.node.utf8_text(code_bytes).unwrap_or("").to_string())
                        .collect::<Vec<_>>().join("\n"));
                    break;
                }
            }
        }
    }

    // 11. Final Assembly
    functions.push(module_info);

    Ok(FileAnalysis { 
        functions, imports, exports, literals, implementations, global_vars: HashMap::new() 
    })
}

pub fn find_call_chain_ids(
    index: &WorkspaceIndex, 
    target_name: &str,
    direction: SliceDirection
) -> Option<Vec<SymbolId>> {
    let targets = index.symbol_map.get(target_name)?;
    if targets.is_empty() { return None; }

    let mut predecessors: HashMap<SymbolId, SymbolId> = HashMap::new();
    let mut queue: VecDeque<SymbolId> = VecDeque::new();
    let mut visited: HashSet<SymbolId> = HashSet::new();

    for &id in targets {
        queue.push_back(id);
        visited.insert(id);
    }

    while let Some(current_id) = queue.pop_front() {
        if direction == SliceDirection::Upstream || direction == SliceDirection::Both {
            for (caller_id, callees) in &index.resolved_calls {
                if callees.contains(&current_id) && !visited.contains(caller_id) {
                    visited.insert(*caller_id);
                    predecessors.insert(*caller_id, current_id); 
                    queue.push_back(*caller_id);
                }
            }
        }

        if direction == SliceDirection::Downstream || direction == SliceDirection::Both {
            if let Some(callees) = index.resolved_calls.get(&current_id) {
                for &callee_id in callees {
                    if !visited.contains(&callee_id) {
                        visited.insert(callee_id);
                        predecessors.insert(callee_id, current_id);
                        queue.push_back(callee_id);
                    }
                }
            }
        }

        if let Some(children) = index.inheritance.get(&current_id) {
            for &child_id in children {
                if !visited.contains(&child_id) {
                    visited.insert(child_id);
                    predecessors.insert(child_id, current_id); 
                    queue.push_back(child_id);
                }
            }
        }
    }

    let mut final_list: Vec<SymbolId> = visited.into_iter().collect();
    final_list.sort_by_key(|id| {
        let mut depth = 0;
        let mut curr = *id;
        while let Some(&p) = predecessors.get(&curr) {
            depth += 1;
            curr = p;
        }
        depth
    });

    if direction == SliceDirection::Downstream { final_list.reverse(); }
    Some(final_list)
}

pub fn generate_context_from_ids(
    index: &WorkspaceIndex,
    chain: &[SymbolId],
    include_docs: bool,
    exclude_tests: bool,
) -> String {
    if chain.is_empty() {
        return String::from("// No context found.");
    }

    // 1. Filter the chain first
    let filtered_chain: Vec<SymbolId> = if exclude_tests {
        chain
            .iter()
            .filter(|&&id| {
                index.symbols.get(&id).map_or(true, |s| !s.is_test)
            })
            .cloned()
            .collect()
    } else {
        chain.to_vec()
    };

    if filtered_chain.is_empty() {
        return String::from("// All relevant context was filtered out (test exclusion active).");
    }

    let mut context = String::new();
    
    // 2. Metadata Header
    // Use the first symbol in the FILTERED chain as the primary context reference
    let primary_id = filtered_chain.first().unwrap();
    let primary_name = index.symbols.get(primary_id).map(|s| s.name.as_str()).unwrap_or("Unknown");

    context.push_str(&format!("// Context for search: `{}`\n", primary_name));
    
    let names: Vec<String> = filtered_chain.iter()
        .filter_map(|id| index.symbols.get(id).map(|s| s.name.clone()))
        .collect();
    context.push_str(&format!("// Resolved Symbols: {}\n", names.join(", ")));
    
    if exclude_tests {
        context.push_str("// Note: Test files and functions have been excluded from this output.\n");
    }
    context.push('\n');

    // 3. Extraction logic
    let mut seen_files = HashSet::new();

    for &sym_id in &filtered_chain {
        if let Some(sym) = index.symbols.get(&sym_id) {
            
            // --- NEW: Handle External Symbols (Boundary Context) ---
            if sym.is_external {
                context.push_str("// ==========================================================\n");
                context.push_str(&format!("// External Library: {}\n", sym.external_source.as_deref().unwrap_or("Unknown")));
                context.push_str("// ==========================================================\n");
                context.push_str(&format!("// Symbol: {}\n", sym.name));
                
                if let Some(docs) = &sym.doc_comment {
                    context.push_str(&format!("// {}\n", docs));
                }
                
                context.push_str("// (Source code not available for external libraries)\n");
                context.push_str("\n\n");
                continue; // Skip file reading logic for external symbols
            }

            // --- EXISTING: Handle Local Symbols ---
            if let Some(file_node) = index.files.values().find(|f| f.id == sym.file_id) {
                // Print a clean header when moving to a new file
                if !seen_files.contains(&file_node.id) {
                    context.push_str("// ==========================================================\n");
                    context.push_str(&format!("// File: {}\n", file_node.path));
                    if file_node.is_test {
                        context.push_str("// (Test File)\n");
                    }
                    context.push_str("// ==========================================================\n");
                    seen_files.insert(file_node.id);
                }

                // Add Documentation if requested
                if include_docs {
                    if let Some(docs) = &sym.doc_comment {
                        context.push_str(docs);
                        context.push('\n');
                    }
                }

                // Extract Source Code
                if let Ok(content) = std::fs::read_to_string(&file_node.path) {
                    if sym.range_end <= content.len() {
                        let text = String::from_utf8_lossy(&content.as_bytes()[sym.range_start..sym.range_end]);
                        context.push_str(&text);
                    } else {
                        context.push_str("// Error: Source range out of bounds for this file version");
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

pub fn find_related_symbols(
    index: &WorkspaceIndex, 
    target_name: &str,
) -> Option<Vec<SymbolId>> {
    let targets = index.symbol_map.get(target_name)?;
    if targets.is_empty() { return None; }

    let mut queue: VecDeque<(SymbolId, TraversalMode)> = VecDeque::new();
    let mut visited: HashSet<(SymbolId, TraversalMode)> = HashSet::new();
    let mut result_set: HashSet<SymbolId> = HashSet::new();

    for &id in targets {
        queue.push_back((id, TraversalMode::Both));
        visited.insert((id, TraversalMode::Both));
        result_set.insert(id);
    }

    while let Some((current_id, mode)) = queue.pop_front() {
        // 1. DOWNSTREAM
        if mode == TraversalMode::Both || mode == TraversalMode::Downstream {
            if let Some(callees) = index.resolved_calls.get(&current_id) {
                for &callee_id in callees {
                    if !visited.contains(&(callee_id, TraversalMode::Downstream)) {
                        visited.insert((callee_id, TraversalMode::Downstream));
                        result_set.insert(callee_id);
                        queue.push_back((callee_id, TraversalMode::Downstream));
                    }
                }
            }
        }

        // 2. UPSTREAM
        if mode == TraversalMode::Both || mode == TraversalMode::Upstream {
            for (caller_id, callees) in &index.resolved_calls {
                if callees.contains(&current_id) {
                    if !visited.contains(&(*caller_id, TraversalMode::Upstream)) {
                        visited.insert((*caller_id, TraversalMode::Upstream));
                        result_set.insert(*caller_id);
                        queue.push_back((*caller_id, TraversalMode::Upstream));
                    }
                }
            }
        }

        // 3. STRUCTURAL
        if let Some(children) = index.inheritance.get(&current_id) {
            for &child_id in children {
                if !visited.contains(&(child_id, mode)) {
                    visited.insert((child_id, mode));
                    result_set.insert(child_id);
                    queue.push_back((child_id, mode));
                }
            }
        }
        
        for (parent_id, children) in &index.inheritance {
            if children.contains(&current_id) {
                if !visited.contains(&(*parent_id, mode)) {
                    visited.insert((*parent_id, mode));
                    result_set.insert(*parent_id);
                    queue.push_back((*parent_id, mode));
                }
            }
        }

        // 4. CONTAINMENT
        if let Some(sym) = index.symbols.get(&current_id) {
            if let Some(p_id) = sym.parent_id {
                if !visited.contains(&(p_id, mode)) {
                    visited.insert((p_id, mode));
                    result_set.insert(p_id);
                    queue.push_back((p_id, mode));
                }
            }
            if sym.kind == "container" {
                for (&s_id, s_node) in &index.symbols {
                    if s_node.parent_id == Some(current_id) {
                        if !visited.contains(&(s_id, mode)) {
                            visited.insert((s_id, mode));
                            result_set.insert(s_id);
                            queue.push_back((s_id, mode));
                        }
                    }
                }
            }
        }
    }

    let mut final_list: Vec<SymbolId> = result_set.into_iter().collect();
    final_list.sort_by(|a, b| {
        let sym_a = index.symbols.get(a).unwrap();
        let sym_b = index.symbols.get(b).unwrap();
        sym_a.file_id.cmp(&sym_b.file_id).then(sym_a.range_start.cmp(&sym_b.range_start))
    });

    Some(final_list)
}