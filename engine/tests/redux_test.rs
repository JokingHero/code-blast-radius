mod common;
use common::TestWorkspace;
use blast_radius_engine::models::{EdgeKind}; // Removed StagingArea import
use blast_radius_engine::resolution::{Indexer, pipeline::Pipeline};
use blast_radius_engine::query::traversal::find_related_symbols;

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
    let mut pipeline = Pipeline::new();
    
    // CHANGE: Use .run() to ensure Scan -> Hydrate -> Resolve flow
    pipeline.run(&mut indexer, &workspace.path);

    // --- Assertions ---

    // 1. Check if symbols exist
    let reducer_ids = indexer.lookup.symbol_map.get("authReducer").expect("Reducer symbol not found");
    let handler_id = reducer_ids[0];

    let component_ids = indexer.lookup.symbol_map.get("handleLogin").expect("Component symbol not found");
    let caller_id = component_ids[0];

    // 2. Check Extraction (White-box testing the index)
    // CHANGE: Inspect persisted SymbolNode data instead of transient StagingArea
    let handler_sym = indexer.index.symbols.get(&handler_id).unwrap();
    assert!(handler_sym.handled_actions.contains(&"AUTH/LOGIN_SUCCESS".to_string()), "Reducer should persist handled actions");
    assert!(handler_sym.handled_actions.contains(&"AUTH/LOGOUT".to_string()));

    let caller_sym = indexer.index.symbols.get(&caller_id).unwrap();
    assert!(caller_sym.dispatched_actions.contains(&"AUTH/LOGIN_SUCCESS".to_string()), "Component should persist dispatched actions");

    // 3. Check Resolution (The Linkage)
    let edges = indexer.index.graph.get(&caller_id).unwrap();
    let is_linked = edges.iter().any(|e| e.target_id == handler_id && (e.kind == EdgeKind::Dispatches || e.kind == EdgeKind::Calls));
    
    assert!(is_linked, "handleLogin should be semantically linked to authReducer");

    // 4. Check Context Slice (End-to-End)
    // If I ask for "handleLogin", I should get "authReducer" in the result
    let related = find_related_symbols(&indexer.index, &indexer.lookup, &indexer.reverse_graph, "handleLogin", None).unwrap();
    let names: Vec<String> = related.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();

    assert!(names.contains(&"authReducer".to_string()), "Context for handleLogin should include authReducer");
}

#[test]
fn test_redux_object_map_linking() {
    let workspace = TestWorkspace::new();

    // 1. Object Map Reducer
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

    // 2. Dispatcher using 'put'
    workspace.create_file("sagas/todoSaga.js", r#"
        function* createTodoSaga() {
            yield put({ type: 'TODO/ADD', payload: 'New Item' });
        }
    "#);

    let mut indexer = Indexer::new();
    let mut pipeline = Pipeline::new();
    
    // CHANGE: Use .run() to ensure full pipeline execution including hydration
    pipeline.run(&mut indexer, &workspace.path);

    let saga_id = indexer.lookup.symbol_map.get("createTodoSaga").unwrap()[0];

    // --- LOGIC START ---
    
    // 1. Check the graph for outgoing edges from the Saga
    let edges = indexer.index.graph.get(&saga_id);
    
    // We expect *some* resolution. 
    assert!(edges.is_some(), "Saga should have resolved dependencies (edges)");
    let edges = edges.unwrap();
    assert!(!edges.is_empty(), "Saga should have at least one outgoing edge");

    // 2. Resolve the target IDs from the edges
    let resolved_target_ids: Vec<u32> = edges.iter()
        .map(|e| e.target_id)
        .collect();

    // 3. Verify linkage to the reducer file
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