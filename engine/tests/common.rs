use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

// A wrapper around a temporary directory that acts as our "Workspace"
pub struct TestWorkspace {
    // We hold this field so the directory isn't deleted until the struct is dropped
    _temp: TempDir, 
    pub path: PathBuf,
}

impl TestWorkspace {
    pub fn new() -> Self {
        let temp = TempDir::new().expect("Failed to create temp dir");
        let path = temp.path().to_path_buf();
        Self { _temp: temp, path }
    }

    // Helper to create a file with specific content in the workspace
    pub fn create_file(&self, relative_path: &str, content: &str) {
        let file_path = self.path.join(relative_path);
        
        // Ensure parent directories exist (e.g., if path is "src/main.rs")
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).expect("Failed to create parent dirs");
        }

        let mut file = File::create(&file_path).expect("Failed to create file");
        file.write_all(content.as_bytes()).expect("Failed to write content");
    }
}