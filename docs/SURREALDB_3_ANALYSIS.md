# SurrealDB 3.0 Architecture Analysis for Matrix Integration

## Authentication System

### Core Components

**Auth Structure** (`/forks/surrealdb/crates/core/src/iam/auth.rs`):
- `Auth` contains an `Actor` with roles and authentication level
- Supports hierarchical levels: `No`, `Root`, `Namespace(ns)`, `Database(ns, db)`, `Record(ns, db, ac)`
- Built-in permission checking with `is_allowed()` method
- Role-based access control with `Owner`, `Editor`, `Viewer` roles

**Actor Structure** (`/forks/surrealdb/crates/core/src/iam/entities/resources/actor.rs`):
- Contains `Resource` (id, kind, level) and `Vec<Role>`
- Hierarchical role checking: Owner > Editor > Viewer
- Integrates with `DefineUserStatement` and `DefineAccessStatement`

**Session Management** (`/forks/surrealdb/crates/core/src/dbs/session.rs`):
- `Session` struct contains `Auth`, connection metadata, namespace/database selection
- Session expiration support with `expired()` method
- Real-time query support flag (`rt: bool`)
- Token and record authentication data storage (`tk`, `rd`)
- Built-in session value extraction for context variables

### Token System

**JWT Integration** (`/forks/surrealdb/crates/core/src/iam/token.rs`):
- Standard JWT claims with SurrealDB extensions
- Custom claims support with `custom_claims: HashMap<String, serde_json::Value>`
- Namespace (`NS`), Database (`DB`), Access (`AC`), Record (`ID`), Roles (`RL`) claims
- Automatic conversion to SurrealDB `Object` format

## LiveQuery System

### Core Architecture

**LiveStatement Structure** (`/forks/surrealdb/crates/core/src/expr/statements/live.rs`):
- Stores authentication context: `auth: Option<Auth>` and `session: Option<Value>`
- Node-based distribution with `node: Uuid` for cluster support
- Query registration in both node storage (`/node/{node_id}/lq/{query_id}`) and table storage (`/table/{ns}/{db}/{table}/lq/{query_id}`)
- Cache invalidation support for live query updates

**Live Query Processing** (`/forks/surrealdb/crates/core/src/doc/lives.rs`):
- **Authentication Context Preservation**: Each live query maintains the original user's auth and session
- **Permission Enforcement**: Live queries check permissions using the original user's context
- **Real-time Notifications**: Supports CREATE, UPDATE, DELETE actions with before/after document states
- **Session Variable Injection**: Provides `$access`, `$auth`, `$token`, `$session`, `$event`, `$value`, `$before`, `$after`

### Distributed Features

**Node Management** (`/forks/surrealdb/crates/core/src/dbs/node.rs`):
- Cluster node tracking with heartbeat timestamps
- Node archival and garbage collection
- UUID-based node identification for distributed live queries

## Key Integration Points for Matrix

### 1. Authentication Mapping
- **Matrix Access Tokens** → SurrealDB JWT tokens with custom claims
- **Matrix User IDs** → SurrealDB Record-level authentication
- **Matrix Device IDs** → JWT custom claims or session metadata
- **Federation Signatures** → SurrealDB namespace/database level authentication

### 2. Real-time Event Distribution
- **Matrix Room Events** → SurrealDB LiveQuery on room tables
- **Presence Updates** → LiveQuery on user presence tables
- **Typing Indicators** → LiveQuery on ephemeral event tables
- **Read Receipts** → LiveQuery on receipt tables

### 3. Permission Integration
- **Matrix Room Permissions** → SurrealDB table-level permissions
- **Power Levels** → SurrealDB role hierarchy (Owner/Editor/Viewer mapping)
- **Federation ACLs** → SurrealDB namespace-level permissions

### 4. Session Management
- **Matrix Sessions** → SurrealDB Session with Matrix-specific metadata
- **Device Management** → Session tracking with device_id in custom claims
- **Token Expiration** → SurrealDB session expiration handling
- **Refresh Tokens** → Custom JWT claims for refresh token storage

## Technical Advantages

1. **Native Authentication Context**: LiveQueries automatically preserve and enforce original user permissions
2. **Distributed Architecture**: Node-based live query distribution supports Matrix federation
3. **Flexible Permission Model**: Hierarchical levels map well to Matrix room/server permissions
4. **Real-time Performance**: Built-in cache invalidation and efficient notification delivery
5. **JWT Integration**: Standard token format with extensible custom claims for Matrix metadata
6. **Session Persistence**: Comprehensive session management with expiration and metadata support

## Implementation Strategy

The SurrealDB 3.0 architecture provides native support for all Matrix requirements:
- Authentication context preservation in live queries
- Distributed real-time event processing
- Hierarchical permission enforcement
- JWT-based token management with custom claims
- Session lifecycle management with expiration

This eliminates the need for external authentication middleware or custom real-time event systems.