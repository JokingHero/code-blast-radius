use std::fs;
use std::path::Path;

/// Normalizes paths and strips Windows UNC prefixes (\\?\)
pub fn to_index_path(path: &Path) -> String {
    let abs_path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let path_str = abs_path.to_string_lossy().to_string();

    if path_str.starts_with(r"\\?\") {
        path_str[4..].to_string()
    } else {
        path_str
    }
}

pub fn is_test_path(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    let normalized = path_str.replace('\\', "/").to_lowercase();
    let path_obj = Path::new(&normalized);

    // 1. Check for directory names
    let has_test_folder = path_obj.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        matches!(
            s.as_ref(),
            "test"
                | "tests"
                | "__tests__"
                | "spec"
                | "specs"
                | "integration-test"
                | "fixtures"
                | "__fixtures__"
                | "mocks"
                | "__mocks__"
                | "stubs"
        )
    });

    if has_test_folder {
        return true;
    }

    // 2. Check for filename patterns
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    if filename.ends_with(".test.ts")
        || filename.ends_with(".test.tsx")
        || filename.ends_with(".spec.ts")
        || filename.ends_with(".spec.tsx")
        || filename.ends_with(".fixture.ts")
        || filename.ends_with(".mock.ts")
        || filename.ends_with(".test.js")
        || filename.ends_with(".test.jsx")
        || filename.ends_with(".spec.js")
        || filename.ends_with(".spec.jsx")
        || filename.ends_with("_test.rs")
        || filename.ends_with("_spec.rs")
        || filename == "test.rs"
        || filename.starts_with("test_")
        || filename.ends_with("_test.py")
        || filename.ends_with("test.java")
        || filename.ends_with("tests.java")
    {
        return true;
    }

    if (filename.contains("mock") || filename.contains("fixture"))
        && (filename.ends_with(".ts")
            || filename.ends_with(".js")
            || filename.ends_with(".rs")
            || filename.ends_with(".py"))
    {
        return true;
    }

    false
}
