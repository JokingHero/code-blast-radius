use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};

use crate::language::{LanguageConfig, SupportedLanguage, get_language};
use crate::schema::{ImportNode, ExportNode, WorkspaceIndex, SymbolId};

#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub name: String,
    pub kind: String, // Carries the detected type (macro, function, container) to the Indexer
    pub is_anonymous: bool, 
    pub range_start: usize,
    pub range_end: usize,
    pub source_code: String,
    pub documentation: Option<String>,
    pub calls: Vec<String>, 
    pub type_refs: Vec<String>, 
    pub decorators: Vec<String>,
    pub dispatched_actions: Vec<String>, 
    pub handled_actions: Vec<String>,
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
    pub middleware_usage: Vec<String>,
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
    path: &Path,
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
            literals: vec![], implementations: vec![], global_vars: HashMap::new(),
            middleware_usage: vec![],
        });
    }

    let tree = parser.parse(source_code, None).ok_or("Failed to parse code")?;
    let root_node = tree.root_node();
    
    // --- STEP 0: CONSTANT PROPAGATION ---
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

    // ... (Imports, Exports, Literals, Implements steps 1-4 remain same) ...
    // 1. Imports
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
                        if let Some(resolved) = local_constants.get(&text) {
                            src = resolved.clone();
                        } else {
                            src = text.replace(['"', '\''], ""); 
                        }
                    } else if capture_name == "import.dynamic" {
                        if let Some(resolved) = local_constants.get(&text) {
                            src = resolved.clone();
                            src = src.replace(['"', '\'', '`'], "");
                        }
                    } else if capture_name == "import.name" { name = text; } 
                    else if capture_name == "import.alias" { name = "*".to_string(); alias = Some(text); }
                }
                if !src.is_empty() {
                    if config.lang_enum == SupportedLanguage::Python {
                        if src.contains('.') && !src.starts_with("./") && !src.starts_with("../") {
                            if src != "." && src != ".." { src = src.replace('.', "/"); }
                        }
                    }
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
                        if let Some(resolved) = local_constants.get(&text) { src = resolved.clone(); } 
                        else { src = text.replace(['"', '\''], ""); }
                    } else if capture_name == "export.name" { name = Some(text); }
                }
                if !src.is_empty() { exports.push(ExportNode { name, source: src }); }
            }
        }
    }
    // 3. Literals & Template Expansion
    if !config.query_literals.is_empty() {
        if let Ok(q) = Query::new(&language, config.query_literals) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&q, root_node, code_bytes);
            
            while let Some(m) = matches.next() {
                for cap in m.captures {
                    let node = cap.node;
                    let node_kind = node.kind();

                    // Logic for Template Strings (JS/TS `...` or Python f"...")
                    if node_kind == "template_string" || node_kind == "string" { // Python sometimes wraps f-strings in 'string'
                        let mut synthetic = String::new();
                        let mut is_complex = false;
                        
                        // Iterate over children to build the string
                        let mut cursor = node.walk();
                        for child in node.children(&mut cursor) {
                            let k = child.kind();
                            
                            // JS/TS: string_fragment, Python: string_content
                            if k == "string_fragment" || k == "string_content" {
                                synthetic.push_str(&child.utf8_text(code_bytes).unwrap_or(""));
                            } 
                            // JS/TS: ${var}
                            else if k == "template_substitution" || k == "interpolation" {
                                is_complex = true;
                                let mut found_const = false;
                                
                                // Try to find the identifier inside the substitution
                                // We look for the first identifier child
                                let mut sub_cursor = child.walk();
                                for sub_child in child.children(&mut sub_cursor) {
                                    if sub_child.kind() == "identifier" {
                                        let var_name = sub_child.utf8_text(code_bytes).unwrap_or("");
                                        if let Some(val) = local_constants.get(var_name) {
                                            // STRIP QUOTES from the constant value before inserting
                                            let raw_val = val.trim_matches(|c| c == '"' || c == '\'' || c == '`');
                                            synthetic.push_str(raw_val);
                                            found_const = true;
                                        }
                                        break; // Only handle simple ${var}, not ${func()}
                                    }
                                }
                                
                                if !found_const {
                                    // If we can't resolve it, add a wildcard for fuzzy matching later
                                    synthetic.push('*');
                                }
                            }
                        }

                        // Check if we actually built something meaningful
                        if is_complex && !synthetic.is_empty() {
                             literals.push(synthetic.clone());
                        }
                    }

                    // Fallback to standard raw text extraction for normal strings
                    let text = node.utf8_text(code_bytes).unwrap_or("")
                        .trim_matches(|c| c == '"' || c == '\'' || c == '`')
                        .to_string();
                    
                    if text.len() > 1 { 
                        literals.push(text); 
                    }
                }
            }
        }
    }
    for val in local_constants.values() { if val.len() > 1 { literals.push(val.clone()); } }
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
                    if name == "impl.child" { child = text; } else if name == "impl.parent" { parent = text; }
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
    } else { None };
    
    let types_query = if !config.query_types.is_empty() {
        Some(Query::new(&language, config.query_types)
            .map_err(|e| format!("Invalid types query for {:?}: {}", config.lang_enum, e))?)
    } else { None };

    let decorators_query = if !config.query_decorators.is_empty() {
        Some(Query::new(&language, config.query_decorators)
            .map_err(|e| format!("Invalid decorators query for {:?}: {}", config.lang_enum, e))?)
    } else { None };

    // --- Create the Module Symbol ---
    let module_name = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let mut module_info = FunctionInfo {
        name: format!("(module) {}", module_name),
        kind: "module".to_string(),
        is_anonymous: false,
        range_start: root_node.start_byte(),
        range_end: root_node.end_byte(),
        source_code: String::new(), 
        documentation: None, 
        calls: Vec::new(),
        type_refs: Vec::new(),
        decorators: Vec::new(),
        fingerprints: HashMap::new(),
        return_type: None,
        local_types: HashMap::new(),
        local_assigns: HashMap::new(),
        config_keys: Vec::new(),
        dispatched_actions: Vec::new(),
        handled_actions: Vec::new(),
    };

    // 6. Extract Definitions
    let mut variable_hints = Vec::new(); 
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&defs_query, root_node, code_bytes);

    // --- DEBUG START ---
    println!("DEBUG: Analyzing {}", path.display());
    // --- DEBUG END ---

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
            
            // --- DEBUG START ---
            println!("  Capture: {} -> {}", cap_name, text);
            // --- DEBUG END ---

            match cap_name {
                "function.definition" => def_node = Some(capture.node),
                "function.name" => name_opt = Some(text.to_string()),
                "function.return_type" => {
                    return_type = Some(text.trim_start_matches(|c| c == ':' || c == '=' || c == '>').trim().to_string());
                }
                "variable.name" => {
                    v_name = Some(text.to_string());
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
            let node_kind = node.kind();
            
            // --- DEBUG START ---
            println!("  -> Found Definition: {:?} Kind: {}", name_opt, node_kind);
            // --- DEBUG END ---

            let kind = if node_kind == "macro_definition" {
                "macro".to_string()
            } else if node_kind == "macro_invocation" {
                "macro_generated".to_string()
            } else if node_kind.contains("class") 
                   || node_kind.contains("interface") 
                   || node_kind.contains("struct") 
                   || node_kind.contains("impl") {
                "container".to_string()
            } else {
                "function".to_string()
            };

            functions.push(FunctionInfo {
                name: name_opt.clone().unwrap_or_else(|| "anonymous".to_string()),
                kind, 
                is_anonymous: name_opt.is_none(),
                range_start: node.start_byte(),
                range_end: node.end_byte(),
                source_code: node.utf8_text(code_bytes).unwrap_or("").to_string(),
                documentation: None,
                calls: Vec::new(),
                type_refs: Vec::new(),
                decorators: Vec::new(),
                fingerprints: HashMap::new(),
                return_type,
                local_types: HashMap::new(),
                local_assigns: HashMap::new(),
                config_keys: Vec::new(),
                dispatched_actions: Vec::new(),
                handled_actions: Vec::new(),
            });
        } else if let Some(vn) = v_name {
            // --- DEBUG START ---
            println!("  -> Found Var Hint: {}", vn);
            // --- DEBUG END ---
            variable_hints.push((match_.captures[0].node.byte_range(), vn, v_type, v_assign));
        }
    }

    // Helper: Smallest Container
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

    // 8.5 Extract Decorators
    if let Some(ref q) = decorators_query {
        let mut dec_cursor = QueryCursor::new();
        let mut dec_matches = dec_cursor.matches(q, root_node, code_bytes);
        
        while let Some(dm) = dec_matches.next() {
            for cap in dm.captures {
                let text = cap.node.utf8_text(code_bytes).unwrap_or("").to_string();
                let clean_name = text.trim_matches(|c| c == '@' || c == '#' || c == '[' || c == ']' || c == '(' || c == ')').to_string();
                
                if !clean_name.is_empty() {
                    let range = cap.node.byte_range();
                    
                    if let Some(idx) = get_owner_index(range.start, range.end, &functions) {
                        functions[idx].decorators.push(clean_name);
                    } else {
                        let mut found_neighbor = false;
                        for func in &mut functions {
                            if func.range_start > range.end && (func.range_start - range.end) < 200 {
                                func.decorators.push(clean_name.clone());
                                found_neighbor = true;
                                break;
                            }
                        }
                        if !found_neighbor {
                            module_info.decorators.push(clean_name);
                        }
                    }
                }
            }
        }
    }

    // 9. Extract and Distribute Calls
    let mut c_cursor = QueryCursor::new();
    let mut c_matches = c_cursor.matches(&calls_query, root_node, code_bytes);
    
    while let Some(cm) = c_matches.next() {
        let mut m_name = None;
        let mut r_name = None;
        let mut dynamic_receiver = None; 
        let mut call_range = None;

        for cp in cm.captures {
            let t = cp.node.utf8_text(code_bytes).unwrap_or("").to_string();
            let cap_name = calls_query.capture_names()[cp.index as usize];
            
            if cap_name == "call.name" { 
                m_name = Some(t); 
                call_range = Some(cp.node.byte_range());
            }
            else if cap_name == "call.receiver" { 
                r_name = Some(t); 
            }
            else if cap_name == "call.dynamic_dispatch" {
                dynamic_receiver = Some(t);
                call_range = Some(cp.node.byte_range());
            }
        }

        // Logic A: Standard Method/Function Call
        // We use call_range.clone() here so we don't consume the Option 
        // if we need to fall through to Logic B.
        if let (Some(m), Some(range)) = (m_name, call_range.clone()) {
            if let Some(idx) = get_owner_index(range.start, range.end, &functions) {
                let func = &mut functions[idx];
                func.calls.push(m.clone());
                if let Some(r) = r_name {
                    func.fingerprints.entry(r).or_default().push(m);
                }
            } else {
                module_info.calls.push(m.clone());
                if let Some(r) = r_name {
                    module_info.fingerprints.entry(r).or_default().push(m);
                }
            }
        }
        
        // Logic B: Dynamic Dispatch / Reflection
        // We register a wildcard "*" to tell the indexer: "Link everything about this type."
        else if let (Some(dr), Some(range)) = (dynamic_receiver, call_range) {
            if let Some(idx) = get_owner_index(range.start, range.end, &functions) {
                let func = &mut functions[idx];
                func.fingerprints.entry(dr).or_default().push("*".to_string());
            } else {
                 module_info.fingerprints.entry(dr).or_default().push("*".to_string());
            }
        }
    }

    // 9.5 Extract Type References
    if let Some(ref q) = types_query {
        let mut t_cursor = QueryCursor::new();
        let mut t_matches = t_cursor.matches(q, root_node, code_bytes);
        
        while let Some(tm) = t_matches.next() {
            for cap in tm.captures {
                let type_name = cap.node.utf8_text(code_bytes).unwrap_or("").to_string();
                if !type_name.is_empty() {
                    let range = cap.node.byte_range();
                    if let Some(idx) = get_owner_index(range.start, range.end, &functions) {
                        functions[idx].type_refs.push(type_name);
                    } else {
                        module_info.type_refs.push(type_name);
                    }
                }
            }
        }
    }

    // 9.6 Extract State Actions
    let actions_query = if !config.query_actions.is_empty() {
        Some(Query::new(&language, config.query_actions).unwrap())
    } else { None };

    if let Some(ref q) = actions_query {
        let mut a_cursor = QueryCursor::new();
        let mut a_matches = a_cursor.matches(q, root_node, code_bytes);
        
        while let Some(am) = a_matches.next() {
            for cap in am.captures {
                let raw_text = cap.node.utf8_text(code_bytes).unwrap_or("").to_string();                
                let resolved_text = if let Some(val) = local_constants.get(&raw_text) {
                    val.clone()
                } else {
                    raw_text
                };

                let text = resolved_text.trim_matches(|c| c == '"' || c == '\'' || c == '`').to_string();                
                let capture_name = q.capture_names()[cap.index as usize];
                let range = cap.node.byte_range();

                if let Some(idx) = get_owner_index(range.start, range.end, &functions) {
                    if capture_name == "action.dispatch" {
                        functions[idx].dispatched_actions.push(text);
                    } else if capture_name == "action.handle" {
                        functions[idx].handled_actions.push(text);
                    }
                } else {
                    let mut found_neighbor = false;
                    
                    if capture_name == "action.handle" {
                        for func in &mut functions {
                            if func.range_start > range.end && (func.range_start - range.end) < 200 {
                                func.handled_actions.push(text.clone());
                                found_neighbor = true;
                                break;
                            }
                        }
                    }

                    if !found_neighbor {
                        if capture_name == "action.dispatch" {
                            module_info.dispatched_actions.push(text);
                        } else if capture_name == "action.handle" {
                            module_info.handled_actions.push(text);
                        }
                    }
                }
            }
        }
    }

    // 9.7 Extract Middleware Usage (NEW)
    let middleware_query = if !config.query_middleware.is_empty() {
        Some(Query::new(&language, config.query_middleware).map_err(|e| e.to_string())?)
    } else { None };

    let mut detected_middleware = Vec::new();

    if let Some(ref q) = middleware_query {
        let mut mw_cursor = QueryCursor::new();
        let mut mw_matches = mw_cursor.matches(q, root_node, code_bytes);
        
        while let Some(m) = mw_matches.next() {
            for cap in m.captures {
                let capture_name = q.capture_names()[cap.index as usize];
                let text = cap.node.utf8_text(code_bytes).unwrap_or("").to_string();
                
                // Clean quotes for Django strings
                let clean_text = text.trim_matches(|c| c == '"' || c == '\'' || c == '`').to_string();
                
                if capture_name == "middleware.use" || capture_name == "middleware.config" {
                    detected_middleware.push(clean_text);
                }
            }
        }
    }

    // Deduplicate logic
    for func in &mut functions {
        func.config_keys.sort(); func.config_keys.dedup();
        func.type_refs.sort(); func.type_refs.dedup();
        func.decorators.sort(); func.decorators.dedup();
        func.dispatched_actions.sort(); func.dispatched_actions.dedup();
        func.handled_actions.sort(); func.handled_actions.dedup();
    }
    module_info.config_keys.sort(); module_info.config_keys.dedup();
    module_info.type_refs.sort(); module_info.type_refs.dedup();
    module_info.decorators.sort(); module_info.decorators.dedup();

    // 10. Extract Docs
    let mut d_cursor = QueryCursor::new();
    let mut d_matches = d_cursor.matches(&docs_query, root_node, code_bytes);
    while let Some(dm) = d_matches.next() {
        let d_def = dm.captures.iter()
            .find(|c| docs_query.capture_names()[c.index as usize] == "function.definition")
            .map(|c| c.node);
        
        if let Some(d_node) = d_def {
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
        functions, imports, exports, literals, implementations, global_vars: HashMap::new(), middleware_usage: detected_middleware,
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
                continue;
            }

            if let Some(file_node) = index.files.values().find(|f| f.id == sym.file_id) {
                if !seen_files.contains(&file_node.id) {
                    context.push_str("// ==========================================================\n");
                    context.push_str(&format!("// File: {}\n", file_node.path));
                    if file_node.is_test {
                        context.push_str("// (Test File)\n");
                    }
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
        // 1. DOWNSTREAM (Function -> Types it uses, or Function -> Calls)
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
            // Follow Type References
            if let Some(type_ids) = index.resolved_type_refs.get(&current_id) {
                for &tid in type_ids {
                    if !visited.contains(&(tid, TraversalMode::Downstream)) {
                        visited.insert((tid, TraversalMode::Downstream));
                        result_set.insert(tid);
                        queue.push_back((tid, TraversalMode::Downstream));
                    }
                }
            }
        }

        // 2. UPSTREAM (Function <- Callers, or Type <- Function using it)
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
            // Find functions that use this Type
            for (func_id, used_types) in &index.resolved_type_refs {
                if used_types.contains(&current_id) {
                    if !visited.contains(&(*func_id, TraversalMode::Upstream)) {
                        visited.insert((*func_id, TraversalMode::Upstream));
                        result_set.insert(*func_id);
                        queue.push_back((*func_id, TraversalMode::Upstream));
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