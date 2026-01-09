use serde::{Serialize, Deserialize};
use crate::models::{WorkspaceIndex, FileId};
use std::collections::HashMap;

/// Generic container for context results. 
/// T can be `FileContextMetadata` (for GUI lists) or `FileContent` (for CLI/Export).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContextOutput<T> {
    pub target: String,
    pub files: Vec<T>,
}

/// Lightweight metadata used for UI lists and lazy loading.
/// Does not contain the full file string.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileContextMetadata {
    pub file_id: FileId,
    pub path: String,
    pub root_name: Option<String>,
    pub language: String,
    pub is_test: bool,
    pub relevant_lines: Vec<LineRange>,
}

/// Heavyweight struct containing the actual source code.
/// Flattens metadata during serialization to maintain JSON compatibility.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileContent {
    #[serde(flatten)]
    pub metadata: FileContextMetadata,
    pub content: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LineRange {
    pub start: usize, // 1-based
    pub end: usize,
}

// Internal helper for range calculation
struct RawByteRange {
    start: usize,
    end: usize,
}

// XML generation is only possible if we have the content
impl ContextOutput<FileContent> {
    pub fn to_xml(&self) -> String {
        let mut xml = String::from("<documents>\n");
        
        for file in &self.files {
            xml.push_str(&format!(
                "  <document path=\"{}\" language=\"{}\">\n",
                escape_xml_attribute(&file.metadata.path),
                escape_xml_attribute(&file.metadata.language)
            ));
            
            xml.push_str("    <source_code>\n");
            xml.push_str(&escape_xml_content(&file.content));
            xml.push_str("\n    </source_code>\n");
            
            xml.push_str("  </document>\n");
        }
        
        xml.push_str("</documents>");
        xml
    }
}

pub fn escape_xml_attribute(input: &str) -> String {
    input.replace('&', "&amp;")
         .replace('<', "&lt;")
         .replace('>', "&gt;")
         .replace('"', "&quot;")
         .replace('\'', "&apos;")
}

pub fn escape_xml_content(input: &str) -> String {
    // For content, quotes don't strictly need escaping, but <, >, & do.
    input.replace('&', "&amp;")
         .replace('<', "&lt;")
         .replace('>', "&gt;")
}

/// Generates the full output (Metadata + Content).
/// Used by CLI 'Radius' command which requires immediate full output.
pub fn generate_context_output(
    index: &WorkspaceIndex, 
    symbol_ids: &[u32],
    id_map: &HashMap<FileId, std::path::PathBuf> // Added
) -> ContextOutput<FileContent> {
    
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

        // Resolve absolute path from id_map
        let abs_path = match id_map.get(&file_id) {
            Some(p) => p,
            None => continue, 
        };

        // We must read the file here because this function returns ContextOutput<FileContent>
        let source_code = match std::fs::read_to_string(abs_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        // Determine language from extension for metadata
        let ext = abs_path
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
        // Handle edge case where prefix ends with newline
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

        // Construct the Split Object
        let metadata = FileContextMetadata {
            file_id: file_node.id,
            path: file_node.relative_path.clone(), // Use relative path for UI
            root_name: None,
            language: ext,
            is_test: file_node.is_test,
            relevant_lines: final_line_ranges,
        };

        output_files.push(FileContent {
            metadata,
            content: source_code,
        });
    }

    // Sort by path for deterministic output
    output_files.sort_by(|a, b| a.metadata.path.cmp(&b.metadata.path));

    ContextOutput {
        target: target_name,
        files: output_files,
    }
}