# SPEC_FEDERATION_07: Fix Query Directory Routing Conflict

## Status
**BLOCKED - ROUTING CONFLICT**

The directory query endpoint is fully implemented in `packages/server/src/_matrix/federation/v1/query/by_query_type.rs` but is **inaccessible** due to a stub route registered before the working handler.

## Issue

**File**: `packages/server/src/main.rs`  
**Line**: 568

The stub route is registered BEFORE the generic handler, causing Axum to route all `/v1/query/directory` requests to the stub instead of the working implementation:

```rust
// Line 568 - REMOVE THIS LINE
.route("/v1/query/directory", get(_matrix::federation::v1::query::directory::get))
// Line 569 - This works but is shadowed by line 568
.route("/v1/query/{query_type}", get(_matrix::federation::v1::query::by_query_type::get))
```

The stub returns `{}` while the working implementation provides complete functionality:
- X-Matrix authentication and signature validation
- Room alias format validation (`#localpart:domain`)
- Database query via `FederationRepository.get_room_alias_info()`
- Proper JSON response with `room_id` and `servers` array
- 404 handling for unknown aliases

## Required Fix

### Step 1: Remove Stub Route
Delete line 568 in `packages/server/src/main.rs`:

```rust
// DELETE THIS:
.route("/v1/query/directory", get(_matrix::federation::v1::query::directory::get))
```

The generic `/{query_type}` handler on line 569 already supports directory queries.

### Step 2: Optional Cleanup
Delete the unused stub file:
```bash
rm packages/server/src/_matrix/federation/v1/query/directory.rs
```

Update `packages/server/src/_matrix/federation/v1/query/mod.rs` to remove the directory module export.

## Definition of Done

1. ✅ Stub route removed from main.rs (line 568 deleted)
2. ✅ Endpoint accessible via generic handler
3. ✅ `GET /_matrix/federation/v1/query/directory?room_alias=#room:server` returns valid JSON with room_id and servers
4. ✅ 404 returned for unknown aliases
5. ✅ Optional: Stub file deleted and mod.rs updated

## Priority
**CRITICAL** - 1-line fix to enable production functionality
