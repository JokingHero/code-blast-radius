mod common;
use common::TestWorkspace;
use blast_radius_engine::resolution::Indexer;

#[test]
fn test_pubsub_wildcards() {
    let workspace = TestWorkspace::new();

    // 1. Publisher (Exact Topic)
    workspace.create_file("publisher.ts", r#"
        eventBus.emit("user.created.v1", { id: 1 });
    "#);

    // 2. Subscriber A (Single level wildcard)
    // Should match "user.created.v1"
    workspace.create_file("sub_wildcard.ts", r#"
        eventBus.on("user.*.v1", () => {});
    "#);

    // 3. Subscriber B (Multi level wildcard)
    // Should match "user.created.v1"
    workspace.create_file("sub_deep.ts", r#"
        eventBus.on("user.#", () => {});
    "#);

    // 4. Unrelated Subscriber
    // Should NOT match
    workspace.create_file("sub_other.ts", r#"
        eventBus.on("order.*", () => {});
    "#);

    let mut indexer = Indexer::new();
    common::run_pipeline(&mut indexer, &workspace.path);

    let pub_id = indexer.index.files.values().find(|f| f.relative_path.contains("publisher.ts")).unwrap().id;
    let wild_id = indexer.index.files.values().find(|f| f.relative_path.contains("sub_wildcard.ts")).unwrap().id;
    let deep_id = indexer.index.files.values().find(|f| f.relative_path.contains("sub_deep.ts")).unwrap().id;
    let other_id = indexer.index.files.values().find(|f| f.relative_path.contains("sub_other.ts")).unwrap().id;

    let pub_deps = indexer.index.file_dependencies.get(&pub_id).expect("Publisher should have deps");

    // Assert connections
    assert!(pub_deps.contains(&wild_id), "Should link 'user.created.v1' to 'user.*.v1'");
    assert!(pub_deps.contains(&deep_id), "Should link 'user.created.v1' to 'user.#'");
    
    // Assert separation
    assert!(!pub_deps.contains(&other_id), "Should NOT link 'user.created.v1' to 'order.*'");
}

#[test]
fn test_slash_separator_wildcards() {
    let workspace = TestWorkspace::new();

    // MQTT style
    workspace.create_file("sensor.js", "mqtt.pub('home/kitchen/temp')");
    workspace.create_file("monitor.js", "mqtt.sub('home/+/temp')"); // + is standard MQTT single wildcard, assuming we map * to it or support it

    // Note: My implementation above supported `*` and `#`. 
    // If you want to support `+` for MQTT, update `matches_topic` to treat `+` like `*`.
    // For this test, let's use the implementation's supported `*`.
    
    workspace.create_file("monitor_wild.js", "mqtt.sub('home/*/temp')");

    let mut indexer = Indexer::new();
    common::run_pipeline(&mut indexer, &workspace.path);

    let sensor_id = indexer.index.files.values().find(|f| f.relative_path.contains("sensor.js")).unwrap().id;
    let mon_id = indexer.index.files.values().find(|f| f.relative_path.contains("monitor_wild.js")).unwrap().id;

    let deps = indexer.index.file_dependencies.get(&sensor_id).expect("Deps found");
    assert!(deps.contains(&mon_id));
}