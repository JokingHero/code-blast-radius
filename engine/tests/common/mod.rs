use blast_radius_engine::analysis::boundary::extract_boundary;
use blast_radius_engine::analysis::language::get_config_for_extension;
use blast_radius_engine::inference::conventions::ConventionEngine;
use blast_radius_engine::inference::frameworks::FrameworkManager;
use blast_radius_engine::inference::{configs, InferenceEngine};
use blast_radius_engine::models::{BoundaryIndex, FileId};
use blast_radius_engine::query::walker::JitWalker;
use std::collections::HashMap;

pub struct TestWorkspace {
    pub index: BoundaryIndex,
    pub inference_engine: InferenceEngine,
    pub path_to_id: HashMap<String, FileId>,
}

impl TestWorkspace {
    pub fn new() -> Self {
        let mut inference_engine = InferenceEngine::new();

        // Register Conventions
        inference_engine.register(ConventionEngine::new());

        // Register Frameworks
        let mut fw_manager = FrameworkManager::new();
        configs::register_all(&mut fw_manager);
        inference_engine.register(fw_manager);

        Self {
            index: BoundaryIndex::new(),
            inference_engine,
            path_to_id: HashMap::new(),
        }
    }

    pub fn add_file(&mut self, path: &str, content: &str) -> FileId {
        let extension = std::path::Path::new(path)
            .extension()
            .and_then(|s| s.to_str())
            .expect("File must have extension");

        let config = get_config_for_extension(extension).expect("Language not supported in test");

        // 1. Parse (Tree-sitter)
        let mut boundary = extract_boundary(
            path, content, config, [0; 32], // Dummy hash
        )
        .expect("Failed to extract boundary");

        // 2. Infer (Frameworks)
        self.inference_engine.run(&mut boundary);

        // 3. Index
        let id = self.index.next_file_id;
        self.index.next_file_id += 1;
        boundary.id = id;

        self.index.files.insert(id, boundary);
        self.path_to_id.insert(path.to_string(), id);

        id
    }

    pub fn rebuild_index(&mut self) {
        // Replicate the logic from scanner.rs rebuild_maps
        self.index.symbol_map.clear();
        self.index.usage_map.clear();

        for file in self.index.files.values() {
            // Index Definitions
            for def in &file.defs {
                self.index
                    .symbol_map
                    .entry(def.name.clone())
                    .or_default()
                    .push(file.id);
            }
            for syn in &file.synthetic_defs {
                self.index
                    .symbol_map
                    .entry(syn.clone())
                    .or_default()
                    .push(file.id);
            }

            // Index Usages (Literals & References -> Framework Keys)
            // JitWalker uses lowercase anchors for candidate selection, so we must index usages as lowercase.
            for literal in &file.literals {
                self.index
                    .usage_map
                    .entry(literal.to_lowercase())
                    .or_default()
                    .push(file.id);
                // Also index generic token usage (for standard imports)
                if let Some(token) = simple_token_extract(literal) {
                    self.index.usage_map.entry(token).or_default().push(file.id);
                }
            }
            // Fix: Index References too! (For Java types, HTML tags, etc.)
            for reference in &file.symbol_refs {
                self.index
                    .usage_map
                    .entry(reference.to_lowercase())
                    .or_default()
                    .push(file.id);
            }

            // Index Imports
            for imp in &file.imports {
                if let Some(token) = simple_token_extract(imp) {
                    self.index.usage_map.entry(token).or_default().push(file.id);
                }
            }
        }
    }

    pub fn assert_connected(&self, from: &str, to: &str) {
        let from_id = *self.path_to_id.get(from).expect("Source file not found");
        let to_id = *self.path_to_id.get(to).expect("Target file not found");

        let walker = JitWalker::new(&self.index);
        let impacted = walker.walk_impact(&[from_id], 5);

        assert!(
            impacted.contains(&to_id),
            "Files not connected!\nSource: {}\nTarget: {}\nDefs in Source: {:?}\nSynthetic Defs in Source: {:?}\nLiterals in Target: {:?}\nRefs in Target: {:?}",
            from,
            to,
            self.index.files.get(&from_id).unwrap().defs,
            self.index.files.get(&from_id).unwrap().synthetic_defs,
            self.index.files.get(&to_id).unwrap().literals,
            self.index.files.get(&to_id).unwrap().symbol_refs
        );
    }
}

fn simple_token_extract(s: &str) -> Option<String> {
    std::path::Path::new(s)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
}
