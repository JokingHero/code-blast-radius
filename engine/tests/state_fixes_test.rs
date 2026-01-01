mod common;
use common::TestWorkspace;
use rfc_engine::resolution::{Indexer, pipeline::Pipeline};
use rfc_engine::query::traversal::find_related_symbols;

#[test]
fn test_redux_constant_propagation() {
    let workspace = TestWorkspace::new();

    // 1. Reducer using a local constant
    // The analyzer must resolve 'LOGIN_ACTION' -> 'AUTH/LOGIN' locally
    workspace.create_file("reducer.ts", r#"
        const LOGIN_ACTION = 'AUTH/LOGIN';
        
        export function authReducer(state, action) {
            switch (action.type) {
                case LOGIN_ACTION:
                    return { loggedIn: true };
                default: 
                    return state;
            }
        }
    "#);

    // 2. Dispatcher using a local constant (simulating an import or separate definition)
    // The analyzer must resolve 'CMD_LOGIN' -> 'AUTH/LOGIN' locally
    workspace.create_file("actions.ts", r#"
        const CMD_LOGIN = 'AUTH/LOGIN';

        export function doLogin() {
            dispatch({ type: CMD_LOGIN });
        }
    "#);

    let mut indexer = Indexer::new();
    let mut pipeline = Pipeline::new();
    pipeline.run(&mut indexer, &workspace.path);

    // 3. Verify linkage
    let related = find_related_symbols(&indexer.index, &indexer.lookup, &indexer.reverse_graph, "doLogin").unwrap();
    let names: Vec<String> = related.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();

    assert!(names.contains(&"authReducer".to_string()), 
        "Context should link doLogin -> authReducer via resolved constant 'AUTH/LOGIN'");
}

#[test]
fn test_vuex_commit_support() {
    let workspace = TestWorkspace::new();

    // 1. Vuex Store (Mutation Handler)
    workspace.create_file("store.js", r#"
        const mutations = {
            'INCREMENT_COUNT': (state) => {
                state.count++;
            }
        };
    "#);

    // 2. Vue Component (Trigger)
    // Uses store.commit() which is specific to Vuex
    workspace.create_file("Counter.vue", r#"
        export default {
            methods: {
                increment() {
                    // This uses 'commit' instead of 'dispatch'
                    this.$store.commit('INCREMENT_COUNT');
                }
            }
        }
    "#);

    let mut indexer = Indexer::new();
    let mut pipeline = Pipeline::new();
    pipeline.run(&mut indexer, &workspace.path);

    // 3. Verify linkage
    let related = find_related_symbols(&indexer.index, &indexer.lookup, &indexer.reverse_graph, "increment").expect("...");
    
    // We expect to find the file where 'INCREMENT_COUNT' is handled.
    let store_file_id = indexer.index.files.values()
        .find(|f| f.path.contains("store.js"))
        .unwrap().id;

    let linked_files: Vec<u32> = related.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().file_id)
        .collect();

    assert!(linked_files.contains(&store_file_id), 
        "Vuex commit('INCREMENT_COUNT') should link to store.js");
}

#[test]
fn test_mixed_dispatch_variable() {
    let workspace = TestWorkspace::new();

    // 1. Handler looks for string literal "USER_CLICK"
    workspace.create_file("events.js", r#"
        const handlers = {
            "USER_CLICK": () => console.log("clicked")
        };
    "#);

    // 2. Trigger uses a variable to hold "USER_CLICK"
    workspace.create_file("trigger.js", r#"
        const EVENT_NAME = "USER_CLICK";
        
        function run() {
            // Using 'emit' with a variable instead of a literal
            emitter.emit(EVENT_NAME);
        }
    "#);

    let mut indexer = Indexer::new();
    let mut pipeline = Pipeline::new();
    pipeline.run(&mut indexer, &workspace.path);

    let related = find_related_symbols(&indexer.index, &indexer.lookup, &indexer.reverse_graph, "run").unwrap();
    
    // Check if we found the definition file in the related context
    let events_file_found = related.iter().any(|id| {
        let fid = indexer.index.symbols.get(id).unwrap().file_id;
        let fpath = &indexer.index.files.get(&indexer.index.files.keys().find(|k| indexer.index.files[*k].id == fid).unwrap().clone()).unwrap().path;
        fpath.contains("events.js")
    });

    assert!(events_file_found, "Should link generic emit(VARIABLE) to handler string literal");
}