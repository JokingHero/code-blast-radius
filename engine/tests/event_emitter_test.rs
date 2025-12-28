mod common;
use common::TestWorkspace;
use rfc_engine::indexer::Indexer;
use rfc_engine::analyzer::find_related_symbols;

#[test]
fn test_node_event_emitter() {
    let workspace = TestWorkspace::new();

    // 1. Emitter (Express style)
    workspace.create_file("server.js", r#"
        function createUser() {
            // New pattern: direct string argument
            app.emit('USER_SIGNUP', { id: 1 });
        }
    "#);

    // 2. Listener
    workspace.create_file("analytics.js", r#"
        function setupAnalytics() {
            // New pattern: 'on' with string argument
            app.on('USER_SIGNUP', (data) => {
                track(data);
            });
        }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    let related = find_related_symbols(&indexer.index, "createUser").unwrap();
    let names: Vec<String> = related.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();

    assert!(names.contains(&"setupAnalytics".to_string()), "Express emit/on pattern should link symbols");
}

#[test]
fn test_python_django_signals() {
    let workspace = TestWorkspace::new();

    // 1. Sender
    workspace.create_file("views.py", r#"
        def register(request):
            # Matches: signal.send("literal")
            user_signal.send("USER_REGISTERED", user=u)
    "#);

    // 2. Receiver
    workspace.create_file("signals.py", r#"
        @receiver("USER_REGISTERED")
        def handle_registration(sender, **kwargs):
            print("Email sent")
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    let related = find_related_symbols(&indexer.index, "register").unwrap();
    let names: Vec<String> = related.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();

    assert!(names.contains(&"handle_registration".to_string()), "Django signal send/receiver pattern should link symbols");
}

#[test]
fn test_rust_match_handler() {
    let workspace = TestWorkspace::new();

    workspace.create_file("main.rs", r#"
        fn run_event() {
            bus.emit("SYSTEM_BOOT");
        }

        fn process_events(event: &str) {
            match event {
                "SYSTEM_BOOT" => { init(); },
                "SHUTDOWN" => { stop(); },
                _ => {}
            }
        }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    let related = find_related_symbols(&indexer.index, "run_event").unwrap();
    let names: Vec<String> = related.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();

    assert!(names.contains(&"process_events".to_string()), "Rust emit/match pattern should link symbols");
}