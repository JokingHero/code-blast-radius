mod common;
use common::TestWorkspace;
use blast_radius_engine::resolution::{Indexer, pipeline::Pipeline};
use blast_radius_engine::query::traversal::find_related_symbols;
use std::collections::HashSet;

#[test]
fn test_monorepo_vertical_isolation() {
    let workspace = TestWorkspace::new();

    // ==========================================
    // 0. CONFIG & SHARED KERNEL
    // ==========================================
    workspace.create_file("tsconfig.json", r#"{ "compilerOptions": { "paths": { "@shared/*": ["libs/shared/*"] } } }"#);

    // [SHARED] This should appear in BOTH contexts
    workspace.create_file("libs/shared/base.entity.ts", r#"
        export class BaseEntity {
            id: string;
            createdAt: Date;
        }
    "#);

    // ==========================================
    // 1. DATABASE LAYER (PRISMA)
    // ==========================================
    // A single file containing both models. The engine must distinguish them.
    workspace.create_file("prisma/schema.prisma", r#"
        model User {
            id String @id
            email String
        }

        model Order {
            id String @id
            total Float
        }
    "#);

    // ==========================================
    // 2. DOMAIN A: USERS (The Target)
    // ==========================================

    // [BACKEND]
    workspace.create_file("apps/api/src/users/users.service.ts", r#"
        import { Injectable } from '@nestjs/common';
        import { PrismaClient } from '@prisma/client';

        @Injectable()
        export class UsersService {
            prisma = new PrismaClient();
            
            findMany() {
                // Link: Code -> Prisma Model 'User'
                return this.prisma.user.findMany();
            }
        }
    "#);

    workspace.create_file("apps/api/src/users/users.controller.ts", r#"
        import { Controller, Get } from '@nestjs/common';
        import { UsersService } from './users.service';

        @Controller('users')
        export class UsersController {
            constructor(private service: UsersService) {}

            @Get()
            getUsers() { return this.service.findMany(); }
        }
    "#);

    // [FRONTEND]
    workspace.create_file("apps/web/src/app/users/user.service.ts", r#"
        import { Injectable } from '@angular/core';
        import { HttpClient } from '@angular/common/http';
        import { BaseEntity } from '@shared/base.entity'; // Shared usage

        @Injectable()
        export class UserService {
            constructor(private http: HttpClient) {}

            list() {
                // Link: Frontend -> Backend Route '/users'
                return this.http.get<BaseEntity[]>('/users');
            }
        }
    "#);

    workspace.create_file("apps/web/src/app/users/user-profile.component.html", r#"
        <div *ngFor="let user of users">{{ user.id }}</div>
    "#);

    workspace.create_file("apps/web/src/app/users/user-profile.component.ts", r#"
        import { Component } from '@angular/core';
        import { UserService } from './user.service';

        @Component({
            selector: 'app-user-profile',
            templateUrl: './user-profile.component.html' // Link: TS -> HTML
        })
        export class UserProfileComponent {
            constructor(private api: UserService) {}

            load() {
                this.api.list().subscribe();
            }
        }
    "#);

    // ==========================================
    // 3. DOMAIN B: ORDERS (The Noise)
    // ==========================================
    // This looks almost identical structure-wise, but uses distinct names/routes.

    // [BACKEND]
    workspace.create_file("apps/api/src/orders/orders.service.ts", r#"
        import { Injectable } from '@nestjs/common';
        import { PrismaClient } from '@prisma/client';

        @Injectable()
        export class OrdersService {
            prisma = new PrismaClient();
            findMany() { return this.prisma.order.findMany(); }
        }
    "#);

    workspace.create_file("apps/api/src/orders/orders.controller.ts", r#"
        import { Controller, Get } from '@nestjs/common';
        import { OrdersService } from './orders.service';

        @Controller('orders')
        export class OrdersController {
            constructor(private service: OrdersService) {}
            
            @Get()
            getOrders() { return this.service.findMany(); }
        }
    "#);

    // [FRONTEND]
    workspace.create_file("apps/web/src/app/orders/order.service.ts", r#"
        import { Injectable } from '@angular/core';
        import { HttpClient } from '@angular/common/http';
        import { BaseEntity } from '@shared/base.entity'; 

        @Injectable()
        export class OrderService {
            constructor(private http: HttpClient) {}
            list() { return this.http.get<BaseEntity[]>('/orders'); }
        }
    "#);

    workspace.create_file("apps/web/src/app/orders/order-list.component.ts", r#"
        import { Component } from '@angular/core';
        import { OrderService } from './order.service';

        @Component({
            selector: 'app-order-list',
            template: '<div>Orders</div>'
        })
        export class OrderListComponent {
            constructor(private api: OrderService) {}
            load() { this.api.list().subscribe(); }
        }
    "#);

    // ==========================================
    // 4. EXECUTION
    // ==========================================
    let mut indexer = Indexer::new();
    let mut pipeline = Pipeline::new();
    pipeline.run(&mut indexer, &workspace.path);

    // We query the Frontend Component for Users
    let related_ids = find_related_symbols(&indexer.index, &indexer.lookup, &indexer.reverse_graph, "UserProfileComponent", None)
        .expect("Should find UserProfileComponent");

    // ==========================================
    // 5. ANALYSIS & VALIDATION
    // ==========================================
    
    // Collect all unique File IDs and Symbol Names involved in the context
    let mut involved_file_paths = HashSet::new();
    let mut involved_symbol_names = HashSet::new();

    // Also look at file dependencies. 
    // If Symbol A is in File A, and File A depends on File B (e.g. HTML template), 
    // we want to ensure File B is conceptually "in context".
    // Note: 'find_related_symbols' returns Symbols. We map those to files.
    
    for &sid in &related_ids {
        let sym = indexer.index.symbols.get(&sid).unwrap();
        involved_symbol_names.insert(sym.name.clone());
        
        let fid = sym.file_id;
        if let Some(file_node) = indexer.index.files.values().find(|f| f.id == fid) {
            involved_file_paths.insert(file_node.path.clone());

            // Check if this file has dependencies (like HTML templates)
            if let Some(deps) = indexer.index.file_dependencies.get(&fid) {
                for &dep_fid in deps {
                    if let Some(dep_node) = indexer.index.files.values().find(|f| f.id == dep_fid) {
                        involved_file_paths.insert(dep_node.path.clone());
                    }
                }
            }
        }
    }

    // Helper to print what we found if assertions fail
    println!("--- CONTEXT SLICE RESULTS ---");
    for path in &involved_file_paths {
        println!("File: {}", path);
    }
    for name in &involved_symbol_names {
        println!("Symbol: {}", name);
    }
    println!("-----------------------------");

    // --- POSITIVE ASSERTIONS (What we MUST find) ---
    
    // 1. Frontend Logic
    assert!(involved_symbol_names.contains("UserProfileComponent"), "Missing Component TS");
    assert!(involved_symbol_names.contains("UserService"), "Missing Frontend Service");
    
    // 2. Frontend Template
    // The engine links TS -> HTML via literal dependency. 
    // We check if the HTML file path is in the involved files.
    let found_html = involved_file_paths.iter().any(|p| p.ends_with("user-profile.component.html"));
    assert!(found_html, "Missing Component HTML Template");

    // 3. Backend Logic (via Route '/users')
    assert!(involved_symbol_names.contains("UsersController"), "Missing Backend Controller (Route Link)");
    assert!(involved_symbol_names.contains("UsersService"), "Missing Backend Service (DI Link)");

    // 4. Database (via Prisma)
    assert!(involved_symbol_names.contains("User"), "Missing Prisma 'User' Model");

    // 5. Shared Code
    assert!(involved_symbol_names.contains("BaseEntity"), "Missing Shared Entity");


    // --- NEGATIVE ASSERTIONS (The "Orders" domain should NOT be here) ---

    // 1. Frontend Noise
    assert!(!involved_symbol_names.contains("OrderListComponent"), "Leaked Order Component");
    assert!(!involved_symbol_names.contains("OrderService"), "Leaked Order Frontend Service");

    // 2. Backend Noise
    assert!(!involved_symbol_names.contains("OrdersController"), "Leaked Order Controller");
    assert!(!involved_symbol_names.contains("OrdersService"), "Leaked Order Backend Service");

    // 3. Database Noise
    assert!(!involved_symbol_names.contains("Order"), "Leaked Prisma 'Order' Model");
}