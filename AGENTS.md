# AGENTS.md - Code Blast Radius Development Guide

This file contains build commands, code style guidelines, and development conventions for agentic coding agents working in this repository.

## Project Structure

This is a Rust workspace with three main crates:
- `engine/` - Core analysis and resolution engine
- `cli/` - Command-line interface (`cblast`)
- `app/src-tauri/` - Tauri-based GUI application

## Build/Test/Lint Commands

### Testing
```bash
# Run all tests across workspace
cargo test

# Run tests for specific crate
cargo test -p blast-radius-engine
cargo test -p cblast

# Run specific test file with output
cargo test --test definitions_state_test -- --no-capture
cargo test --test integration_tests -- --no-capture

# Run tests with specific filter
cargo test test_comprehensive_definitions_extraction
```

### Code Quality
```bash
# Format all code
cargo fmt

# Check formatting without applying changes
cargo fmt --check

# Lint with clippy
cargo clippy

# Clippy with all targets and features
cargo clippy --all-targets --all-features

# Standard cargo check
cargo check
```

### Building
```bash
# Build all workspace members
cargo build

# Build with optimizations
cargo build --release

# Build specific crate
cargo build -p blast-radius-engine
cargo build -p cblast
```

## Code Style Guidelines

### Imports and Dependencies
- Use `std::` imports for standard library items when possible
- Group imports: `std::` first, then external crates, then local modules
- Prefer specific imports over `use crate::*` except in test files
- External crate dependencies should be added to appropriate `Cargo.toml`

```rust
// Standard library
use std::collections::{HashMap, HashSet};
use std::path::Path;

// External crates
use anyhow::Result;
use serde::Deserialize;

// Local modules
use crate::analysis::language::SupportedLanguage;
use crate::models::SymbolId;
```

### Naming Conventions
- **Types**: `PascalCase` for structs, enums, type aliases
- **Functions**: `snake_case` for functions and methods
- **Constants**: `SCREAMING_SNAKE_CASE` for const items
- **Modules**: `snake_case` for module names and files
- **Fields**: `snake_case` for struct fields

```rust
pub struct WorkspaceIndex {
    pub symbols: HashMap<SymbolId, SymbolInfo>,
    pub files: HashMap<FileId, FileInfo>,
}

pub fn resolve_symbol_across_files(
    index: &WorkspaceIndex,
    target_file: FileId,
    symbol_name: &str,
) -> Option<SymbolId> {
    // Implementation
}
```

### Error Handling
- Use `anyhow::Result<T>` for application-level errors
- Use `thiserror` for custom error types in public APIs
- Prefer `?` operator for error propagation
- Avoid `unwrap()` except in test code

```rust
use anyhow::Result;

pub fn analyze_file(path: &Path) -> Result<FileAnalysis> {
    let content = std::fs::read_to_string(path)?;
    let analysis = parse_content(&content)?;
    Ok(analysis)
}
```

### Data Structures
- Use `rkyv` for serialization with `Archive`, `Deserialize`, `Serialize` derives
- Prefer `HashMap` for lookup tables, `HashSet` for uniqueness
- Use `Option<T>` for nullable fields
- Add `Debug`, `Clone` derives where appropriate

```rust
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: SymbolKind,
    pub file_id: FileId,
    pub range: Option<Range>,
}
```

### Function Design
- Keep functions focused and small (< 50 lines when possible)
- Use descriptive parameter names
- Return early for error conditions
- Add doc comments for public APIs

```rust
/// Resolves a symbol reference across the entire workspace.
/// 
/// # Arguments
/// * `index` - The workspace index containing all symbols
/// * `symbol_name` - The name of the symbol to resolve
/// 
/// # Returns
/// The symbol ID if found, None otherwise
pub fn resolve_workspace_symbol(
    index: &WorkspaceIndex,
    symbol_name: &str,
) -> Option<SymbolId> {
    // Implementation
}
```

### Testing Patterns
- Use `TestWorkspace` from `common.rs` for test setup
- Follow `test_case_name()` naming convention
- Use descriptive test names that explain what is being tested
- Include expected results in test case structs

```rust
#[test]
fn test_function_definition_extraction() {
    let test_case = DefTestCase {
        lang: SupportedLanguage::Rust,
        name: "simple_function",
        code: "fn hello() { println!(\"world\"); }",
        expected: vec![("hello", SymbolKind::Function)],
    };
    
    let result = analyze_source(test_case.code, test_case.lang);
    assert_expected_symbols(result, test_case.expected);
}
```

### Tree-sitter Integration
- Use tree-sitter query patterns in language configs
- Follow the query structure established in existing language modules
- Use `@function.name`, `@function.body`, `@function.definition` captures
- Include comments explaining query patterns

### Performance Considerations
- Use `rayon` for parallel processing of large datasets
- Implement caching for expensive operations
- Use `blake3` for fast file hashing
- Consider `memmap2` for large file processing

## Development Workflow

1. **Before implementing**: Run existing tests to ensure baseline
2. **During development**: Use `cargo check` frequently for fast feedback
3. **Before committing**: Run `cargo fmt`, `cargo clippy`, and `cargo test`
4. **For new features**: Add corresponding tests following existing patterns

## Language Support

The engine supports multiple languages via tree-sitter grammars. When adding new language support:

1. Add tree-sitter dependency to `engine/Cargo.toml`
2. Create language module in `engine/src/analysis/languages/`
3. Implement `config()` function following existing patterns
4. Add comprehensive tests in `engine/tests/`

## Test Organization

- Integration tests go in `engine/tests/`
- Unit tests go in respective source files
- Use `common.rs` for shared test utilities
- Test files should be named `*_test.rs` or `*_tests.rs`

## Dependencies

Key external dependencies:
- `tree-sitter` - Parsing framework
- `rkyv` - Zero-copy serialization
- `serde` - JSON serialization
- `anyhow`/`thiserror` - Error handling
- `rayon` - Parallel processing
- `blake3` - Fast hashing
- `clap` - CLI argument parsing