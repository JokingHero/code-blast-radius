use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::{Parser, Query, QueryCursor};
use walkdir::WalkDir;
use tree_sitter::StreamingIterator;

use crate::language::{get_language, LanguageConfig};

// Represents a single function found in the codebase.
#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub name: String,
    pub file_path: PathBuf,
    pub source_code: String,
    pub documentation: Option<String>,
    pub calls: Vec<String>,
}

// A map from function name to its info.
pub type CodebaseGraph = HashMap<String, FunctionInfo>;

// Main function to build the entire codebase graph.
pub fn build_codebase_graph(
    dir: &Path,
    configs: &[&'static LanguageConfig],
) -> CodebaseGraph {
    let mut graph = CodebaseGraph::new();
    let lang_map: HashMap<String, &'static LanguageConfig> = configs
        .iter()
        .flat_map(|&config| {
            config
                .file_extensions
                .iter()
                .map(move |&ext| (ext.to_string(), config))
        })
        .collect();

    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
    {
        let path = entry.path();
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            if let Some(config) = lang_map.get(ext) {
                if let Ok(functions) = parse_file(path, config) {
                    for func in functions {
                        graph.insert(func.name.clone(), func);
                    }
                }
            }
        }
    }
    graph
}

// Parses a single file and extracts all function information.
fn parse_file(path: &Path, config: &LanguageConfig) -> Result<Vec<FunctionInfo>, String> {
    let code = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let code_bytes = code.as_bytes();
    let language = get_language(config.lang_enum);

    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .map_err(|e| e.to_string())?;
    let tree = parser.parse(&code, None).ok_or("Failed to parse code")?;

    // Unwrap is safe here because we hardcoded valid queries in language.rs
    // If it panics now, it means our query syntax is wrong for the grammar (which we just fixed).
    let defs_query = Query::new(&language, config.query_defs).expect("Invalid definitions query");
    let docs_query = Query::new(&language, config.query_docs).expect("Invalid docs query");
    let calls_query = Query::new(&language, config.query_calls).expect("Invalid calls query");

    let mut functions = Vec::new();
    let mut cursor = QueryCursor::new();

    let mut matches = cursor.matches(&defs_query, tree.root_node(), code_bytes);
    
    // Iterate over every function definition found
    while let Some(match_) = matches.next() {
        // Extract function definition node
        let def_node = match_
            .captures
            .iter()
            .find(|c| defs_query.capture_names()[c.index as usize] == "function.definition")
            .unwrap()
            .node;
        
        // Extract function name node
        let name_node = match_
            .captures
            .iter()
            .find(|c| defs_query.capture_names()[c.index as usize] == "function.name")
            .unwrap()
            .node;

        let name = name_node.utf8_text(code_bytes).unwrap().to_string();
        let source_code = def_node.utf8_text(code_bytes).unwrap().to_string();

        // 1. Find Calls inside this function
        let mut calls = Vec::new();
        let mut calls_cursor = QueryCursor::new();
        let mut call_matches = calls_cursor.matches(&calls_query, def_node, code_bytes);
        while let Some(call_match) = call_matches.next() {
            let call_name_node = call_match
                .captures
                .iter()
                .find(|c| calls_query.capture_names()[c.index as usize] == "call.name")
                .unwrap()
                .node;
            
            if let Ok(call_name) = call_name_node.utf8_text(code_bytes) {
                calls.push(call_name.to_string());
            }
        }

        // 2. Find Documentation for this function
        let mut documentation = None;
        let mut doc_cursor = QueryCursor::new();
        let mut doc_matches = doc_cursor.matches(&docs_query, tree.root_node(), code_bytes);
        
        // We look for a doc match where the "function.definition" part matches our current def_node
        while let Some(doc_match) = doc_matches.next() {
            let doc_def_node = doc_match
                .captures
                .iter()
                .find(|c| docs_query.capture_names()[c.index as usize] == "function.definition")
                .unwrap()
                .node;
            
            if doc_def_node == def_node {
                // COLLECT ALL DOC NODES (Fixes the Rust Docs bug)
                let doc_lines: Vec<String> = doc_match
                    .captures
                    .iter()
                    .filter(|c| docs_query.capture_names()[c.index as usize] == "function.docs")
                    .map(|c| c.node.utf8_text(code_bytes).unwrap_or("").to_string())
                    .collect();
                
                if !doc_lines.is_empty() {
                     // Join multiple line comments with newline
                    documentation = Some(doc_lines.join("\n"));
                }
                break;
            }
        }

        functions.push(FunctionInfo {
            name,
            file_path: path.to_path_buf(),
            source_code,
            documentation,
            calls,
        });
    }

    Ok(functions)
}

// Finds the call chain leading to the target function using a reverse BFS.
pub fn find_call_chain(graph: &CodebaseGraph, target_function: &str) -> Option<Vec<String>> {
    if !graph.contains_key(target_function) {
        return None;
    }

    let mut predecessors = HashMap::new();
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

// Generates the final formatted context string.
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