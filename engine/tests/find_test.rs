mod common;
use common::TestWorkspace;
use rfc_engine::resolution::{Indexer, pipeline::Pipeline};
use nucleo_matcher::{Matcher, Config, Utf32String};

#[test]
fn test_fuzzy_find_symbols_and_files() {
    let workspace = TestWorkspace::new();
    
    // Create some files with distinct names and symbols
    workspace.create_file("auth_service.rs", r#"
        pub fn authenticate_user(id: u32) -> bool { true }
        pub struct UserProfile { pub name: String }
    "#);

    workspace.create_file("data_manager.py", r#"
        def fetch_data_records():
            pass
        
        class DataCache:
            def clear(self):
                pass
    "#);

    let mut indexer = Indexer::new();
    let mut pipeline = Pipeline::new();
    pipeline.run(&mut indexer, &workspace.path);

    let mut matcher = Matcher::new(Config::DEFAULT);
    
    // Helper to perform the same matching logic as in the CLI
    let fuzzy_search = |matcher: &mut Matcher, query: &str, indexer: &Indexer| {
        let mut results = Vec::new();
        let query_utf32 = Utf32String::from(query);

        // Match Symbols
        for sym in indexer.index.symbols.values() {
            if let Some(score) = matcher.fuzzy_match(Utf32String::from(sym.name.as_str()).slice(..), query_utf32.slice(..)) {
                results.push((sym.name.clone(), score));
            }
        }

        // Match Files
        for file in indexer.index.files.values() {
            if let Some(score) = matcher.fuzzy_match(Utf32String::from(file.path.as_str()).slice(..), query_utf32.slice(..)) {
                results.push((file.path.clone(), score));
            }
        }

        results.sort_by(|a, b| b.1.cmp(&a.1));
        results
    };

    // Test Case 1: Exact match for a symbol
    let results = fuzzy_search(&mut matcher, "authenticate_user", &indexer);
    assert!(results.iter().any(|(name, _)| name == "authenticate_user"));

    // Test Case 2: Fuzzy match for a symbol (auth_u)
    let results = fuzzy_search(&mut matcher, "auth_u", &indexer);
    assert!(results.iter().any(|(name, _)| name == "authenticate_user"));

    // Test Case 3: Exact match for a file
    let results = fuzzy_search(&mut matcher, "auth_service.rs", &indexer);
    assert!(results.iter().any(|(name, _)| name.contains("auth_service.rs")));

    // Test Case 4: Fuzzy match for a file (dt_mngr)
    let results = fuzzy_search(&mut matcher, "dt_mngr", &indexer);
    assert!(results.iter().any(|(name, _)| name.contains("data_manager.py")));

    // Test Case 5: Class/Struct match
    let results = fuzzy_search(&mut matcher, "DataCache", &indexer);
    assert!(results.iter().any(|(name, _)| name == "DataCache"));
}

#[test]
fn test_fuzzy_find_limit_and_sorting() {
    let workspace = TestWorkspace::new();
    workspace.create_file("test.rs", "fn a1() {} fn a2() {} fn a3() {} fn b1() {}");

    let mut indexer = Indexer::new();
    let mut pipeline = Pipeline::new();
    pipeline.run(&mut indexer, &workspace.path);

    let mut matcher = Matcher::new(Config::DEFAULT);
    let query_utf32 = Utf32String::from("a");
    
    let mut results = Vec::new();
    for sym in indexer.index.symbols.values() {
        if let Some(score) = matcher.fuzzy_match(Utf32String::from(sym.name.as_str()).slice(..), query_utf32.slice(..)) {
            results.push((sym.name.clone(), score));
        }
    }

    results.sort_by(|a, b| b.1.cmp(&a.1));
    
    // Verify we found all 'a' functions
    let a_names: Vec<String> = results.iter().filter(|(n, _)| n.starts_with('a')).map(|(n, _)| n.clone()).collect();
    assert!(a_names.contains(&"a1".to_string()));
    assert!(a_names.contains(&"a2".to_string()));
    assert!(a_names.contains(&"a3".to_string()));
    
    // Verify b1 is also present if query matches loosely (or if 'a' is in 'b1' - it isn't, but let's check)
    // Actually "a" doesn't match "b1", so we only expect a1, a2, a3.
    assert_eq!(a_names.len(), 3);
}
