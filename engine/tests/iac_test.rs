mod common;
use common::TestWorkspace;
use rfc_engine::indexer::Indexer;

#[test]
fn test_environment_bridge() {
    let workspace = TestWorkspace::new();

    // 1. Application Code (JavaScript)
    // Uses process.env.MY_BUCKET_NAME
    workspace.create_file("src/uploader.js", r#"
        const bucket = process.env.MY_BUCKET_NAME;
        s3.putObject({ Bucket: bucket });
    "#);

    // 2. Infrastructure Code (Terraform)
    // Passes the literal "MY_BUCKET_NAME" to the container definition
    workspace.create_file("infra/main.tf", r#"
        resource "aws_ecs_task_definition" "app" {
            container_definitions = <<DEFINITION
            [
              {
                "environment": [
                  {"name": "MY_BUCKET_NAME", "value": "${aws_s3_bucket.main.id}"}
                ]
              }
            ]
            DEFINITION
        }

        resource "aws_s3_bucket" "main" {
            bucket = "my-production-bucket"
        }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    let js_id = indexer.index.files.values()
        .find(|f| f.path.contains("uploader.js")).unwrap().id;
    let tf_id = indexer.index.files.values()
        .find(|f| f.path.contains("main.tf")).unwrap().id;

    let deps = indexer.index.file_dependencies.get(&js_id).expect("JS file should have deps");
    
    assert!(deps.contains(&tf_id), 
        "JS file should be linked to Terraform file via shared ENV VAR literal 'MY_BUCKET_NAME'");
}

#[test]
fn test_cloud_resource_heuristic() {
    let workspace = TestWorkspace::new();

    // 1. App imports AWS SDK
    workspace.create_file("src/storage.ts", r#"
        import { S3 } from "@aws-sdk/client-s3";
        const client = new S3();
    "#);

    // 2. Terraform defines an S3 bucket
    // Note: No shared variable names here! Only the "Type" relationship.
    workspace.create_file("infra/storage.tf", r#"
        resource "aws_s3_bucket" "uploads" {
            bucket = "user-uploads"
        }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    let ts_id = indexer.index.files.values()
        .find(|f| f.path.contains("storage.ts")).unwrap().id;
    let tf_id = indexer.index.files.values()
        .find(|f| f.path.contains("storage.tf")).unwrap().id;

    let deps = indexer.index.file_dependencies.get(&ts_id).expect("TS file should have deps");
    
    assert!(deps.contains(&tf_id), 
        "App using AWS SDK S3 should be loosely linked to Terraform defining aws_s3_bucket");
}