//! Extracts local constant variable assignments from source code.
//! This is a pre-processing step used to resolve dynamic imports,
//! template strings, and other patterns that rely on local variables.

use std::collections::HashMap;
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};
use crate::analysis::language::LanguageConfig;

/// Scans the source code for constant variable assignments.
///
/// # Arguments
/// * `root_node` - The root node of the parsed tree-sitter tree.
/// * `source` - The raw byte slice of the source code.
/// * `language` - The tree-sitter language object.
/// * `config` - The language-specific configuration containing the query for values.
///
/// # Returns
/// A `HashMap` where the key is the variable name and the value is its string literal content.
pub fn extract_local_constants(
    root_node: Node,
    source: &[u8],
    language: &tree_sitter::Language,
    config: &LanguageConfig
) -> HashMap<String, String> {
    let mut constants: HashMap<String, String> = HashMap::new();

    if !config.query_vals.is_empty() {
        if let Ok(q) = Query::new(language, config.query_vals) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&q, root_node, source);
            // StreamingIterator must be in scope for this .next() call to work
            while let Some(m) = matches.next() {
                let mut name = String::new();
                let mut val = String::new();

                for cap in m.captures {
                    let text = cap.node.utf8_text(source).unwrap_or("").to_string();
                    let capture_name = q.capture_names()[cap.index as usize];

                    if capture_name == "val.name" {
                        name = text;
                    } else if capture_name == "val.value" {
                        val = text
                            .trim_matches(|c| c == '"' || c == '\'' || c == '`')
                            .to_string();
                    }
                }

                if !name.is_empty() && !val.is_empty() {
                    constants.insert(name, val);
                }
            }
        }
    }

    constants
}