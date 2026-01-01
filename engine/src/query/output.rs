use serde::Serialize;
use crate::models::{WorkspaceIndex, FileId};
use std::collections::HashMap;

#[derive(Serialize)]
pub struct ContextOutput {
    pub target: String,
    pub files: Vec<FileContext>,
}

#[derive(Serialize)]
pub struct FileContext {
    pub path: String,
    pub language: String,
    pub is_test: bool,
    pub relevant_lines: Vec<LineRange>,
    pub content: String,
}

#[derive(Serialize)]
pub struct LineRange {
    pub start: usize, // 1-based
    pub end: usize,
}

struct RawByteRange {
    start: usize,
    end: usize,
}

pub fn generate_context_output(
    index: &WorkspaceIndex, 
    symbol_ids: &[u32]
) -> ContextOutput {
    
    // 1. Identify Target
    let mut target_name = "Unknown".to_string();
    if let Some(first) = symbol_ids.first() {
        if let Some(sym) = index.symbols.get(first) {
            target_name = sym.name.clone();
        }
    }

    // 2. Group by File
    let mut file_map: HashMap<FileId, Vec<RawByteRange>> = HashMap::new();
    for &symbol_id in symbol_ids {
        if let Some(symbol) = index.symbols.get(&symbol_id) {
            if symbol.is_external { continue; }
            file_map.entry(symbol.file_id).or_default().push(RawByteRange {
                start: symbol.range_start,
                end: symbol.range_end,
            });
        }
    }

    let mut output_files = Vec::new();

    // 3. Process Files
    for (file_id, mut ranges) in file_map {
        let file_node = match index.files.values().find(|f| f.id == file_id) {
            Some(f) => f,
            None => continue,
        };

        let source_code = match std::fs::read_to_string(&file_node.path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        // Determine language from extension for metadata
        let ext = std::path::Path::new(&file_node.path)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("txt")
            .to_string();

        // 4. Calculate Relevant Lines (Metadata)
        ranges.sort_by_key(|r| r.start);
        
        let mut merged_ranges: Vec<RawByteRange> = Vec::new();
        // Merge ranges closer than ~5 lines (approx 200 bytes) to reduce noise
        let merge_threshold = 200; 

        if !ranges.is_empty() {
            let mut current = RawByteRange { start: ranges[0].start, end: ranges[0].end };
            for next in ranges.iter().skip(1) {
                if next.start <= current.end + merge_threshold {
                    // Merge
                    current.end = std::cmp::max(current.end, next.end);
                } else {
                    // Push and start new
                    merged_ranges.push(current);
                    current = RawByteRange { start: next.start, end: next.end };
                }
            }
            merged_ranges.push(current);
        }

        // Convert Bytes to Lines
        // FIX: Handle edge case where prefix ends with newline
        let byte_to_line = |b: usize| -> usize {
            let slice = &source_code[..b.min(source_code.len())];
            let count = slice.lines().count();
            if slice.ends_with('\n') {
                count + 1
            } else {
                count.max(1)
            }
        };

        let mut final_line_ranges = Vec::new();
        for range in merged_ranges {
            final_line_ranges.push(LineRange {
                start: byte_to_line(range.start),
                end: byte_to_line(range.end)
            });
        }

        output_files.push(FileContext {
            path: file_node.path.clone(),
            language: ext,
            is_test: file_node.is_test,
            relevant_lines: final_line_ranges,
            content: source_code,
        });
    }

    ContextOutput {
        target: target_name,
        files: output_files,
    }
}