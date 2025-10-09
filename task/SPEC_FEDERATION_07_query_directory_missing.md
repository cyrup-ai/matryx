# SPEC_FEDERATION_07: Implement Query Directory Endpoint

## Status
**IMPLEMENTED BUT SHADOWED BY ROUTING CONFLICT**

The directory query functionality is **fully implemented** in [`packages/server/src/_matrix/federation/v1/query/by_query_type.rs`](../packages/server/src/_matrix/federation/v1/query/by_query_type.rs) but is shadowed by a stub route registered in [`packages/server/src/main.rs`](../packages/server/src/main.rs).

## Description
The directory query endpoint resolves room aliases to room IDs across federation. This is critical for the room join process when users attempt to join via a room alias (e.g., `#general:example.org`).

## Spec Requirements

Per [Matrix Federation API spec](https://spec.matrix.org/unstable/server-server-api/#get_matrixfederationv1querydirectory) and [`spec/server/09-room-joins.md`](../spec/server/09-room-joins.md):

### Endpoint
`GET /_matrix/federation/v1/query/directory`

### Purpose
- Resolve room alias to room ID
- Return list of resident servers for federation
- Used during room joins for server discovery (step 1 of join handshake)

### Query Parameters
- `room_alias`: The room alias to look up (required, format: `#localpart:domain`)

### Response Format (200)
```json
{
  "room_id": "!roomid:example.org",
  "servers": [
    "example.org",
    "example.com",
    "another.example.org"
  ]
}
```

### Error Response (404)
```json
{
  "errcode": "M_NOT_FOUND",
  "error": "Room alias not found"
}
```

## Current Implementation Analysis

### WORKING Implementation Location
**File**: [`packages/server/src/_matrix/federation/v1/query/by_query_type.rs`](../packages/server/src/_matrix/federation/v1/query/by_query_type.rs)  
**Lines**: 124-161

The `handle_directory_query` function provides complete implementation:

```rust
/// Handle directory queries (room alias resolution)
async fn handle_directory_query(
    state: &AppState,
    params: &HashMap<String, String>,
) -> Result<Json<Value>, StatusCode> {
    let room_alias = params.get("room_alias").ok_or_else(|| {
        warn!("Missing room_alias parameter for directory query");
        StatusCode::BAD_REQUEST
    })?;

    debug!("Directory query for room alias: {}", room_alias);

    // Validate room alias format
    if !room_alias.starts_with('#') || !room_alias.contains(':') {
        warn!("Invalid room alias format: {}", room_alias);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Query database for room alias
    let federation_repo = FederationRepository::new(state.db.clone());
    let alias_result = federation_repo.get_room_alias_info(room_alias).await.map_err(|e| {
        error!("Failed to query room alias {}: {}", room_alias, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match alias_result {
        Some((room_id, servers)) => {
            info!("Directory query successful for alias: {} -> {}", room_alias, room_id);
            Ok(Json(json!({
                "room_id": room_id,
                "servers": servers.unwrap_or_else(|| vec![state.homeserver_name.clone()])
            })))
        },
        None => {
            warn!("Room alias not found: {}", room_alias);
            Err(StatusCode::NOT_FOUND)
        },
    }
}
```

This implementation includes:
- ✅ X-Matrix authentication parsing and validation (lines 34-74)
- ✅ Server signature verification (lines 98-110)
- ✅ Query parameter extraction
- ✅ Room alias format validation (`#localpart:domain`)
- ✅ Database query via `FederationRepository.get_room_alias_info()`
- ✅ Proper response formatting with room_id and servers
- ✅ 404 handling for unknown aliases
- ✅ Comprehensive logging and error handling

### STUB Implementation Location
**File**: [`packages/server/src/_matrix/federation/v1/query/directory.rs`](../packages/server/src/_matrix/federation/v1/query/directory.rs)  
**Lines**: 1-8

```rust
use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/federation/v1/query/directory
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}
```

This is just an empty stub that returns `{}`.

### Routing Conflict
**File**: [`packages/server/src/main.rs`](../packages/server/src/main.rs)  
**Lines**: 568-569

```rust
.route("/v1/query/directory", get(_matrix::federation::v1::query::directory::get))
.route("/v1/query/{query_type}", get(_matrix::federation::v1::query::by_query_type::get))
```

**Problem**: The specific `/v1/query/directory` route is registered BEFORE the generic `/{query_type}` route. Axum matches routes in order, so requests to `/v1/query/directory` hit the stub instead of the working implementation.

### Database Layer
**File**: [`packages/surrealdb/src/repository/federation.rs`](../packages/surrealdb/src/repository/federation.rs)  
**Lines**: 1673-1697

The `FederationRepository::get_room_alias_info` method queries the `room_alias` table:

```rust
/// Get room alias information for federation directory query
pub async fn get_room_alias_info(
    &self,
    alias: &str,
) -> Result<Option<(String, Option<Vec<String>>)>, RepositoryError> {
    let query = "
        SELECT room_id, servers
        FROM room_alias
        WHERE alias = $alias
        LIMIT 1
    ";

    let mut response = self.db
        .query(query)
        .bind(("alias", alias.to_string()))
        .await?;

    #[derive(serde::Deserialize)]
    struct AliasResult {
        room_id: String,
        servers: Option<Vec<String>>,
    }

    let alias_result: Option<AliasResult> = response.take(0)?;
    Ok(alias_result.map(|result| (result.room_id, result.servers)))
}
```

### Database Schema
**File**: [`packages/surrealdb/src/repository/directory.rs`](../packages/surrealdb/src/repository/directory.rs)  
**Lines**: 156-174

The `room_alias` table structure:
```rust
pub async fn create_room_alias(
    &self,
    alias: &str,
    room_id: &str,
    creator_id: &str,
) -> Result<(), RepositoryError> {
    let alias_data = serde_json::json!({
        "alias": alias,           // Primary key
        "room_id": room_id,       // Target room
        "creator_id": creator_id, // Who created the alias
        "created_at": Utc::now(), // Creation timestamp
        "servers": [              // List of servers where room is resident
            "localhost"
        ]
    });

    let _created: Option<serde_json::Value> =
        self.db.create(("room_alias", alias)).content(alias_data).await?;

    Ok(())
}
```

## What Needs to Change

You have **two solution options**:

### Option 1: Remove Stub Route (RECOMMENDED - Simplest)

**Action**: Delete or comment out the specific `/v1/query/directory` route in `main.rs`

**File**: [`packages/server/src/main.rs`](../packages/server/src/main.rs)  
**Line**: 568

**Change**:
```rust
// Before (lines 568-569):
.route("/v1/query/directory", get(_matrix::federation::v1::query::directory::get))
.route("/v1/query/{query_type}", get(_matrix::federation::v1::query::by_query_type::get))

// After (remove line 568):
.route("/v1/query/{query_type}", get(_matrix::federation::v1::query::by_query_type::get))
```

**Rationale**: The generic `/{query_type}` handler already supports `directory` queries along with `profile` and `client_versions`. There's no need for a separate route.

**Optional Cleanup**: Delete [`packages/server/src/_matrix/federation/v1/query/directory.rs`](../packages/server/src/_matrix/federation/v1/query/directory.rs) since it's no longer used.

### Option 2: Move Implementation to directory.rs

**Action**: Copy the implementation from `by_query_type.rs` to `directory.rs`

**File**: [`packages/server/src/_matrix/federation/v1/query/directory.rs`](../packages/server/src/_matrix/federation/v1/query/directory.rs)

**Replace stub with**:
```rust
use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
};
use matryx_surrealdb::FederationRepository;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

use crate::state::AppState;

/// Query parameters for directory lookup
#[derive(Debug, Deserialize)]
pub struct DirectoryQueryParams {
    room_alias: String,
}

/// GET /_matrix/federation/v1/query/directory
/// 
/// Resolves a room alias to a room ID and list of resident servers.
/// This is used during the room join process for server discovery.
///
/// Reference: spec/server/09-room-joins.md (Directory Resolution - Step 1)
pub async fn get(
    State(state): State<AppState>,
    Query(params): Query<DirectoryQueryParams>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    let room_alias = &params.room_alias;
    
    debug!("Directory query for room alias: {}", room_alias);

    // Parse and validate X-Matrix authentication
    // (You would need to extract parse_x_matrix_auth from by_query_type.rs)
    let x_matrix_auth = parse_x_matrix_auth(&headers).map_err(|e| {
        warn!("Failed to parse X-Matrix authentication: {}", e);
        e
    })?;

    // Validate server signature
    let _validation = state
        .session_service
        .validate_server_signature(
            &x_matrix_auth.origin,
            &x_matrix_auth.key_id,
            &x_matrix_auth.signature,
            "GET",
            "/_matrix/federation/v1/query/directory",
            &[],
        )
        .await
        .map_err(|e| {
            warn!("Server signature validation failed: {:?}", e);
            StatusCode::UNAUTHORIZED
        })?;

    // Validate room alias format
    if !room_alias.starts_with('#') || !room_alias.contains(':') {
        warn!("Invalid room alias format: {}", room_alias);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Query database for room alias
    let federation_repo = FederationRepository::new(state.db.clone());
    let alias_result = federation_repo
        .get_room_alias_info(room_alias)
        .await
        .map_err(|e| {
            error!("Failed to query room alias {}: {}", room_alias, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match alias_result {
        Some((room_id, servers)) => {
            info!("Directory query successful: {} -> {}", room_alias, room_id);
            Ok(Json(json!({
                "room_id": room_id,
                "servers": servers.unwrap_or_else(|| vec![state.homeserver_name.clone()])
            })))
        },
        None => {
            warn!("Room alias not found: {}", room_alias);
            Err(StatusCode::NOT_FOUND)
        },
    }
}

// Helper function for X-Matrix authentication (copy from by_query_type.rs)
#[derive(Debug, Clone)]
struct XMatrixAuth {
    origin: String,
    key_id: String,
    signature: String,
}

fn parse_x_matrix_auth(headers: &HeaderMap) -> Result<XMatrixAuth, StatusCode> {
    // ... (copy implementation from by_query_type.rs lines 34-74)
}
```

**Rationale**: Keeps each query type in its own file for better organization. However, this duplicates the X-Matrix auth parsing logic.

## AppState Access Patterns

The handler needs these fields from `AppState` (see [`packages/server/src/state.rs`](../packages/server/src/state.rs)):

- `state.db` - SurrealDB connection for database queries
- `state.session_service` - For server signature validation
- `state.homeserver_name` - Default server name for response

Example from existing code:
```rust
State(state): State<AppState>
```

## Implementation Patterns

### X-Matrix Authentication
Already implemented in [`by_query_type.rs`](../packages/server/src/_matrix/federation/v1/query/by_query_type.rs) lines 34-74:
- Parses `Authorization: X-Matrix origin=...,key=...,sig=...` header
- Extracts origin, key ID, and signature
- Returns structured `XMatrixAuth` object

### Server Signature Validation
Already implemented in [`by_query_type.rs`](../packages/server/src/_matrix/federation/v1/query/by_query_type.rs) lines 98-110:
```rust
state.session_service.validate_server_signature(
    &origin,
    &key_id,
    &signature,
    "GET",
    &request_path,
    &[],
).await
```

### Query Parameter Extraction
Use Axum's `Query` extractor with a struct:
```rust
#[derive(Debug, Deserialize)]
pub struct QueryParams {
    room_alias: String,
}

Query(params): Query<QueryParams>
```

## Definition of Done

The endpoint is considered complete when:

1. **Routing Fixed**: Either stub route removed OR implementation moved to directory.rs
2. **Endpoint Responds**: `GET /_matrix/federation/v1/query/directory?room_alias=#room:server` returns valid JSON
3. **Authentication Works**: X-Matrix auth header is parsed and validated
4. **Alias Resolution Works**: Room aliases are resolved to room IDs from database
5. **Servers List Returned**: Response includes array of resident servers
6. **404 for Unknown**: Returns proper Matrix error for non-existent aliases
7. **Format Validation**: Rejects invalid alias formats (missing # or :)
8. **Compilation Success**: Code compiles without errors

## Files Involved

### Must Change
- [`packages/server/src/main.rs`](../packages/server/src/main.rs) - Route registration (line 568)

### Optional Change (if choosing Option 2)
- [`packages/server/src/_matrix/federation/v1/query/directory.rs`](../packages/server/src/_matrix/federation/v1/query/directory.rs) - Stub to full implementation

### Reference (Working Code)
- [`packages/server/src/_matrix/federation/v1/query/by_query_type.rs`](../packages/server/src/_matrix/federation/v1/query/by_query_type.rs) - Complete working implementation
- [`packages/surrealdb/src/repository/federation.rs`](../packages/surrealdb/src/repository/federation.rs) - Database layer (lines 1673-1697)
- [`packages/surrealdb/src/repository/directory.rs`](../packages/surrealdb/src/repository/directory.rs) - Room alias CRUD operations (lines 156-287)
- [`packages/server/src/state.rs`](../packages/server/src/state.rs) - AppState structure

### Specification Reference
- [`spec/server/09-room-joins.md`](../spec/server/09-room-joins.md) - Room join flow with directory resolution
- [`spec/server/25-querying-information.md`](../spec/server/25-querying-information.md) - Query endpoint overview

## Priority
**HIGH** - Critical for room joins via alias in federation scenarios

## Recommended Approach

**Use Option 1** (remove stub route). This is the cleanest solution because:

1. The generic handler already supports directory queries
2. Maintains consistency with how `profile` queries are handled
3. No code duplication
4. Minimal change (1 line deletion)
5. The implementation is production-ready and feature-complete

Simply delete line 568 in [`packages/server/src/main.rs`](../packages/server/src/main.rs) and the endpoint will work immediately.
