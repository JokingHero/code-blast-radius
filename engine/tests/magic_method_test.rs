mod common;
use common::TestWorkspace;
use blast_radius_engine::resolution::{Indexer, pipeline::Pipeline};
use blast_radius_engine::query::traversal::find_related_symbols;

#[test]
fn test_python_getattr_proxy() {
    let workspace = TestWorkspace::new();

    // 1. A Proxy Class with __getattr__
    workspace.create_file("proxy.py", r#"
        class APIProxy:
            def __init__(self, url):
                self.url = url

            # This catches calls like .get_users() or .post_data()
            def __getattr__(self, name):
                return lambda: print(f"Calling {name} on {self.url}")
    "#);

    // 2. Consumer Code using the magic methods
    workspace.create_file("main.py", r#"
        from proxy import APIProxy

        def run_workflow():
            client = APIProxy("http://api.com")
            
            # 'get_users' is NOT defined on APIProxy
            # But it should resolve to __getattr__
            client.get_users()
            
            # 'post_data' should also resolve to __getattr__
            client.post_data()
    "#);

    let mut indexer = Indexer::new();
    let mut pipeline = Pipeline::new();
    pipeline.run(&mut indexer, &workspace.path);

    // 3. Verify Linkage
    let related = find_related_symbols(&indexer.index, &indexer.lookup, &indexer.reverse_graph, "run_workflow").unwrap();
    let names: Vec<String> = related.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();

    assert!(names.contains(&"__getattr__".to_string()), 
        "Context for run_workflow should include __getattr__ because explicit methods get_users/post_data were missing");
    
    assert!(names.contains(&"APIProxy".to_string()), 
        "Context should include the Proxy class");
}