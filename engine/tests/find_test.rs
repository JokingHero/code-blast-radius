use blast_radius_engine::workspace::WorkspaceManager;
use nucleo_matcher::{Config, Matcher, Utf32String};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_fuzzy_find_symbols_and_files() {
    let dir = tempdir().unwrap();
    let root_path = dir.path().to_path_buf();

    // 1. Create some files with distinct names and symbols (using simplified simple syntax)
    // Rust
    fs::write(
        root_path.join("auth_service.rs"),
        r#"
        pub fn authenticate_user(id: u32) -> bool { true }
        pub struct UserProfile { pub name: String }
    "#,
    )
    .unwrap();

    // Python - simplified for test stability
    fs::write(
        root_path.join("data_manager.py"),
        r#"
        def fetch_data_records():
            pass
        
        class DataCache:
            def clear(self):
                pass
    "#,
    )
    .unwrap();

    // 2. Initialize Manager (In-Memory linked to temp dir)
    let manager =
        WorkspaceManager::new_in_memory(vec![root_path.clone()]).expect("Failed to init workspace");
    // Sync happens in new_in_memory

    let index = &manager.index;

    let mut matcher = Matcher::new(Config::DEFAULT);

    // Helper to perform the same matching logic as in the CLI/App
    // Note: We access index directly
    let fuzzy_search = |matcher: &mut Matcher, query: &str| {
        let mut results = Vec::new();
        let query_utf32 = Utf32String::from(query);

        // Match Symbols
        for (name, _) in &index.symbol_map {
            if let Some(score) = matcher.fuzzy_match(
                Utf32String::from(name.as_str()).slice(..),
                query_utf32.slice(..),
            ) {
                results.push((name.clone(), score));
            }
        }

        // Match Files
        for file in index.files.values() {
            // Match relative path
            if let Some(score) = matcher.fuzzy_match(
                Utf32String::from(file.path.as_str()).slice(..),
                query_utf32.slice(..),
            ) {
                results.push((file.path.clone(), score));
            }
        }

        results.sort_by(|a, b| b.1.cmp(&a.1));
        results
    };

    // Test Case 1: Exact match for a symbol
    let results = fuzzy_search(&mut matcher, "authenticate_user");
    assert!(
        results.iter().any(|(name, _)| name == "authenticate_user"),
        "Failed to find authenticate_user"
    );

    // Test Case 2: Fuzzy match for a symbol (auth_u)
    let results = fuzzy_search(&mut matcher, "auth_u");
    assert!(
        results.iter().any(|(name, _)| name == "authenticate_user"),
        "Failed fuzzy match auth_u"
    );

    // Test Case 3: Exact match for a file
    let results = fuzzy_search(&mut matcher, "auth_service.rs");
    // Note: file.path will be relative to root, e.g. "auth_service.rs" (since it's at root of root)
    // Actually FileScanner calculates relative path from root.
    assert!(
        results
            .iter()
            .any(|(name, _)| name.contains("auth_service.rs")),
        "Failed to find file auth_service.rs"
    );

    // Test Case 4: Fuzzy match for a file (dt_mngr)
    let results = fuzzy_search(&mut matcher, "dt_mngr");
    assert!(
        results
            .iter()
            .any(|(name, _)| name.contains("data_manager.py")),
        "Failed to fuzzy find data_manager.py"
    );

    // Test Case 5: Class/Struct match
    let results = fuzzy_search(&mut matcher, "DataCache");
    assert!(
        results.iter().any(|(name, _)| name == "DataCache"),
        "Failed to find DataCache"
    );
}

#[test]
fn test_fuzzy_find_limit_and_sorting() {
    let dir = tempdir().unwrap();
    let root_path = dir.path().to_path_buf();

    fs::write(
        root_path.join("test.rs"),
        "fn a1() {} fn a2() {} fn a3() {} fn b1() {}",
    )
    .unwrap();

    let manager =
        WorkspaceManager::new_in_memory(vec![root_path.clone()]).expect("Failed to init workspace");
    let index = &manager.index;

    let mut matcher = Matcher::new(Config::DEFAULT);
    let query_utf32 = Utf32String::from("a");

    let mut results = Vec::new();
    // Only search symbols for this test
    for (name, _) in &index.symbol_map {
        if let Some(score) = matcher.fuzzy_match(
            Utf32String::from(name.as_str()).slice(..),
            query_utf32.slice(..),
        ) {
            results.push((name.clone(), score));
        }
    }

    results.sort_by(|a, b| b.1.cmp(&a.1));

    // Verify we found all 'a' functions
    let a_names: Vec<String> = results
        .iter()
        .filter(|(n, _)| n.starts_with('a'))
        .map(|(n, _)| n.clone())
        .collect();
    assert!(a_names.contains(&"a1".to_string()));
    assert!(a_names.contains(&"a2".to_string()));
    assert!(a_names.contains(&"a3".to_string()));

    // Verify b1 is NOT present in "a*" prefix filter (though fuzzy match might find it given score, but purely checking if we found at least the 3 expected)
    assert!(a_names.len() >= 3);
}
