use blast_radius_engine::{models::{BoundaryIndex, FileBoundary}, query::walker::JitWalker};

#[test]
fn test_monorepo_alias_resolution() {
    let mut index = BoundaryIndex::new();
    index.package_map.insert("@my-org/ui".to_string(), "packages/ui".to_string());

    let producer_id = 1;
    let producer = FileBoundary {
        id: producer_id,
        path: "packages/ui/src/Button.tsx".to_string(),
        ..Default::default()
    };
    index.files.insert(producer_id, producer);

    let consumer_id = 2;
    let consumer = FileBoundary {
        id: consumer_id,
        path: "apps/web/src/App.tsx".to_string(),
        imports: vec!["@my-org/ui/src/Button".to_string()], 
        ..Default::default()
    };
    index.files.insert(consumer_id, consumer);
    index.usage_map.insert("button".to_string(), vec![consumer_id]);
    // ----------------------------------------

    let walker = JitWalker::new(&index);
    let impacted = walker.walk_impact(&[producer_id], 5);

    assert!(impacted.contains(&consumer_id));
}

#[test]
fn test_standard_relative_import() {
    let mut index = BoundaryIndex::new();

    let utils_id = 1;
    let utils = FileBoundary {
        id: utils_id,
        path: "src/utils.ts".to_string(),
        ..Default::default()
    };
    index.files.insert(utils_id, utils);

    let main_id = 2;
    let main = FileBoundary {
        id: main_id,
        path: "src/main.ts".to_string(),
        imports: vec!["./utils".to_string()],
        ..Default::default()
    };
    index.files.insert(main_id, main);
    index.usage_map.insert("utils".to_string(), vec![main_id]);
    // ----------------------------------------

    let walker = JitWalker::new(&index);
    let impacted = walker.walk_impact(&[utils_id], 1);

    assert!(impacted.contains(&main_id));
}