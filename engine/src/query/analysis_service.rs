use anyhow::{Result};
use std::collections::{HashMap, HashSet};
use crate::models::BoundaryIndex;
use crate::query::output::{generate_context_output, ContextOutput, FileContent};
use crate::query::walker::JitWalker;

pub struct AnalysisService;

impl AnalysisService {
    pub fn calculate_radius(
        index: &BoundaryIndex, 
        symbol_or_path: &str, 
        depth: usize, 
        exclude_tests: bool
    ) -> Result<ContextOutput<FileContent>> {
        
        // Resolve Input (Symbol or Path) to File IDs
        let matching_file_id = index
            .files
            .values()
            .find(|f| f.path.ends_with(symbol_or_path) || f.path == symbol_or_path)
            .map(|f| f.id);

        let mut start_ids = Vec::new();

        if let Some(fid) = matching_file_id {
            start_ids.push(fid);
        } else {
            if let Some(ids) = index.symbol_map.get(symbol_or_path) {
                start_ids.extend(ids.iter());
            } else {
                anyhow::bail!("Symbol or File not found: {}", symbol_or_path);
            }
        }

        let walker = JitWalker::new(index);
        
        // Use a HashSet to avoid duplicates immediately
        let mut related_ids = HashSet::new();
        for &id in &start_ids {
            related_ids.insert(id);
        }

        // Walk Dependencies (Who calls me?)
        let deps = walker.walk_dependencies(&start_ids, depth);
        related_ids.extend(deps);
        
        // Walk Impact (Who do I call?)
        let impacted = walker.walk_impact(&start_ids, depth);
        related_ids.extend(impacted);

        let final_ids: Vec<u32> = related_ids.into_iter().collect();

        // Filter Results
        let mut final_filtered_ids = final_ids.clone();

        if exclude_tests {
            final_filtered_ids.retain(|&id| {
                if let Some(f) = index.files.get(&id) {
                    !f.is_test
                } else {
                    true
                }
            });
        }

        // Generate Content
        // We create a map of ID -> Path for the output generator
        let id_map: HashMap<u32, String> = index
            .files
            .values()
            .map(|f| (f.id, f.path.clone()))
            .collect();

        let output = generate_context_output(index, &final_filtered_ids, &id_map);
        
        Ok(output)
    }
}