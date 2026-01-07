mod common;
use common::TestWorkspace;
use blast_radius_engine::resolution::Indexer;
use blast_radius_engine::query::traversal::find_related_symbols;

#[test]
fn test_typescript_nestjs_injection() {
    let workspace = TestWorkspace::new();

    // 1. Interface
    workspace.create_file("cats.repository.ts", r#"
        export interface ICatsRepo {
            findAll(): string[];
        }
    "#);

    // 2. Implementation with Decorator
    workspace.create_file("cats.sql.ts", r#"
        import { ICatsRepo } from "./cats.repository";
        
        @Injectable()
        export class SqlCatsRepo implements ICatsRepo {
            findAll() { return ["cat1", "cat2"]; }
        }
    "#);

    // 3. Consumer (Service) using Constructor Injection
    workspace.create_file("cats.service.ts", r#"
        import { ICatsRepo } from "./cats.repository";

        @Injectable()
        export class CatsService {
            constructor(private repo: ICatsRepo) {}

            getAll() {
                return this.repo.findAll();
            }
        }
    "#);

    let mut indexer = Indexer::new();
    common::run_pipeline(&mut indexer, &workspace.path);

    // 4. Trace Context
    let related = find_related_symbols(&indexer.index, &indexer.lookup, &indexer.reverse_graph, "CatsService", None).unwrap();
    let names: Vec<String> = related.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();

    assert!(names.contains(&"SqlCatsRepo".to_string()), "DI should wire Interface usage to @Injectable implementation");
    assert!(names.contains(&"findAll".to_string()), "Should include methods from the specific implementation");
}

#[test]
fn test_java_spring_injection() {
    let workspace = TestWorkspace::new();

    workspace.create_file("src/UserService.java", r#"
        public interface UserService {
            void save();
        }
    "#);

    workspace.create_file("src/UserServiceImpl.java", r#"
        @Service
        public class UserServiceImpl implements UserService {
            public void save() { ... }
        }
    "#);

    // Field Injection simulation
    workspace.create_file("src/UserController.java", r#"
        @Controller
        public class UserController {
            @Autowired
            private UserService userService;

            public void register() {
                userService.save();
            }
        }
    "#);

    let mut indexer = Indexer::new();
    common::run_pipeline(&mut indexer, &workspace.path);

     let related = find_related_symbols(&indexer.index, &indexer.lookup, &indexer.reverse_graph, "UserController", None).unwrap();
    let names: Vec<String> = related.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();

    assert!(names.contains(&"UserServiceImpl".to_string()), "Spring @Service should be wired to @Controller usage");
}