# PAGEFIX_1: Implement User Permission Validation for Room Message Pagination

**Status**: Ready for Implementation  
**Priority**: HIGH  
**Estimated Effort**: 1-2 days  
**Package**: packages/server

---

## OBJECTIVE

Implement user permission validation for the Matrix `/rooms/{roomId}/messages` endpoint to prevent unauthorized access to room message history. Additionally, add handler-level pagination token validation and limit checking for better error messages and performance.

---

## CURRENT STATE ANALYSIS

### What Already Exists

**Repository Layer** (`packages/surrealdb/src/repository/room.rs:2778-2946`)

The repository already implements complete pagination token handling:

1. **Token Format**: `t{timestamp}_{event_id}` (e.g., "t1704067200000_$event123:homeserver.com")

2. **Existing Functions**:
   ```rust
   // Line 2928-2946: Validates token format and extracts timestamp
   fn parse_pagination_token(token: &str) -> Result<Option<i64>, RepositoryError>
   
   // Line 2948-2951: Generates tokens for responses  
   fn generate_pagination_token(timestamp: i64, event_id: &str) -> String
   
   // Line 2778-2926: Main query function with token support
   pub async fn get_room_messages_paginated(
       &self,
       room_id: &str,
       from_token: Option<&str>,  // Already validates internally
       to_token: Option<&str>,    // Already validates internally
       direction: &str,
       limit: u32,
       filter: Option<&RoomEventFilter>,
   ) -> Result<(Vec<Event>, String, String), RepositoryError>
   ```

3. **Token Validation**: Returns `RepositoryError::Validation` for malformed tokens

**Handler Layer** (`packages/server/src/_matrix/client/v3/rooms/by_room_id/messages.rs`)

Current implementation (158 lines):
- Validates authentication (lines 60-87)
- Validates room_id format (lines 95-98)
- Validates direction parameter (lines 101-104)
- Parses optional filter (lines 107-116)
- **LINE 113-114: TODO about user access validation** ⚠️
- Passes tokens directly to repository without validation
- No limit bounds checking
- Uses StatusCode for error responses

### What's Missing

1. **User Permission Check** (the actual TODO at line 113):
   - No verification that user is a member of the room
   - Matrix spec requires 403 error if user not in room
   
2. **Handler-Level Token Validation** (optional optimization):
   - Tokens only validated at repository layer
   - Invalid tokens hit database before rejection
   - Could provide better error messages earlier

3. **Limit Validation**:
   - No maximum limit enforcement
   - Matrix spec default is 10, but no server-side max

4. **Error Response Improvement**:
   - Using generic StatusCode instead of descriptive error bodies
   - Matrix spec expects structured error responses

---

## PROBLEM DESCRIPTION

The endpoint currently has a TODO marker at line 113-114:

```rust
// TODO: Validate user has access to room
// For now, we'll proceed with the query
```

This allows:
- Users to read messages from rooms they're not members of (security issue)
- Potential data leak from private rooms
- Violation of Matrix specification (should return 403)

Secondary issues:
- Invalid pagination tokens only caught at database layer (performance)
- No limit bounds checking (potential resource exhaustion)
- Poor error messages for client debugging

---

## MATRIX SPECIFICATION REQUIREMENTS

**Source**: [Matrix Client-Server API - Message Pagination](../../tmp/matrix-spec/data/api/client-server/message_pagination.yaml)

**Endpoint**: `GET /_matrix/client/v3/rooms/{roomId}/messages`

**Required Validations**:
1. User must be authenticated (✅ already implemented)
2. User must be member of room (❌ missing - this task)
3. `dir` parameter required, must be "b" or "f" (✅ already implemented)
4. `from` parameter optional (✅ correctly handled)
5. Must return 403 if user not in room (❌ missing)

**Token Requirements** (from spec):
- Tokens are opaque, server-defined strings
- Can come from `/sync` endpoint or previous `/messages` calls  
- Our format `t{timestamp}_{event_id}` is compliant
- Invalid tokens should return clear error messages

**Limit Parameter**:
- Default: 10 (not enforced in our code)
- No maximum specified in spec (recommend 100 for server protection)

---

## IMPLEMENTATION TASKS

### TASK 1: Implement User Permission Validation (Primary Goal)

**Location**: `packages/server/src/_matrix/client/v3/rooms/by_room_id/messages.rs:113-114`

**Replace the TODO with actual validation**:

```rust
// Validate user has access to room
let is_member = state
    .room_operations
    .room_repo()
    .is_user_in_room(&user_id, &room_id)
    .await
    .map_err(|e| {
        error!("Failed to check room membership: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

if !is_member {
    warn!(
        "User {} attempted to access messages in room {} without membership",
        user_id, room_id
    );
    return Err(StatusCode::FORBIDDEN);
}
```

**Required Repository Method**:

Check if `is_user_in_room()` exists in `packages/surrealdb/src/repository/room.rs`. If not, implement:

```rust
pub async fn is_user_in_room(
    &self,
    user_id: &str,
    room_id: &str,
) -> Result<bool, RepositoryError> {
    let query = "
        SELECT VALUE count() > 0
        FROM room_membership
        WHERE user_id = $user_id
          AND room_id = $room_id
          AND membership IN ['join', 'invite']
        LIMIT 1
    ";
    
    let mut response = self.db
        .query(query)
        .bind(("user_id", user_id))
        .bind(("room_id", room_id))
        .await?;
    
    let is_member: Option<bool> = response.take(0)?;
    Ok(is_member.unwrap_or(false))
}
```

**Files to Modify**:
- `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v3/rooms/by_room_id/messages.rs` (lines 113-114)
- `/Volumes/samsung_t9/maxtryx/packages/surrealdb/src/repository/room.rs` (add method if missing)

---

### TASK 2: Add Handler-Level Token Validation (Secondary - Optional Optimization)

**Location**: `packages/server/src/_matrix/client/v3/rooms/by_room_id/messages.rs` (after line 116)

**Purpose**: Reject invalid tokens faster, before hitting database

**Implementation**:

Add this before the repository call (around line 118):

```rust
// Pre-validate pagination tokens for faster rejection and better error messages
if let Some(from_str) = params.from.as_ref() {
    if !is_valid_pagination_token(from_str) {
        warn!("Invalid 'from' pagination token format: {}", from_str);
        return Err(StatusCode::BAD_REQUEST);
    }
}

if let Some(to_str) = params.to.as_ref() {
    if !is_valid_pagination_token(to_str) {
        warn!("Invalid 'to' pagination token format: {}", to_str);
        return Err(StatusCode::BAD_REQUEST);
    }
}
```

Add helper function at module level:

```rust
/// Validates pagination token format without parsing
/// Format: t{timestamp}_{event_id}
fn is_valid_pagination_token(token: &str) -> bool {
    if !token.starts_with('t') {
        return false;
    }
    
    let parts: Vec<&str> = token[1..].splitn(2, '_').collect();
    if parts.len() != 2 {
        return false;
    }
    
    // Check timestamp is numeric
    if parts[0].parse::<i64>().is_err() {
        return false;
    }
    
    // Check event_id starts with $
    if !parts[1].starts_with('$') {
        return false;
    }
    
    true
}
```

**Benefit**: Repository already validates, but this provides:
- Faster rejection (no database query)
- Better error logging at handler level
- Consistent with validation pattern in codebase

---

### TASK 3: Add Limit Validation

**Location**: `packages/server/src/_matrix/client/v3/rooms/by_room_id/messages.rs` (after token validation)

**Implementation**:

```rust
// Validate and enforce limit bounds
let limit = params.limit;
if limit == 0 {
    warn!("Invalid limit parameter: 0");
    return Err(StatusCode::BAD_REQUEST);
}
if limit > 100 {
    warn!("Limit {} exceeds maximum allowed (100)", limit);
    return Err(StatusCode::BAD_REQUEST);
}
```

**Rationale**: 
- Matrix spec has no max, but servers should protect resources
- Synapse uses 100 as default max
- Current code has no upper bound

---

### TASK 4: Improve Error Responses (Optional Enhancement)

**Current**: Uses StatusCode only  
**Better**: Return JSON error bodies per Matrix spec

Example error response structure (if time permits):

```rust
#[derive(Serialize)]
struct MatrixError {
    errcode: String,
    error: String,
}

// Use in responses:
return Err((
    StatusCode::FORBIDDEN,
    Json(MatrixError {
        errcode: "M_FORBIDDEN".to_string(),
        error: "You are not a member of this room".to_string(),
    })
));
```

---

## CODE REFERENCES

### Existing Repository Token Handling

**File**: `/Volumes/samsung_t9/maxtryx/packages/surrealdb/src/repository/room.rs`

**parse_pagination_token** (lines 2928-2946):
```rust
fn parse_pagination_token(token: &str) -> Result<Option<i64>, RepositoryError> {
    if !token.starts_with('t') {
        return Err(RepositoryError::Validation {
            field: "token".to_string(),
            message: format!("Invalid pagination token format: {}", token),
        });
    }

    let parts: Vec<&str> = token[1..].split('_').collect();
    if parts.is_empty() {
        return Err(RepositoryError::Validation {
            field: "token".to_string(),
            message: format!("Invalid pagination token format: {}", token),
        });
    }

    match parts[0].parse::<i64>() {
        Ok(ts) => Ok(Some(ts)),
        Err(_) => Err(RepositoryError::Validation {
            field: "token".to_string(),
            message: format!("Invalid timestamp in token: {}", token),
        }),
    }
}
```

**generate_pagination_token** (lines 2948-2951):
```rust
fn generate_pagination_token(timestamp: i64, event_id: &str) -> String {
    format!("t{}_{}", timestamp, event_id)
}
```

### Similar Permission Check Pattern

**File**: `/Volumes/samsung_t9/maxtryx/packages/server/src/threading.rs` (reference for membership checks)

Shows pattern of checking user permissions before operations.

---

## RESEARCH CITATIONS

1. **Matrix Specification - Message Pagination**  
   File: [./tmp/matrix-spec/data/api/client-server/message_pagination.yaml](../../tmp/matrix-spec/data/api/client-server/message_pagination.yaml)  
   Lines: 18-193 (complete endpoint specification)

2. **Repository Token Implementation**  
   File: [./packages/surrealdb/src/repository/room.rs](../../packages/surrealdb/src/repository/room.rs)  
   Lines: 2778-2951 (pagination implementation)

3. **Current Handler Implementation**  
   File: [./packages/server/src/_matrix/client/v3/rooms/by_room_id/messages.rs](../../packages/server/src/_matrix/client/v3/rooms/by_room_id/messages.rs)  
   Lines: 1-158 (complete handler)

---

## DEFINITION OF DONE

### Required (High Priority):
- [ ] TODO comment at line 113-114 removed
- [ ] User membership validation implemented before database query
- [ ] Returns 403 StatusCode when user not in room
- [ ] is_user_in_room() method exists and works correctly
- [ ] Appropriate logging added for security events

### Recommended (Medium Priority):
- [ ] Handler-level token validation added for early rejection
- [ ] Limit parameter validated with maximum of 100
- [ ] Clear warning logs for invalid parameters

### Optional (Low Priority):
- [ ] Structured error responses with Matrix error codes
- [ ] Improved error messages in response bodies

### Verification:
- [ ] No compilation errors
- [ ] Existing functionality preserved (backward compatible)
- [ ] Authentication still works as before
- [ ] Repository layer unchanged (unless adding is_user_in_room)

---

## CONSTRAINTS

- **NO TESTS**: Do not write unit tests, integration tests, or test fixtures
- **NO BENCHMARKS**: Do not write benchmark code or performance tests
- **NO DOCUMENTATION**: Do not create markdown docs, README files, or API documentation
- **FOCUS ON FUNCTIONALITY**: Only modify production code to implement the requirements

---

## FILES TO MODIFY

1. **Primary File**:  
   `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v3/rooms/by_room_id/messages.rs`
   - Lines 113-114: Replace TODO with user permission check
   - Lines 118+: Add token validation (optional)
   - Lines 118+: Add limit validation

2. **Secondary File** (if method missing):  
   `/Volumes/samsung_t9/maxtryx/packages/surrealdb/src/repository/room.rs`
   - Add is_user_in_room() method if it doesn't exist
   - Check existing methods first before adding

---

## IMPLEMENTATION NOTES

### Discovery: Repository Already Handles Tokens

The task initially assumed no token validation existed, but research revealed:
- Complete token validation in repository layer
- Proper error handling for malformed tokens
- Standard format: `t{timestamp}_{event_id}`

This means we DON'T need to:
- Create a new PaginationToken struct
- Implement token parsing from scratch
- Change repository signatures

### Why Add Handler-Level Validation?

Even though repository validates tokens, handler-level validation provides:
1. **Performance**: Reject invalid tokens before database query
2. **Better Logging**: Capture client errors at entry point
3. **Clearer Error Messages**: Return detailed errors immediately
4. **Consistency**: Match pattern of other validations in handler

### Error Handling Pattern

Current code uses StatusCode directly. For consistency:
- Continue using StatusCode for now
- Could enhance with JSON error bodies later
- See threading.rs for error handling patterns

### Database Query Optimization

The is_user_in_room query should use indexes on:
- (room_id, user_id, membership) for fastest lookup
- Consider creating index if query is slow

---

## ESTIMATED IMPLEMENTATION TIME

**Task 1 (User Permission)**: 2-3 hours
- Research if is_user_in_room exists: 30 min
- Implement if missing: 1 hour  
- Add permission check to handler: 30 min
- Testing manually: 1 hour

**Task 2 (Token Validation)**: 1-2 hours  
- Write validation helper: 30 min
- Add to handler: 30 min
- Test with various tokens: 30-60 min

**Task 3 (Limit Validation)**: 30 minutes
- Simple bounds check: 15 min
- Test edge cases: 15 min

**Total**: 1-2 days for complete implementation

---

## QUESTIONS TO RESOLVE DURING IMPLEMENTATION

1. Does `is_user_in_room()` already exist in RoomRepository?
   - If yes, use it directly
   - If no, implement as shown above

2. Should we check for 'invite' membership or only 'join'?
   - Matrix spec: invited users can read history (depending on settings)
   - Check room's history_visibility setting if needed

3. Should limit validation be strict or allow higher limits for server auth?
   - Recommendation: Apply limit to all requests for consistency

4. Should we add rate limiting for this endpoint?
   - Out of scope for this task, but consider for future

---

## SECURITY CONSIDERATIONS

1. **Authorization Before Database Access**  
   Always check user permissions before querying sensitive data

2. **Information Disclosure**  
   Different error messages for "room not found" vs "not a member" could leak room existence
   - Solution: Return same 403 for both cases

3. **Token Enumeration**  
   Malicious clients could try token enumeration to discover events
   - Mitigation: Rate limiting (separate task)
   - Our validation helps by rejecting obviously invalid tokens early

4. **Resource Exhaustion**  
   Large limit values could exhaust server resources
   - Mitigation: Task 3 adds max limit of 100

---

## INTEGRATION WITH EXISTING CODE

### Authentication Flow (Already Implemented)
Lines 60-87 in messages.rs:
1. Extract Matrix auth from headers
2. Validate token not expired  
3. Extract user_id
4. Reject if anonymous or server auth

### New Flow After This Task
1. Extract and validate auth ✅ (existing)
2. Validate room_id format ✅ (existing)
3. Validate direction parameter ✅ (existing)
4. **Check user in room** ⭐ (this task - primary)
5. **Validate tokens** ⭐ (this task - secondary)
6. **Validate limit** ⭐ (this task - tertiary)
7. Parse filter ✅ (existing)
8. Query database ✅ (existing)

### Repository Method Discovery

Before implementing is_user_in_room, search for existing methods:

```bash
# Search for membership check methods
rg "is_user_in_room|check.*membership|user.*member" packages/surrealdb/src/repository/room.rs

# Search for similar patterns in other files
rg "is_user_in_room" packages/
```

If found, reuse it. If not, add as specified in Task 1.

---

## COMPLETION CHECKLIST

Before marking this task complete:

1. Code Changes:
   - [ ] User permission check added and working
   - [ ] Token validation added (if implementing Task 2)
   - [ ] Limit validation added (if implementing Task 3)
   - [ ] Appropriate error logs added
   - [ ] TODO comment removed

2. Code Quality:
   - [ ] No compilation errors
   - [ ] No clippy warnings introduced
   - [ ] Code formatted with `cargo fmt`
   - [ ] Follows existing code style

3. Functionality:
   - [ ] Authorized users can read messages
   - [ ] Unauthorized users get 403 error
   - [ ] Invalid tokens rejected with clear errors
   - [ ] Excessive limits rejected
   - [ ] Existing tests pass (if any)

4. Verification:
   - [ ] Manually tested with valid user
   - [ ] Manually tested with unauthorized user
   - [ ] Manually tested with invalid tokens
   - [ ] Manually tested with various limit values

---

**TASK READY FOR IMPLEMENTATION**

This task is now ready for a developer to implement with complete context about:
- What exists in the codebase
- What needs to be added
- Why it's needed
- How to implement it
- Where to make changes
- How to verify it works

All research has been completed and cited with specific file paths and line numbers.
