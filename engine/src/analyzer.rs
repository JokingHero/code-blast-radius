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
}

pub struct FileAnalysis {
    pub functions: Vec<FunctionInfo>,
    pub imports: Vec<ImportNode>,
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
        return Ok(FileAnalysis { functions: vec![], imports: vec![] });
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

            if !current_source.is_empty() && !current_name.is_empty() {
                imports.push(ImportNode {
                    name: current_name,
                    source: current_source,
                    alias: None, 
                });
            }
        }
    }

    // --- 2. Extract Functions & Calls ---
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
        let mut calls_cursor = QueryCursor::new();
        let mut call_matches = calls_cursor.matches(&calls_query, def_node, code_bytes);
        while let Some(call_match) = call_matches.next() {
            let call_name_node = call_match.captures.iter().find(|c| calls_query.capture_names()[c.index as usize] == "call.name").unwrap().node;
            if let Ok(call_name_str) = call_name_node.utf8_text(code_bytes) {
                calls.push(call_name_str.to_string());
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
        });
    }

    Ok(FileAnalysis { functions, imports })
}

pub fn find_call_chain_ids(index: &WorkspaceIndex, target_name: &str) -> Option<Vec<SymbolId>> {
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
        for (caller_id, callees) in &index.resolved_calls {
            if callees.contains(&current_id) {
                if !visited.contains(caller_id) {
                    visited.insert(*caller_id);
                    predecessors.insert(current_id, *caller_id); 
                    queue.push_back(*caller_id);
                }
            }
        }
    }

    let mut current = targets[0]; 
    for &t in targets {
        if predecessors.contains_key(&t) {
            current = t;
            break;
        }
    }

    let mut chain = vec![current];
    while let Some(&parent) = predecessors.get(&current) {
        chain.push(parent);
        current = parent;
    }

    chain.reverse(); 
    Some(chain)
}

pub fn generate_context_from_ids(
    index: &WorkspaceIndex,
    chain: &[SymbolId],
    include_docs: bool,
) -> String {
    let mut context = String::new();
    let target_id = chain.last().unwrap();
    let target_name = index.symbols.get(target_id).map(|s| s.name.as_str()).unwrap_or("Unknown");

    context.push_str(&format!("// Context for function: `{}`\n", target_name));
    
    let names: Vec<String> = chain.iter()
        .filter_map(|id| index.symbols.get(id).map(|s| s.name.clone()))
        .collect();
    context.push_str(&format!("// Call Chain: {}\n\n", names.join(" -> ")));

    for &sym_id in chain {
        if let Some(sym) = index.symbols.get(&sym_id) {
            if let Some(file_node) = index.files.values().find(|f| f.id == sym.file_id) {
                context.push_str("// ==========================================================\n");
                context.push_str(&format!("// File: {}\n", file_node.path));
                context.push_str("// ==========================================================\n");

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
                        context.push_str("// Error: Source range out of bounds");
                    }
                } else {
                    context.push_str("// Error: Could not read file");
                }
                context.push_str("\n\n");
            }
        }
    }
    context
}