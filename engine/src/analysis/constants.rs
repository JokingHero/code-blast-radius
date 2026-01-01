use std::collections::HashMap;
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};
use crate::analysis::language::LanguageConfig;

pub fn extract_local_constants(
    root_node: Node,
    source: &[u8],
    language: &tree_sitter::Language,
    config: &LanguageConfig
) -> HashMap<String, String> {
    let mut constants: HashMap<String, String> = HashMap::new();

    // Access: config.queries.vals
    if let Some(query_str) = config.queries.vals {
        if let Ok(q) = Query::new(language, query_str) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&q, root_node, source);
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