mod common;
use common::TestWorkspace;
use blast_radius_engine::resolution::{Indexer, pipeline::Pipeline};
use blast_radius_engine::query::traversal::find_related_symbols;

#[test]
fn test_data_structure_dependency() {
    let workspace = TestWorkspace::new();

    // 1. Define the Noun (The Data Structure)
    workspace.create_file("types.ts", r#"
        export interface User {
            id: string;
            email: string;
        }
    "#);

    // 2. Define the Verb (The Function using the Noun)
    workspace.create_file("service.ts", r#"
        import { User } from "./types";

        export function sendEmail(u: User) {
            console.log(u.email);
        }
    "#);

    let mut indexer = Indexer::new();
    let mut pipeline = Pipeline::new();
    pipeline.run(&mut indexer, &workspace.path);

    // Case 1: Search for the Function -> Should see the Type
    let related_func = find_related_symbols(&indexer.index, &indexer.lookup, &indexer.reverse_graph, "sendEmail").unwrap();
    let names_func: Vec<String> = related_func.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();
    
    assert!(names_func.contains(&"User".to_string()), "Function context should include the Types it uses as arguments");

    // Case 2: Search for the Type -> Should see the Function (Impact Analysis)
    let related_type = find_related_symbols(&indexer.index, &indexer.lookup, &indexer.reverse_graph, "User").unwrap();
    let names_type: Vec<String> = related_type.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();

    assert!(names_type.contains(&"sendEmail".to_string()), "Type context should include functions that depend on it");
}

#[test]
fn test_python_noun_tracking() {
    let workspace = TestWorkspace::new();

    // 1. Define a Class (The Noun)
    workspace.create_file("models.py", r#"
class User:
    pass
"#);

    // 2. Define a Function using that Class as a Type Hint (The Verb)
    workspace.create_file("service.py", r#"
from models import User

def process_user(u: User) -> None:
    print(u)
"#);

    let mut indexer = Indexer::new();
    let mut pipeline = Pipeline::new();
    pipeline.run(&mut indexer, &workspace.path);

    // 3. Search for 'process_user' -> Context should contain 'User'
    let related = find_related_symbols(&indexer.index, &indexer.lookup, &indexer.reverse_graph, "process_user").expect("Should find symbol");
    let names: Vec<String> = related.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();

    assert!(names.contains(&"User".to_string()), "Python function context should include its type hints");
}