pub mod routes;
use crate::models::FileBoundary;

pub trait InferenceRule: Send + Sync {
    /// Inspects a file's physical boundary and returns a list of logical 
    /// definitions derived from it.
    fn infer_definitions(&self, boundary: &FileBoundary) -> Vec<String>;
}

pub struct InferenceEngine {
    rules: Vec<Box<dyn InferenceRule>>,
}

impl InferenceEngine {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn register<R: InferenceRule + 'static>(&mut self, rule: R) {
        self.rules.push(Box::new(rule));
    }

    pub fn run(&self, boundary: &mut FileBoundary) {
        let mut new_defs = Vec::new();
        for rule in &self.rules {
            new_defs.extend(rule.infer_definitions(boundary));
        }
        boundary.synthetic_defs = new_defs;
    }
}