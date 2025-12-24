use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};

use crate::language::{get_language, LanguageConfig};
use crate::schema::{ImportNode, WorkspaceIndex, SymbolId};

#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub name: String,
    pub range_start: usize,
    pub range_end: usize,
    pub source_code: String,
    pub documentation: Option<String>,
    pub calls: Vec<String>, 
    pub fingerprints: HashMap<String, Vec<String>>,
}

pub struct FileAnalysis {
    pub functions: Vec<FunctionInfo>,
    pub imports: Vec<ImportNode>,
    pub literals: Vec<String>,
    pub implementations: Vec<(String, String)>, 
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SliceDirection {
    Upstream,
    Downstream,
    Both,
}

/// Internal helper for find_related_symbols to prevent zig-zag pollution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum TraversalMode {
    Downstream, // Looking at what we use
    Upstream,   // Looking at who uses us
    Both,       // Initial state
}

pub fn analyze_source(
    _path: &Path,
    source_code: &str,
    config: &LanguageConfig,
) -> Result<FileAnalysis, String> {
    let code_bytes = source_code.as_bytes();
    let language = get_language(config.lang_enum);

    let mut parser = Parser::new();
    parser.set_language(&language).map_err(|e| e.to_string())?;
    
    if source_code.trim().is_empty() {
        return Ok(FileAnalysis { 
            functions: vec![], 
            imports: vec![], 
            literals: vec![], 
            implementations: vec![] 
        });
    }

    let tree = parser.parse(source_code, None).ok_or("Failed to parse code")?;

    // --- 1. Extract Imports ---
    let mut imports = Vec::new();
    if !config.query_imports.is_empty() {
        let imports_query = Query::new(&language, config.query_imports).expect("Invalid imports query");
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&imports_query, tree.root_node(), code_bytes);

        while let Some(match_) = matches.next() {
            let mut current_source = String::new();
            let mut current_name = String::new();

            for capture in match_.captures {
                let capture_name = imports_query.capture_names()[capture.index as usize];
                let text = capture.node.utf8_text(code_bytes).unwrap_or("").to_string();

                if capture_name == "import.source" {
                    current_source = text.replace(['"', '\''], "");
                } else if capture_name == "import.name" {
                    current_name = text;
                }
            }

            if !current_source.is_empty() {
                imports.push(ImportNode {
                    name: current_name,
                    source: current_source,
                    alias: None, 
                });
            }
        }
    }

    // --- 2. Extract Literals ---
    let mut literals = Vec::new();
    if !config.query_literals.is_empty() {
        let lit_query = Query::new(&language, config.query_literals).expect("Invalid literals query");
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&lit_query, tree.root_node(), code_bytes);

        while let Some(match_) = matches.next() {
            for capture in match_.captures {
                let text = capture.node.utf8_text(code_bytes).unwrap_or("").to_string();
                let clean = text.trim_matches(|c| c == '"' || c == '\'' || c == '`').to_string();
                if !clean.is_empty() && clean.len() > 1 {
                    literals.push(clean);
                }
            }
        }
    }

    // --- 3. Extract Implementations ---
    let mut implementations = Vec::new();
    if !config.query_implements.is_empty() {
        let impl_query = Query::new(&language, config.query_implements).expect("Invalid implements query");
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&impl_query, tree.root_node(), code_bytes);
        
        while let Some(match_) = matches.next() {
            let mut child = String::new();
            let mut parent = String::new();
            
            for capture in match_.captures {
                let name = impl_query.capture_names()[capture.index as usize];
                let text = capture.node.utf8_text(code_bytes).unwrap_or("").to_string();
                
                if name == "impl.child" { child = text; } 
                else if name == "impl.parent" { parent = text; }
            }
            
            if !child.is_empty() && !parent.is_empty() {
                implementations.push((child, parent));
            }
        }
    }

    // --- 4. Extract Functions, Calls & Fingerprints ---
    let defs_query = Query::new(&language, config.query_defs).expect("Invalid definitions query");
    let docs_query = Query::new(&language, config.query_docs).expect("Invalid docs query");
    let calls_query = Query::new(&language, config.query_calls).expect("Invalid calls query");

    let mut functions = Vec::new();
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&defs_query, tree.root_node(), code_bytes);
    
    while let Some(match_) = matches.next() {
        let def_node = match_.captures.iter().find(|c| defs_query.capture_names()[c.index as usize] == "function.definition").unwrap().node;
        let name_node = match_.captures.iter().find(|c| defs_query.capture_names()[c.index as usize] == "function.name").unwrap().node;

        let name = name_node.utf8_text(code_bytes).unwrap().to_string();
        let func_source = def_node.utf8_text(code_bytes).unwrap().to_string();
        let range_start = def_node.start_byte();
        let range_end = def_node.end_byte();

        let mut calls = Vec::new();
        let mut fingerprints: HashMap<String, Vec<String>> = HashMap::new();
        let mut calls_cursor = QueryCursor::new();
        let mut call_matches = calls_cursor.matches(&calls_query, def_node, code_bytes);

        while let Some(call_match) = call_matches.next() {
            let mut method_name = None;
            let mut receiver_name = None;

            for capture in call_match.captures {
                let capture_name = calls_query.capture_names()[capture.index as usize];
                let text = capture.node.utf8_text(code_bytes).unwrap_or("").to_string();

                if capture_name == "call.name" {
                    method_name = Some(text);
                } else if capture_name == "call.receiver" {
                    receiver_name = Some(text);
                }
            }

            if let Some(m) = method_name {
                calls.push(m.clone());
                if let Some(r) = receiver_name {
                    fingerprints.entry(r).or_default().push(m);
                }
            }
        }

        let mut documentation = None;
        let mut doc_cursor = QueryCursor::new();
        let mut doc_matches = doc_cursor.matches(&docs_query, tree.root_node(), code_bytes);
        while let Some(doc_match) = doc_matches.next() {
            let doc_def_node = doc_match.captures.iter().find(|c| docs_query.capture_names()[c.index as usize] == "function.definition").unwrap().node;
            if doc_def_node == def_node {
                 let doc_lines: Vec<String> = doc_match.captures.iter()
                    .filter(|c| docs_query.capture_names()[c.index as usize] == "function.docs")
                    .map(|c| c.node.utf8_text(code_bytes).unwrap_or("").to_string())
                    .collect();
                if !doc_lines.is_empty() { documentation = Some(doc_lines.join("\n")); }
                break;
            }
        }
        
        functions.push(FunctionInfo {
            name,
            range_start,
            range_end,
            source_code: func_source,
            documentation,
            calls,
            fingerprints,
        });
    }

    Ok(FileAnalysis { functions, imports, literals, implementations })
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
        // --- 1. UPSTREAM Logic ---
        if direction == SliceDirection::Upstream || direction == SliceDirection::Both {
            for (caller_id, callees) in &index.resolved_calls {
                if callees.contains(&current_id) && !visited.contains(caller_id) {
                    visited.insert(*caller_id);
                    predecessors.insert(*caller_id, current_id); 
                    queue.push_back(*caller_id);
                }
            }
        }

        // --- 2. DOWNSTREAM Logic ---
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

        // --- 3. STRUCTURAL Logic ---
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

    if direction == SliceDirection::Downstream {
        final_list.reverse();
    }

    Some(final_list)
}

pub fn generate_context_from_ids(
    index: &WorkspaceIndex,
    chain: &[SymbolId],
    include_docs: bool,
) -> String {
    if chain.is_empty() { return String::from("// No context found."); }

    let mut context = String::new();
    let target_id = chain.first().unwrap(); 
    let target_name = index.symbols.get(target_id).map(|s| s.name.as_str()).unwrap_or("Unknown");

    context.push_str(&format!("// Context for search: `{}`\n", target_name));
    
    let names: Vec<String> = chain.iter()
        .filter_map(|id| index.symbols.get(id).map(|s| s.name.clone()))
        .collect();
    context.push_str(&format!("// Resolved Symbols: {}\n\n", names.join(", ")));

    let mut seen_files = HashSet::new();

    for &sym_id in chain {
        if let Some(sym) = index.symbols.get(&sym_id) {
            if let Some(file_node) = index.files.values().find(|f| f.id == sym.file_id) {
                if !seen_files.contains(&file_node.id) {
                    context.push_str("// ==========================================================\n");
                    context.push_str(&format!("// File: {}\n", file_node.path));
                    context.push_str("// ==========================================================\n");
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
                        let bytes = content.as_bytes();
                        let slice = &bytes[sym.range_start..sym.range_end];
                        let text = String::from_utf8_lossy(slice);
                        context.push_str(&text);
                    } else {
                        context.push_str("// Error: Source range out of bounds (file changed?)");
                    }
                } else {
                    context.push_str(&format!("// Error: Could not read file at {}", file_node.path));
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
        // 1. DOWNSTREAM Traversal (Dependencies)
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

        // 2. UPSTREAM Traversal (Callers)
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

        // 3. STRUCTURAL Traversal (Inheritance/Containers)
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
    }

    let mut final_list: Vec<SymbolId> = result_set.into_iter().collect();
    final_list.sort_by(|a, b| {
        let sym_a = index.symbols.get(a).unwrap();
        let sym_b = index.symbols.get(b).unwrap();
        sym_a.file_id.cmp(&sym_b.file_id).then(sym_a.range_start.cmp(&sym_b.range_start))
    });

    Some(final_list)
}