mod common;
use common::TestWorkspace;
use rfc_engine::resolution::Indexer;
use rfc_engine::query::traversal::find_related_symbols;

#[test]
fn test_redux_switch_case_linking() {
    let workspace = TestWorkspace::new();

    // 1. The Reducer (The Handler)
    // Matches: (switch_case value: (string)) @action.handle
    workspace.create_file("store/reducer.ts", r#"
        export function authReducer(state = {}, action) {
            switch (action.type) {
                case 'AUTH/LOGIN_SUCCESS':
                    return { ...state, user: action.payload };
                case 'AUTH/LOGOUT':
                    return {};
                default:
                    return state;
            }
        }
    "#);

    // 2. The Component (The Dispatcher)
    // Matches: dispatch({ type: 'AUTH/LOGIN_SUCCESS' }) @action.dispatch
    workspace.create_file("components/LoginButton.tsx", r#"
        function handleLogin() {
            const data = { id: 1 };
            // This call should be linked to authReducer via the action string
            dispatch({ 
                type: 'AUTH/LOGIN_SUCCESS', 
                payload: data 
            });
        }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    // --- Assertions ---

    // 1. Check if symbols exist
    let reducer_ids = indexer.index.symbol_map.get("authReducer").expect("Reducer symbol not found");
    let handler_id = reducer_ids[0];

    let component_ids = indexer.index.symbol_map.get("handleLogin").expect("Component symbol not found");
    let caller_id = component_ids[0];

    // 2. Check Extraction (White-box testing the index)
    // The handler should have recorded the action string
    let handled = indexer.index.raw_action_handlers.get(&handler_id).expect("Reducer should capture handled actions");
    assert!(handled.contains(&"AUTH/LOGIN_SUCCESS".to_string()));
    assert!(handled.contains(&"AUTH/LOGOUT".to_string()));

    // The dispatcher should have recorded the action string
    let dispatched = indexer.index.raw_action_dispatches.get(&caller_id).expect("Component should capture dispatched actions");
    assert!(dispatched.contains(&"AUTH/LOGIN_SUCCESS".to_string()));

    // 3. Check Resolution (The Linkage)
    let resolved = indexer.index.resolved_calls.get(&caller_id).expect("Dispatch call should be resolved");
    assert!(resolved.contains(&handler_id), "handleLogin should be semantically linked to authReducer");

    // 4. Check Context Slice (End-to-End)
    // If I ask for "handleLogin", I should get "authReducer" in the result
    let related = find_related_symbols(&indexer.index, "handleLogin").unwrap();
    let names: Vec<String> = related.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();

    assert!(names.contains(&"authReducer".to_string()), "Context for handleLogin should include authReducer");
}

#[test]
fn test_redux_object_map_linking() {
    let workspace = TestWorkspace::new();

    // 1. Object Map Reducer (Common in older Redux or custom frameworks)
    // Matches: (pair key: (string) @action.handle)
    workspace.create_file("store/todos.js", r#"
        const todoHandlers = {
            'TODO/ADD': (state, action) => {
                state.push(action.payload);
            },
            'TODO/REMOVE': (state, action) => {
                state.pop();
            }
        };

        function todoReducer(state, action) {
            const handler = todoHandlers[action.type];
            return handler ? handler(state, action) : state;
        }
    "#);

    // 2. Dispatcher using 'put' (redux-saga style) or generic 'dispatch'
    workspace.create_file("sagas/todoSaga.js", r#"
        function* createTodoSaga() {
            // Using 'put' instead of 'dispatch' to test regex flexibility
            yield put({ type: 'TODO/ADD', payload: 'New Item' });
        }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    let saga_id = indexer.index.symbol_map.get("createTodoSaga").unwrap()[0];

    // We expect to find the specific arrow function inside todoHandlers, 
    // OR the container variable depending on how the tree-sitter ownership logic resolved it.
    // In our analyzer logic: 
    // Since `todoHandlers` is a variable declaration, the object keys might be attached to `todoHandlers` 
    // or (if we parsed arrow functions as definitions) to the anonymous arrow function symbols.
    // Let's verify via `resolved_calls`.

    let resolved = indexer.index.resolved_calls.get(&saga_id);
    
    // We expect *some* resolution. 
    assert!(resolved.is_some(), "Saga should have resolved dependencies");
    
    // Let's trace what it found
    let targets: Vec<String> = resolved.unwrap().iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();

    println!("Found targets for saga: {:?}", targets);

    // It should find either "anonymous" (the arrow function) or the file module, 
    // basically establishing a link to `store/todos.js`.
    // The most important part is that a link exists between the files/symbols.
    assert!(!targets.is_empty(), "Should resolve link to reducer handler");
    
    // Verify the handler file ID is in the resolved list's file IDs
    let target_file_ids: Vec<u32> = resolved.unwrap().iter()
        .map(|id| indexer.index.symbols[id].file_id)
        .collect();

    let reducer_file_id = indexer.index.files.values()
        .find(|f| f.path.contains("todos.js"))
        .unwrap().id;

    assert!(target_file_ids.contains(&reducer_file_id), "Saga should link to todoReducer file via action matching");
}