mod common;
use common::TestWorkspace;
use rfc_engine::models::{EdgeKind, StagingArea};
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
    let mut staging = StagingArea::default();
    indexer.scan(&workspace.path, &mut staging);
    indexer.resolve_references(&mut staging);

    // --- Assertions ---

    // 1. Check if symbols exist
    let reducer_ids = indexer.lookup.symbol_map.get("authReducer").expect("Reducer symbol not found");
    let handler_id = reducer_ids[0];

    let component_ids = indexer.lookup.symbol_map.get("handleLogin").expect("Component symbol not found");
    let caller_id = component_ids[0];

    // 2. Check Extraction (White-box testing the index)
    // The handler should have recorded the action string
    let handled = staging.raw_action_handlers.get(&handler_id).expect("Reducer should capture handled actions");
    assert!(handled.contains(&"AUTH/LOGIN_SUCCESS".to_string()));
    assert!(handled.contains(&"AUTH/LOGOUT".to_string()));

    // The dispatcher should have recorded the action string
    let dispatched = staging.raw_action_dispatches.get(&caller_id).expect("Component should capture dispatched actions");
    assert!(dispatched.contains(&"AUTH/LOGIN_SUCCESS".to_string()));

    // 3. Check Resolution (The Linkage)
    let edges = indexer.index.graph.get(&caller_id).unwrap();
    let is_linked = edges.iter().any(|e| e.target_id == handler_id && (e.kind == EdgeKind::Dispatches || e.kind == EdgeKind::Calls));
    
    assert!(is_linked, "handleLogin should be semantically linked to authReducer");

    // 4. Check Context Slice (End-to-End)
    // If I ask for "handleLogin", I should get "authReducer" in the result
    let related = find_related_symbols(&indexer.index, &indexer.lookup, &indexer.reverse_graph, "handleLogin").unwrap();
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
    let mut staging = StagingArea::default();
    indexer.scan(&workspace.path, &mut staging);
    indexer.resolve_references(&mut staging);

    let saga_id = indexer.lookup.symbol_map.get("createTodoSaga").unwrap()[0];

    // --- LOGIC START ---
    
    // 1. Check the graph for outgoing edges from the Saga
    let edges = indexer.index.graph.get(&saga_id);
    
    // We expect *some* resolution. 
    assert!(edges.is_some(), "Saga should have resolved dependencies (edges)");
    let edges = edges.unwrap();
    assert!(!edges.is_empty(), "Saga should have at least one outgoing edge");

    // 2. Resolve the target IDs from the edges
    // The resolver adds EdgeKind::Dispatches and implicit EdgeKind::Calls
    let resolved_target_ids: Vec<u32> = edges.iter()
        .map(|e| e.target_id)
        .collect();

    // Debugging output to see what we matched
    let targets: Vec<String> = resolved_target_ids.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();
    println!("Found targets for saga: {:?}", targets);

    // 3. Verify linkage to the reducer file
    // We expect the graph to link to a symbol defined inside "store/todos.js"
    let reducer_file_id = indexer.index.files.values()
        .find(|f| f.path.contains("todos.js"))
        .expect("Reducer file should be indexed")
        .id;

    let links_to_reducer_file = resolved_target_ids.iter().any(|&tid| {
        let sym = indexer.index.symbols.get(&tid).unwrap();
        sym.file_id == reducer_file_id
    });

    assert!(links_to_reducer_file, "Saga should link to todoReducer file via action matching");
}