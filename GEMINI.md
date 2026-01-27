# Code Blast Radius - Project Context

The main purpose of code blast radius is to provide context for LLM agents, but also allow humans
to manage context by hand for large codebases. Both users and agents can create and manage Recipes. 
Recipes should be able to persist in a moving codebase for quite some time. Importantly, because this is LLM context managment tool, we focus on Recall (we want to find all relevant code files), not Precision
(we can have additional context).

This is an ambicious project where we want to support veyr fast searches of context across large codebases, mixed languages, multiple folder roots, mutiple frameworks and at the same time to support this on 3 platforms (windows, mac and linux).

## Project Architecture

The project is structured as a Rust workspace with three main components:

- **`engine/` (`blast-radius-engine`):** The core logic library.
  - **Analysis:** Uses `tree-sitter` to parse multiple languages and extract symbols/dependencies.
  - **Incremental Sync:** Uses `blake3` hashing to track file changes and update a persistent index.
  - **Query Engine:** Implements "walkers" to traverse the dependency graph (upwards for impact, downwards for dependencies).
  - **Recipes:** A system for defining and executing complex context extraction logic (XML/JSON output).
- **`cli/` (`cblast`):** A command-line interface for interacting with the engine.
  - Commands: `radius`, `find`, `workspace`, `recipe`.
- **`app/` (`code-blast-radius-gui`):** A Tauri-based desktop application.
  - **Frontend:** SolidJS, TypeScript, Tailwind CSS, Vite.
  - **Features:** Visual graph explorer, recipe builder, search, and workspace management.

## Supported Languages

The engine supports a wide array of languages via tree-sitter:
Rust, TypeScript, JavaScript, Python, Bash, Java, HTML, Julia, R, JSON, GDScript, Dart, YAML, TOML, SQL, Prisma, HCL, Go, C#, PHP, Ruby, C, and C++.

We also strive to support all the main frameworks.

## Development Workflow

### Building and Running

- **Full Workspace:**
  - Build: `cargo build`
  - Test: `cargo test` (All tests should pass).
- **CLI Tool:**
  - Run: `cargo run -p cblast -- --path <dir_or_cblast_file> <command>`
  - Example: `cargo run -p cblast -- --path . find --query MySymbol`
- **GUI App:**
  - Development: `cd app && npm run tauri dev`
  - Build: `cd app && npm run tauri build`

### Testing Conventions

- Run all tests: `cargo test`
- Run specific test file: `cargo test --test <test_name> -- --no-capture`
- Tests are located in `engine/tests/` and cover various aspects like definitions, impact analysis, and persistence.

### Key Files

- `Cargo.toml`: Workspace configuration and dependencies.
- `engine/src/lib.rs`: Entry point for the core engine logic.
- `cli/src/main.rs`: Entry point for the CLI tool.
- `app/src-tauri/tauri.conf.json`: Tauri configuration for the GUI.
- `app/package.json`: Frontend dependencies and scripts.

## Persistence

The tool generates a `.cblast` directory/file to store its index and workspace configuration. This allows for fast incremental updates on subsequent runs.
