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

pub fn find_call_chain(graph: &CodebaseGraph, target_name: &str) -> Option<Vec<String>> {
    // 1. Find all nodes that match the target name (Handling Collisions)
    // The keys are now "path::name", so we can't do a direct lookup.
    // We filter the graph values.
    let target_keys: Vec<String> = graph.iter()
        .filter(|(_, info)| info.name == target_name)
        .map(|(key, _)| key.clone())
        .collect();

    if target_keys.is_empty() {
        return None;
    }

    // Predecessors map: Child Key -> Parent Key
    let mut predecessors: HashMap<String, String> = HashMap::new();
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();

    // Initialize queue with ALL matching targets
    // Example: if we look for "init", we start with "db.rs::init" AND "log.rs::init"
    for key in &target_keys {
        queue.push_back(key.clone());
        visited.insert(key.clone());
    }

    while let Some(current_key) = queue.pop_front() {
        // We need the Short Name of the current node to check if others call it.
        // But wait, the `calls` list contains Short Names (e.g. "init").
        // So we need to check: Does Caller have "init" in its calls list?
        // AND does current_node.name == "init"? 
        // Yes, this is implicit because we are traversing *up*.
        
        let current_short_name = &graph.get(&current_key)?.name;

        for (caller_key, caller_info) in graph.iter() {
            // Check if this caller calls our current function (by short name)
            // Note: This is "Loose Resolution". If main() calls "init", 
            // and we have DB::init and Log::init, main() becomes a parent of BOTH.
            // This is desired behavior for a context tool (show all possibilities).
            if caller_info.calls.contains(current_short_name) && !visited.contains(caller_key) {
                visited.insert(caller_key.clone());
                
                // Record path
                predecessors.insert(current_key.clone(), caller_key.clone());
                
                queue.push_back(caller_key.clone());
            }
        }
    }
    
    let mut path = Vec::new();
    
    // Find a node that we reached (is in `visited`) and is likely a Root.
    // We can just iterate `predecessors` to build a full chain.
    // Let's try to find a chain from a Root to ONE of our targets.
    
    let mut current = target_keys[0].clone(); // Start at a target
    // If we have predecessors for this target, trace up.
    // If not, maybe another target has predecessors?
    
    for t in &target_keys {
        if predecessors.contains_key(t) {
            current = t.clone();
            break;
        }
    }
    
    // Trace upwards (Target -> Caller -> Caller)
    path.push(current.clone());
    while let Some(parent) = predecessors.get(&current) {
        path.push(parent.clone());
        current = parent.clone();
    }
    
    // The path is now [Target, Caller, Root]. Reverse it for display [Root, Caller, Target].
    path.reverse();
    
    // Verify valid chain
    if path.len() == 1 && !target_keys.contains(&path[0]) {
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
    
    // Extract short name for display
    let target_key = chain.last().unwrap();
    let target_name = graph.get(target_key).map(|i| i.name.as_str()).unwrap_or(target_key);

    context.push_str(&format!(
        "// Context for function: `{}`\n",
        target_name
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