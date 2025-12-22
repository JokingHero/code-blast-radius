use std::collections::{HashMap, HashSet, VecDeque}; // <--- Added back for find_call_chain
use std::path::{Path, PathBuf};
use tree_sitter::{Parser, Query, QueryCursor};
use tree_sitter::StreamingIterator; // <--- REQUIRED for matches.next()

use crate::language::{get_language, LanguageConfig};

#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub name: String,
    pub file_path: PathBuf,
    pub source_code: String,
    pub documentation: Option<String>,
    pub calls: Vec<String>,
}

pub type CodebaseGraph = HashMap<String, FunctionInfo>;

pub fn analyze_source(
    path: &Path,
    source_code: &str,
    config: &LanguageConfig,
) -> Result<Vec<FunctionInfo>, String> {
    let code_bytes = source_code.as_bytes();
    let language = get_language(config.lang_enum);

    let mut parser = Parser::new();
    parser.set_language(&language).map_err(|e| e.to_string())?;
    
    if source_code.trim().is_empty() {
        return Ok(Vec::new());
    }

    let tree = parser.parse(source_code, None).ok_or("Failed to parse code")?;

    let defs_query = Query::new(&language, config.query_defs).expect("Invalid definitions query");
    let docs_query = Query::new(&language, config.query_docs).expect("Invalid docs query");
    let calls_query = Query::new(&language, config.query_calls).expect("Invalid calls query");

    let mut functions = Vec::new();
    let mut cursor = QueryCursor::new();

    // matches() returns an iterator that requires StreamingIterator to be in scope
    let mut matches = cursor.matches(&defs_query, tree.root_node(), code_bytes);
    
    while let Some(match_) = matches.next() {
        let def_node = match_.captures.iter().find(|c| defs_query.capture_names()[c.index as usize] == "function.definition").unwrap().node;
        let name_node = match_.captures.iter().find(|c| defs_query.capture_names()[c.index as usize] == "function.name").unwrap().node;

        let name = name_node.utf8_text(code_bytes).unwrap().to_string();
        let func_source = def_node.utf8_text(code_bytes).unwrap().to_string();

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
            file_path: path.to_path_buf(),
            source_code: func_source,
            documentation,
            calls,
        });
    }

    Ok(functions)
}

pub fn find_call_chain(graph: &CodebaseGraph, target_function: &str) -> Option<Vec<String>> {
    if !graph.contains_key(target_function) {
        return None;
    }

    // Fix: Explicitly type the HashMap so compiler doesn't panic on "unknown type"
    let mut predecessors: HashMap<String, String> = HashMap::new();
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    
    queue.push_back(target_function.to_string());
    visited.insert(target_function.to_string());

    while let Some(current_func) = queue.pop_front() {
        for (caller_name, caller_info) in graph.iter() {
            if caller_info.calls.contains(&current_func) && !visited.contains(caller_name) {
                visited.insert(caller_name.clone());
                predecessors.insert(current_func.clone(), caller_name.clone());
                queue.push_back(caller_name.clone());
            }
        }
    }

    let mut root_node = target_function.to_string();
    while let Some(parent) = predecessors.get(&root_node) {
        root_node = parent.clone();
    }

    let inverted_predecessors: HashMap<String, String> =
        predecessors.into_iter().map(|(k, v)| (v, k)).collect();

    let mut path = vec![root_node.clone()];
    let mut current = root_node;
    
    // Fix: explicitly hint types here implicitly by usage, but predecessors logic is fixed now
    while let Some(child) = inverted_predecessors.get(&current) {
        path.push(child.clone());
        if child == target_function {
            break;
        }
        current = child.clone();
    }
    
    if path.last().map_or(true, |p| p != target_function) && target_function != path.first().unwrap() {
        return None;
    }

    Some(path)
}

pub fn generate_context(
    graph: &CodebaseGraph,
    chain: &[String],
    include_docs: bool,
) -> String {
    let mut context = String::new();
    context.push_str(&format!(
        "// Context for function: `{}`\n",
        chain.last().unwrap()
    ));
    context.push_str(&format!("// Call Chain: {}\n\n", chain.join(" -> ")));

    for func_name in chain {
        if let Some(info) = graph.get(func_name) {
            context.push_str("// ==========================================================\n");
            context.push_str(&format!("// File: {}\n", info.file_path.display()));
            context.push_str("// ==========================================================\n");
            if include_docs {
                if let Some(docs) = &info.documentation {
                    context.push_str(docs);
                    context.push('\n');
                }
            }
            context.push_str(&info.source_code);
            context.push_str("\n\n");
        }
    }
    context
}