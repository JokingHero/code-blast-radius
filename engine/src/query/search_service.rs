use crate::models::BoundaryIndex;
use nucleo_matcher::{Matcher, Config, Utf32String};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SearchResult {
    pub name: String,
    pub kind: String,
    pub path: String,
    pub score: u16,
}

pub struct SearchService;

impl SearchService {
    pub fn search(index: &BoundaryIndex, query: &str, limit: usize) -> Vec<SearchResult> {
        let mut matcher = Matcher::new(Config::DEFAULT);
        let mut results = Vec::new();
        let query_utf32 = Utf32String::from(query);

        // Search Symbols
        for (name, ids) in &index.symbol_map {
            if let Some(score) = matcher.fuzzy_match(
                Utf32String::from(name.as_str()).slice(..),
                query_utf32.slice(..),
            ) {
                // FIXED: Iterate over ALL file_ids, not just the first one.
                for &file_id in ids {
                    if let Some(file_node) = index.files.get(&file_id) {
                        // We must recalculate 'kind' for every file, because 
                        // 'init' might be a Function in one file and a Method in another.
                        let kind = file_node.defs.iter()
                            .find(|d| d.name == *name)
                            .map(|d| format!("{:?}", d.kind))
                            .or_else(|| {
                                if file_node.synthetic_defs.contains(name) {
                                    // "route:GET:/api" -> "ROUTE"
                                    Some(name.split(':').next().unwrap_or("Concept").to_uppercase())
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_else(|| "Unknown".to_string());

                        results.push(SearchResult {
                            name: name.clone(),
                            kind,
                            path: file_node.path.clone(),
                            score,
                        });
                    }
                }
            }
        }

        // Search Files
        for file in index.files.values() {
            if let Some(score) = matcher.fuzzy_match(
                Utf32String::from(file.path.as_str()).slice(..),
                query_utf32.slice(..),
            ) {
                results.push(SearchResult {
                    name: file.path.clone(),
                    kind: "File".to_string(),
                    path: file.path.clone(),
                    score,
                });
            }
        }

        // Sort by score first, then alphabetically by path to ensure deterministic order for identical scores
        results.sort_by(|a, b| {
            b.score.cmp(&a.score)
                .then_with(|| a.path.cmp(&b.path))
        });
        
        results.truncate(limit);
        results
    }
}