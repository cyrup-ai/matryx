# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

MaxTryX is a Matrix homeserver implementation written in Rust, using SurrealDB as the database backend. The project implements the Matrix Federation API and Client-Server API to provide a full Matrix homeserver experience.

## Architecture

### Package Structure

This is a Rust workspace with 4 main packages:

- **`packages/entity`** - Pure Matrix protocol entity types and data structures (no dependencies on specific implementations)
- **`packages/surrealdb`** - SurrealDB Data Access Object layer with LIVE queries support
- **`packages/server`** - Matrix homeserver HTTP API implementation using Axum
- **`packages/client`** - Matrix client library for interacting with Matrix homeservers

### Key Components

- **AppState** (`packages/server/src/state.rs`): Core application state containing:
  - `db`: SurrealDB connection (`Surreal<Any>`)
  - `session_service`: Authentication and session management
  - `homeserver_name`: Server identity

- **Authentication**: JWT-based authentication with Matrix-compatible user sessions
- **Federation**: Matrix server-to-server federation protocol implementation
- **Database Layer**: SurrealDB with structured repositories for each entity type

## Development Commands

### Building
```bash
cargo build                    # Build all packages
cargo build -p matryx_server  # Build specific package
```

### Testing
```bash
cargo test                     # Run all tests
cargo test -p matryx_server    # Run tests for specific package
cargo nextest run             # Use nextest if available (faster)
```

### Running the Server
```bash
cargo run --bin matryxd       # Run the homeserver daemon
```

### Environment Variables
- `DATABASE_URL` - SurrealDB connection string (defaults to "memory")

### Linting and Formatting
```bash
cargo clippy                   # Run clippy lints
cargo fmt                     # Format code
```

## Matrix API Implementation

The server implements Matrix APIs under `packages/server/src/_matrix/`:

- **Client APIs** (`_matrix/client/`): User-facing REST APIs (v1, v3)
- **Federation APIs** (`_matrix/federation/`): Server-to-server communication (v1, v2)
- **Media APIs** (`_matrix/media/`): File upload/download (v1, v3)
- **Well-known APIs** (`_matrix/_well_known/`): Server discovery

### API Structure Pattern
- Each endpoint is organized by version and resource hierarchy
- Handlers follow Axum patterns with `AppState` injection
- Authentication via middleware for protected endpoints

## Database Integration

### SurrealDB Usage
- Custom SurrealDB fork at `forks/surrealdb/` with enhanced features
- Repository pattern in `packages/surrealdb/src/repository/`
- LIVE queries for real-time updates
- Schema migrations in `packages/surrealdb/migrations/`

### Repository Pattern
Each Matrix entity has a dedicated repository:
- `UserRepository` - User account management
- `RoomRepository` - Room state and events
- `EventRepository` - Matrix events storage
- `SessionRepository` - Authentication sessions
- And more in `packages/surrealdb/src/repository/`

## Key Dependencies

- **Axum 0.8** - HTTP server framework
- **SurrealDB 3.0** - Database (custom fork)
- **Tokio** - Async runtime
- **Serde** - Serialization
- **UUID** - Identifier generation
- **chrono** - Time handling
- **ed25519-dalek** - Cryptographic signing

## Known Issues

The TODO.md file indicates there are currently 252 compilation errors that need systematic resolution. The main categories of issues are:

1. **AppState field access** - Code expects different field names than actual AppState structure
2. **Authentication API changes** - Function signature mismatches
3. **SurrealDB API evolution** - Response type method changes
4. **Import resolution** - Missing database module imports
5. **Lifetime management** - Borrowed data escaping function scope

## Development Guidelines

### Adding New Matrix APIs
1. Create endpoint handler in appropriate `_matrix/` subdirectory
2. Implement authentication middleware if required
3. Add database operations via repository pattern
4. Follow Matrix specification for request/response formats

### Database Operations
- Always use the repository pattern rather than direct DB calls
- Implement proper error handling with custom error types
- Consider using LIVE queries for real-time features

### Testing Strategy
- Unit tests for individual components
- Integration tests for API endpoints
- Use `mockall` for mocking dependencies
- Test database operations with temporary instances