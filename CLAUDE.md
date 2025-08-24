# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

MaxTryX is a Matrix chat client with SurrealDB local storage, built in Rust. It consists of:
- A TUI (Terminal User Interface) application using `ratatui`
- A Matrix SDK wrapper API (`matryx_api`) providing synchronous interfaces
- SurrealDB for local storage with migration support
- Vim-like keybindings and modal interface design

## Architecture

### Package Structure
- `packages/api/` - Matrix SDK wrapper (`matryx_api`) with synchronous interfaces hiding async complexity
- `packages/tui/` - Terminal UI application (`maxtryx` binary) with widgets, modals, and window management
- `packages/api/vendor/matrix-rust-sdk/` - Vendored Matrix Rust SDK
- Root crate contains shared configuration and dependencies

### Key Design Principles
- **Hidden Async Complexity**: The API layer provides synchronous interfaces while managing async operations internally using channels and task spawning
- **SurrealDB Storage**: All persistent data uses SurrealDB with proper migrations
- **Modal UI**: TUI follows vim-like modal patterns with sophisticated keybinding system
- **Widget-based Architecture**: Modular UI components (dialogs, tabs, layouts, text editors)

### Core Modules
- `packages/api/src/db/` - Database layer with migrations, DAOs, and entities
- `packages/tui/src/widgets/` - UI components (window, dialog, tabs, layout, text editor)
- `packages/tui/src/modal/` - Modal system for input handling and keybindings
- `packages/tui/src/windows/` - Application windows (room chat, welcome screen)
- `packages/tui/src/message/` - Message rendering and composition

## Common Development Commands

### Building & Running
```bash
# Build all packages
cargo build
just build

# Run the main TUI application
cargo run -p tui
just run tui

# Build in release mode
cargo build --release
just release
```

### Testing
```bash
# Run all tests with nextest (preferred)
cargo nextest run
just test

# Run integration tests only
cargo nextest run --test "integration"
just test-integration

# Run matrix-specific database tests
just test-matrix-db

# Run matrix tests with tracing enabled
just test-matrix-trace
```

### Linting & Formatting
```bash
# Format code
cargo fmt
just fmt

# Check formatting and run clippy
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
just lint

# Quick check after changes (run after EVERY change)
cargo fmt && cargo check --message-format short --quiet
```

### Database Management
```bash
# Run migrations for all databases
just migrate

# Specific database operations
cd packages/api && just migrate
```

## Development Conventions

### Async Interface Design
- ❌ NEVER use `async_trait` or `async fn` in traits
- ❌ NEVER return `Box<dyn Future>` or `Pin<Box<dyn Future>>` from client interfaces
- ✅ Provide synchronous interfaces with `.await()` called internally
- ✅ Hide async complexity behind `channel` and `task` `spawn`
- ✅ Return intuitive, domain-specific types (e.g., `MatrixFuture<T>`)

### Dependency Management
- Always use `cargo add/remove` commands, never edit Cargo.toml directly
- Use latest versions unless explicitly documented otherwise
- Prefer `cargo search <package> --limit 1` to find latest versions

### Code Quality Standards
- All code MUST pass `cargo check --message-format short --quiet -- -D warnings`
- No file should exceed 300 lines - decompose into elegant modules
- Use `tracing` for logging with appropriate levels
- Tests belong in `tests/` directory only
- Use `nextest` for all test execution
- No suppression of compiler or clippy warnings

### SurrealDB Usage
- Use SurrealDB 2.3+ syntax (significant changes from earlier versions)
- Use `kv-surrealkv` for local file databases, `kv-tikv` for distributed
- Apply appropriate table types: document, relational, graph, time series, vector
- Always use `surrealdb-migrations` 2.2+ for versioned migrations
- Follow patterns in `packages/api/src/db/` for DAOs and entities

### Matrix SDK Integration
- The project uses Matrix SDK 0.13 with custom wrapper in `packages/api/`
- Several modules are disabled (`#[allow(dead_code)]`) pending migration completion
- Focus on `packages/api/src/db/` and `packages/api/src/commands/` for active development
- The wrapper provides synchronous interfaces hiding async complexity from TUI layer

### Error Handling
- Use `Result<T,E>` with custom error types
- No `unwrap()` except in tests
- Handle all Result/Option values explicitly
- Use `anyhow` for application errors, `thiserror` for library errors

### Code Organization
- Follow Rust naming conventions: `snake_case` for variables/functions
- Each binary crate should have exactly one binary target
- Focus on minimal, correct implementations over feature creep
- Test like an end-user: run `cargo run -p tui` to verify functionality

### Development Workflow
1. Make changes
2. Run `cargo fmt && cargo check --message-format short --quiet`
3. Run appropriate tests with `cargo nextest run`
4. Verify functionality with actual binary execution
5. Ensure all warnings are resolved (not suppressed)

## Key Technologies
- **UI Framework**: `ratatui` for terminal interface
- **Async Runtime**: `tokio` with full features
- **Database**: `surrealdb` with `kv-surrealkv` storage
- **Matrix**: `matrix-sdk` 0.13 with encryption support
- **CLI**: `clap` for argument parsing
- **Testing**: `nextest` for fast test execution
- **Cross-terminal**: `crossterm` for terminal abstraction
- **Build**: `just` for task automation

## Notes
- The project is undergoing migration from an older Matrix SDK version
- Some modules in the API package are temporarily disabled during this transition
- Database layer is actively developed with comprehensive DAO patterns
- TUI implements sophisticated modal editing patterns similar to vim
- Configuration uses TOML with support for layouts, keybindings, and user customization