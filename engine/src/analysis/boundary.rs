use crate::analysis::language::{get_language, LanguageConfig};
use crate::models::{Definition, FileBoundary, FrameworkHint, SymbolKind};
use std::collections::HashSet;
use tree_sitter::{Parser, QueryCursor, StreamingIterator};

pub fn extract_boundary(
    path: &str,
    source_code: &str,
    config: &LanguageConfig,
    file_hash: [u8; 32],
) -> Result<FileBoundary, String> {
    let token_count = (source_code.len() / 4) as u32;
    let mut parser = Parser::new();
    let language = get_language(config.lang);
    parser.set_language(&language).map_err(|e| e.to_string())?;

    let tree = parser.parse(source_code, None).ok_or("Failed to parse")?;
    let root_node = tree.root_node();
    let source_bytes = source_code.as_bytes();

    let mut defs = Vec::new();
    let mut imports = Vec::new();
    let mut symbol_refs = HashSet::new();
    let mut literals = HashSet::new();
    let mut framework_hints: Vec<FrameworkHint> = Vec::new();

    let mut cursor = QueryCursor::new();

    // 1. Extract Definitions (The Public API)
    if let Some(query) = &config.queries.definitions {
        let mut matches = cursor.matches(query, root_node, source_bytes);

        while let Some(m) = matches.next() {
            let mut name = String::new();
            let mut def_range = (0, 0);
            let mut body_range = None;
            let mut kind = SymbolKind::Unknown;

            for cap in m.captures {
                let cap_name = query.capture_names()[cap.index as usize];
                let node = cap.node;

                match cap_name {
                    "name" | "function.name" | "variable.name" | "type.name" => {
                        name = node.utf8_text(source_bytes).unwrap_or("").to_string();
                    }
                    "def" | "function.definition" => {
                        def_range = (node.start_byte(), node.end_byte());
                        let k = node.kind();

                        // Detailed Symbol Inference Logic
                        kind = if k == "class_member_definition"
                            || k == "lambda_expression"
                            || k.contains("function")
                            || k.contains("fn")
                            || k.contains("method")
                            || k.contains("macro")
                        {
                            SymbolKind::Function
                        } else if k == "service_declaration"
                            || k == "match_declaration"
                            || k.contains("class")
                            || k.contains("struct")
                            || k.contains("record")
                            || k.contains("object")
                            || k.contains("model")
                            || k.contains("enum")
                        {
                            SymbolKind::Class
                        } else if k.contains("interface")
                            || k.contains("trait")
                            || k.contains("impl")
                        {
                            SymbolKind::Interface
                        } else if k.contains("type") && !k.contains("type_identifier") {
                            // "type_alias_declaration" (TS) or "type_spec" (Go)
                            SymbolKind::Class
                        } else if k.contains("module")
                            || k.contains("namespace")
                            || k.contains("package")
                        {
                            SymbolKind::Module
                        } else if k.contains("const")
                            || k.contains("let")
                            || k.contains("var")
                            || k.contains("declarator")
                        {
                            // Check if this variable holds a function (JS/TS pattern: const foo = () => {})
                            let mut is_fn_var = false;

                            // Check immediate children for function signatures
                            let mut child_cursor = node.walk();
                            for child in node.children(&mut child_cursor) {
                                let ck = child.kind();
                                if ck == "arrow_function"
                                    || ck == "function_expression"
                                    || ck == "call_expression"
                                {
                                    is_fn_var = true;
                                    break;
                                }
                                // Handle: variable_declarator -> value: arrow_function
                                if child
                                    .child_by_field_name("value")
                                    .map(|v| v.kind().contains("function"))
                                    .unwrap_or(false)
                                {
                                    is_fn_var = true;
                                    break;
                                }
                            }

                            if is_fn_var {
                                SymbolKind::Function
                            } else {
                                SymbolKind::Variable
                            }
                        } else if k == "create_table" {
                            SymbolKind::Class // SQL Tables are data structures
                        } else if k == "binary_operator" {
                            // R assignments (foo <- function)
                            SymbolKind::Function
                        } else if k.contains("resource") {
                            SymbolKind::Class // HCL Resources
                        } else {
                            SymbolKind::Unknown
                        };
                    }
                    "body" | "function.body" => {
                        body_range = Some((node.start_byte(), node.end_byte()));
                    }
                    "function.kind" => {
                        // Special handling for HCL/Terraform blocks
                        let k_text = node.utf8_text(source_bytes).unwrap_or("");
                        match k_text {
                            "resource" | "data" => kind = SymbolKind::Class,
                            "variable" | "output" | "provider" => kind = SymbolKind::Variable,
                            "module" => kind = SymbolKind::Module,
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }

            if name.is_empty() {
                name = "anonymous".to_string();
            }

            // HCL fix: If name is strict matches of generic types, ignore them if we have better.
            if name == "resource" || name == "data" {
                continue;
            }

            if def_range.0 != def_range.1 {
                defs.push(Definition {
                    name,
                    kind,
                    range: def_range,
                    body_range,
                });
            }
        }
    }

    // 2. Extract Imports
    if let Some(query) = &config.queries.imports {
        let mut matches = cursor.matches(query, root_node, source_bytes);
        while let Some(m) = matches.next() {
            for cap in m.captures {
                let cap_name = query.capture_names()[cap.index as usize];
                if cap_name == "source" || cap_name == "import.source" {
                    let text = cap.node.utf8_text(source_bytes).unwrap_or("").to_string();
                    let clean = text.trim_matches(|c| c == '"' || c == '\'' || c == '`');
                    if !clean.is_empty() {
                        imports.push(clean.to_string());
                    }
                }
            }
        }
    }

    // 3. Extract References
    if let Some(query) = &config.queries.references {
        let mut matches = cursor.matches(query, root_node, source_bytes);
        while let Some(m) = matches.next() {
            for cap in m.captures {
                let text = cap.node.utf8_text(source_bytes).unwrap_or("");

                // Heuristic filtering for "Dumb" references to reduce noise
                if text.len() < 2 {
                    continue;
                }
                if text
                    .chars()
                    .next()
                    .map_or(false, |c| !c.is_alphabetic() && c != '_')
                {
                    continue;
                }

                symbol_refs.insert(text.to_string());
            }
        }
    }

    // 4. Extract String Literals
    // These are crucial for the Inference Engine to link code strings (e.g. "/api/users")
    // to Synthetic Definitions (e.g. route:/api/users).
    if let Some(query) = &config.queries.literals {
        let mut matches = cursor.matches(query, root_node, source_bytes);
        while let Some(m) = matches.next() {
            for cap in m.captures {
                let text = cap.node.utf8_text(source_bytes).unwrap_or("");
                // Clean the quotes off the literal
                let clean = text.trim_matches(|c| c == '"' || c == '\'' || c == '`');

                // Only store "interesting" literals to keep the index size manageable.
                // Short literals (1 char) or empty strings are rarely useful for concept linking.
                if clean.len() > 1 {
                    literals.insert(clean.to_string());
                }
            }
        }
    }

    if let Some(query) = &config.queries.frameworks {
        let mut matches = cursor.matches(query, root_node, source_bytes);
        let mut raw_hints: Vec<(String, Option<String>, (usize, usize))> = Vec::new();

        while let Some(m) = matches.next() {
            let mut key: Option<String> = None;
            let mut value: Option<String> = None;
            let mut range = (0, 0);

            for cap in m.captures {
                let cap_name = query.capture_names()[cap.index as usize];
                let node = cap.node;

                if cap_name == "framework.key" {
                    let text = node.utf8_text(source_bytes).unwrap_or("").to_string();
                    key = Some(text);
                    range = (node.start_byte(), node.end_byte());
                } else if cap_name == "framework.value" {
                    let text = node.utf8_text(source_bytes).unwrap_or("").to_string();
                    // We perform basic cleanup here (trimming quotes)
                    // so the Regex in the inference engine can be simpler.
                    let clean_text = text.trim_matches(|c| c == '"' || c == '\'' || c == '`');
                    value = Some(clean_text.to_string());
                }
            }

            if let Some(k) = key {
                raw_hints.push((k, value, range));
            }
        }

        // Deduplicate: for each key, keep the match with a value if available
        let mut seen_keys = std::collections::HashSet::new();
        for (k, v, range) in raw_hints {
            if seen_keys.contains(&k) {
                // Skip if we already have this key
                // But first check if we have a better match (with value)
                // We need to find if existing match has value
                let existing_empty = framework_hints
                    .iter()
                    .find(|h| h.key == k && h.value.is_empty());

                if v.is_some() && existing_empty.is_some() {
                    // Replace empty match with one that has value
                    framework_hints.retain(|h| h.key != k || h.value != "");
                    framework_hints.push(FrameworkHint {
                        key: k.clone(),
                        value: v.unwrap(),
                        range,
                    });
                }
                continue;
            }
            seen_keys.insert(k.clone());

            framework_hints.push(FrameworkHint {
                key: k,
                // If no value captured, default to empty string.
                // (e.g. marker decorators like @Injectable() might not capture a value)
                value: v.unwrap_or_default(),
                range,
            });
        }
    }

    // Cleanup: Remove self-definitions from references.
    // A file technically "references" itself when defining a function,
    // but for dependency graphs, we care about external dependencies.
    for def in &defs {
        symbol_refs.remove(&def.name);
    }

    Ok(FileBoundary {
        id: 0, // Assigned by Indexer later
        path: path.to_string(),
        root_id: String::new(),
        hash: file_hash,
        token_count, 
        is_test: false, // Default to false, Scanner will update this based on path
        defs,
        imports,
        symbol_refs: symbol_refs.into_iter().collect(),
        literals: literals.into_iter().collect(),
        framework_hints,
        synthetic_defs: Vec::new(),
    })
}
