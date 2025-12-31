mod common;
use common::TestWorkspace;
use rfc_engine::indexer::Indexer;
use rfc_engine::query::traversal::find_related_symbols;

#[test]
fn test_sql_schema_linking() {
    let workspace = TestWorkspace::new();

    // 1. The Schema (State)
    workspace.create_file("migrations/init.sql", r#"
        -- Create the users table
        CREATE TABLE users (
            id SERIAL PRIMARY KEY,
            email VARCHAR(255)
        );
    "#);

    // 2. The Code (Behavior) using a literal string to refer to the table
    workspace.create_file("src/repo.ts", r#"
        import { db } from "./db";
        
        export function findUser(id) {
            // The string "users" matches the table name
            return db.query("SELECT * FROM users WHERE id = $1", [id]);
        }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    // 3. Context Slice
    let related = find_related_symbols(&indexer.index, "findUser").unwrap();
    let names: Vec<String> = related.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();

    // 4. Assertions
    assert!(names.contains(&"findUser".to_string()));
    assert!(names.contains(&"users".to_string()), "Context should include the SQL Table 'users' based on the string literal match");
}

#[test]
fn test_prisma_linking() {
    let workspace = TestWorkspace::new();

    // 1. Prisma Schema
    workspace.create_file("schema.prisma", r#"
        model Order {
            id        Int     @id @default(autoincrement())
            product   String
        }
    "#);

    // 2. Application Code
    // Prisma Client usage typically looks like prisma.order.findMany
    // The tokenizer breaks `prisma.order` into identifier `prisma` and property `order`.
    // We rely on "order" matching the Model name "Order" (case-insensitive logic might be needed, but exact match for now).
    workspace.create_file("service.ts", r#"
        function getOrders() {
            return prisma.Order.findMany();
        }
    "#);

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    let related = find_related_symbols(&indexer.index, "getOrders").unwrap();
    let names: Vec<String> = related.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();

    assert!(names.contains(&"Order".to_string()), "Context should include Prisma Model 'Order'");
}