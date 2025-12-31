mod common;
use common::TestWorkspace;
use rfc_engine::indexer::Indexer;
use rfc_engine::query::traversal::find_related_symbols;

#[test]
fn test_python_getattr_linking() {
    let workspace = TestWorkspace::new();

    // 1. The Service (The Noun)
    workspace.create_file("services.py", r#"
class UserService:
    def delete_user(self):
        pass
    def create_user(self):
        pass
"#);

    // 2. The Controller (The Verb using Reflection)
    workspace.create_file("controller.py", r#"
from services import UserService

def handle_request(action: str):
    # We instantiate explicitly so type inference works
    service = UserService()
    
    # Dynamic dispatch!
    # The tool should see 'getattr(service, ...)' and link 'service' -> 'UserService'
    method = getattr(service, action)
    method()
"#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    let related = find_related_symbols(&indexer.index, "handle_request").unwrap();
    let names: Vec<String> = related.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();

    // We expect the Class itself to be linked because of the wildcard match
    assert!(names.contains(&"UserService".to_string()), 
        "Reflection using getattr should link to the Service class via type sniffing");
}