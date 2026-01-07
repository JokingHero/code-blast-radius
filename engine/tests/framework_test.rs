mod common;
use common::TestWorkspace;
use blast_radius_engine::resolution::Indexer;

#[test]
fn test_nextjs_api_linking() {
    let workspace = TestWorkspace::new();

    // 1. Implicit Route (Backend)
    // Note the path: pages/api/login.ts
    workspace.create_file("pages/api/login.ts", r#"
        export default function handler(req, res) { res.status(200).json({ ok: true }); }
    "#);

    // 2. Consumer (Frontend)
    // Implicitly calls it via string literal
    workspace.create_file("components/LoginForm.tsx", r#"
        async function doLogin() {
            await fetch('/api/login', { method: 'POST' });
        }
    "#);

    let mut indexer = Indexer::new();
    common::run_pipeline(&mut indexer, &workspace.path);

    let backend_id = indexer.index.files.values()
        .find(|f| f.relative_path.contains("pages/api/login.ts"))
        .unwrap().id;
    
    let frontend_id = indexer.index.files.values()
        .find(|f| f.relative_path.contains("LoginForm.tsx"))
        .unwrap().id;

    // Check File Dependencies
    let deps = indexer.index.file_dependencies.get(&frontend_id)
        .expect("Frontend should depend on Backend via implicit route");

    assert!(deps.contains(&backend_id), "Next.js implicit route not detected");
}