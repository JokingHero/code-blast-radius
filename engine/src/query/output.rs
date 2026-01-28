use crate::models::{BoundaryIndex, FileId};
use serde::{Deserialize, Serialize};
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
    pub token_count: u32,
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
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn escape_xml_content(input: &str) -> String {
    // For content, quotes don't strictly need escaping, but <, >, & do.
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Generates the full output (Metadata + Content).
/// Used by CLI 'Radius' command which requires immediate full output.
pub fn generate_context_output(
    index: &BoundaryIndex,
    file_ids: &[FileId],
    id_map: &HashMap<FileId, String>,
) -> ContextOutput<FileContent> {
    // 1. Identify Target
    // Just use the first file name
    let mut target_name = "Unknown".to_string();
    if let Some(first_id) = file_ids.first() {
        if let Some(f) = index.files.get(first_id) {
            target_name = f.path.clone();
        }
    }

    let mut output_files = Vec::new();

    // 2. Process Files
    for &file_id in file_ids {
        let file_node = match index.files.get(&file_id) {
            Some(f) => f,
            None => continue,
        };

        // We attempt to find the absolute path in the provided map
        // If not found, we check if the relative path exists in CWD (Ad-hoc mode fallback)
        let mut source_code = String::new();

        if let Some(abs_path_str) = id_map.get(&file_id) {
            if let Ok(content) = std::fs::read_to_string(abs_path_str) {
                source_code = content;
            }
        }

        if source_code.is_empty() {
            // Fallback: Try reading relative path from CWD
            if let Ok(content) = std::fs::read_to_string(&file_node.path) {
                source_code = content;
            }
        }

        if source_code.is_empty() {
            source_code = "// Failed to read file content".to_string();
        }

        // Determine language from extension for metadata
        let ext = std::path::Path::new(&file_node.path)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("txt")
            .to_string();

        let line_count = source_code.lines().count();
        let token_count = (source_code.len() / 4) as u32;
        // Construct the Split Object
        let metadata = FileContextMetadata {
            file_id: file_node.id,
            path: file_node.path.clone(),
            root_name: None,
            language: ext,
            is_test: file_node.is_test, 
            relevant_lines: vec![LineRange {
                start: 1,
                end: line_count.max(1),
            }],
            token_count,
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
