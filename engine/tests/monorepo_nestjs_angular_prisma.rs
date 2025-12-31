mod common;
use common::TestWorkspace;
use rfc_engine::resolution::Indexer;
use rfc_engine::query::traversal::find_related_symbols;

#[test]
fn test_scenario_monorepo_nest_angular_prisma() {
    let workspace = TestWorkspace::new();

    // ==========================================
    // 1. CONFIGURATION ( The Glue )
    // ==========================================

    // Root tsconfig defining the Monorepo aliases
    workspace.create_file("tsconfig.json", r#"
    {
        "compilerOptions": {
            "paths": {
                "@shared/*": ["libs/shared/src/*"]
            }
        }
    }
    "#);

    // ==========================================
    // 2. SHARED LIBRARY ( The Common Language )
    // ==========================================
    
    workspace.create_file("libs/shared/src/index.ts", r#"
        export * from './user.dto';
    "#);

    workspace.create_file("libs/shared/src/user.dto.ts", r#"
        export interface UserDto {
            id: string;
            email: string;
        }
    "#);

    // ==========================================
    // 3. BACKEND: NESTJS + PRISMA
    // ==========================================

    // -- Prisma Schema --
    workspace.create_file("apps/api/prisma/schema.prisma", r#"
        model User {
            id    String @id
            email String
        }
    "#);

    // -- App Module (The Container) --
    workspace.create_file("apps/api/src/app.module.ts", r#"
        import { Module } from '@nestjs/common';
        import { UsersController } from './users/users.controller';
        import { UsersService } from './users/users.service';

        @Module({
            controllers: [UsersController],
            providers: [UsersService]
        })
        export class AppModule {}
    "#);

    // -- Users Service (The Logic + DB Access) --
    workspace.create_file("apps/api/src/users/users.service.ts", r#"
        import { Injectable } from '@nestjs/common';
        // Mocking the prisma client import
        import { PrismaClient } from '@prisma/client'; 

        @Injectable()
        export class UsersService {
            private prisma = new PrismaClient();

            findAll() {
                // FINGERPRINT: 'prisma.user.findMany'
                // This connects Code -> Prisma Schema
                return this.prisma.user.findMany();
            }
        }
    "#);

    // -- Users Controller (The API Surface) --
    workspace.create_file("apps/api/src/users/users.controller.ts", r#"
        import { Controller, Get } from '@nestjs/common';
        import { UsersService } from './users.service';
        import { UserDto } from '@shared/user.dto';

        @Controller('users')
        export class UsersController {
            // DI: Injection via Constructor Type Hint
            constructor(private usersService: UsersService) {}

            @Get()
            getAll(): UserDto[] {
                // Call chain: Controller -> Service
                return this.usersService.findAll();
            }
        }
    "#);

    // ==========================================
    // 4. FRONTEND: ANGULAR
    // ==========================================

    // -- Data Service (The HTTP Client) --
    workspace.create_file("apps/web/src/app/data.service.ts", r#"
        import { Injectable } from '@angular/core';
        import { HttpClient } from '@angular/common/http';

        @Injectable({ providedIn: 'root' })
        export class DataService {
            constructor(private http: HttpClient) {}

            fetchUsers() {
                // IMPLICIT ROUTE: '/users' matches @Controller('users')
                return this.http.get('/users');
            }
        }
    "#);

    // -- App Component (The UI) --
    workspace.create_file("apps/web/src/app/app.component.ts", r#"
        import { Component, OnInit } from '@angular/core';
        import { DataService } from './data.service';
        import { UserDto } from '@shared/user.dto';

        @Component({
            selector: 'app-root',
            templateUrl: './app.component.html'
        })
        export class AppComponent implements OnInit {
            users: UserDto[] = [];

            // DI: Injecting the DataService
            constructor(private dataService: DataService) {}

            ngOnInit() {
                this.dataService.fetchUsers().subscribe(data => {
                    this.users = data;
                });
            }
        }
    "#);

    // ==========================================
    // 5. EXECUTION
    // ==========================================

    let mut indexer = Indexer::new();
    indexer.scan(&workspace.path);
    indexer.resolve_references();

    // ==========================================
    // 6. ASSERTIONS ( The Graph Integrity )
    // ==========================================

    // TEST A: Frontend Component -> Backend Controller
    // Start at the UI component. Can we see the Backend Controller in the context?
    // Path: AppComponent -> DataService -> (String Match "/users") -> UsersController
    let ui_related = find_related_symbols(&indexer, "AppComponent").unwrap();
    let ui_names: Vec<String> = ui_related.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();

    assert!(ui_names.contains(&"DataService".to_string()), "UI should link to DataService");
    assert!(ui_names.contains(&"UsersController".to_string()), "UI should link to Backend Controller via implicit route");

    // TEST B: Backend Controller -> Database
    // Start at the Controller. Can we see the Prisma Schema Model?
    // Path: UsersController -> UsersService -> (Prisma Fingerprint) -> User Model
    let api_related = find_related_symbols(&indexer, "UsersController").unwrap();
    let api_names: Vec<String> = api_related.iter()
        .map(|id| indexer.index.symbols.get(id).unwrap().name.clone())
        .collect();

    assert!(api_names.contains(&"UsersService".to_string()), "Controller should link to Service via DI");
    assert!(api_names.contains(&"findAll".to_string()), "Should see service methods");
    
    // Note: Assuming your prisma.rs resolver handles case-insensitivity or exact match.
    // In code: this.prisma.user -> Schema: model User
    // If your engine is case-sensitive, this assertion might need the code to match exactly or the engine to lower-case.
    // Based on your engine code, ensure resolve_database_references checks for "User" (from Schema) inside "prisma.user" (from Code).
    assert!(api_names.contains(&"User".to_string()), "Controller context should reach all the way to Prisma Model 'User'");

    // TEST C: Monorepo Shared Libs
    // Both sides import @shared/user.dto. Do they resolve to the same file?
    let dto_ids = indexer.index.symbol_map.get("UserDto").expect("UserDto should be indexed");
    // We might have multiple symbols (one for the definition, maybe import aliases), 
    // but the definition should be in libs/shared/src/user.dto.ts
    let dto_def = dto_ids.iter().find(|&&id| {
        let fid = indexer.index.symbols[&id].file_id;
        let path = &indexer.index.files.values().find(|f| f.id == fid).unwrap().path;
        path.contains("libs/shared")
    }).expect("Should find UserDto definition in libs/shared");

    assert_eq!(indexer.index.symbols[dto_def].name, "UserDto");
}