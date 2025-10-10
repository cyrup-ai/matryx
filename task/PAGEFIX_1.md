# PAGEFIX_1: Implement Pagination Token Validation

**Status**: Ready for Implementation
**Priority**: HIGH
**Estimated Effort**: 3-5 days
**Package**: packages/server

---

## OBJECTIVE

Implement proper pagination token validation for Matrix room message pagination to prevent security issues, crashes, and data leaks from invalid token handling.

---

## PROBLEM DESCRIPTION

The `/rooms/{roomId}/messages` endpoint currently has a TODO marker and skips pagination token validation:

File: `packages/server/src/_matrix/client/v3/rooms/by_room_id/messages.rs:110-111`
```rust
// TODO: Implement proper from/to token validation
// For now, we'll proceed with the query
```

This causes:
- Invalid pagination tokens can crash the server
- Malformed tokens may leak data or cause incorrect results
- No bounds checking on pagination parameters
- Potential security vulnerability

---

## RESEARCH NOTES

**Matrix Specification**:
- Endpoint: `GET /_matrix/client/v3/rooms/{roomId}/messages`
- Query parameters: `from` (start token), `to` (end token), `dir` (direction), `limit`
- Token format: Server-defined opaque strings, typically encoding timestamp + event_id

**Common Token Formats**:
- Synapse: `s{stream_ordering}_{instance_name}` or `t{topological_ordering}-{stream_ordering}`
- Our format should be: `t{unix_timestamp_millis}_{event_id}`

**Token Requirements**:
- Must be stable (same query returns same token)
- Must encode enough information to resume pagination
- Should be opaque to clients (validated but not relied upon)
- Must handle both forward and backward pagination

---

## SUBTASK 1: Define Pagination Token Structure

**Objective**: Create a strongly-typed PaginationToken struct.

**Location**: `packages/server/src/_matrix/client/v3/rooms/by_room_id/messages.rs`

**Implementation**:

1. Add PaginationToken struct near the top of the file:
```rust
/// Pagination token for room message history
///
/// Format: "t{timestamp}_{event_id}"
/// Example: "t1704067200000_$event123:homeserver.com"
///
/// The timestamp is Unix milliseconds, event_id is the Matrix event ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaginationToken {
    /// Unix timestamp in milliseconds when the event was received
    pub timestamp_ms: i64,
    /// Matrix event ID for this position in the timeline
    pub event_id: String,
}
```

2. Add token encoding/decoding methods:
```rust
impl PaginationToken {
    /// Encode token to string format for API responses
    pub fn encode(&self) -> String {
        format!("t{}_{}", self.timestamp_ms, self.event_id)
    }

    /// Decode token from string, validating format
    pub fn decode(token: &str) -> Result<Self, ApiError> {
        // Validate prefix
        if !token.starts_with('t') {
            return Err(ApiError::InvalidParam {
                param: "pagination_token".to_string(),
                message: "Token must start with 't'".to_string(),
            });
        }

        // Split into parts
        let without_prefix = &token[1..];
        let parts: Vec<&str> = without_prefix.splitn(2, '_').collect();

        if parts.len() != 2 {
            return Err(ApiError::InvalidParam {
                param: "pagination_token".to_string(),
                message: "Token must be in format t{timestamp}_{event_id}".to_string(),
            });
        }

        // Parse timestamp
        let timestamp_ms = parts[0].parse::<i64>().map_err(|_| {
            ApiError::InvalidParam {
                param: "pagination_token".to_string(),
                message: "Invalid timestamp in token".to_string(),
            }
        })?;

        // Validate event ID format
        let event_id = parts[1].to_string();
        if !event_id.starts_with('$') {
            return Err(ApiError::InvalidParam {
                param: "pagination_token".to_string(),
                message: "Event ID must start with '$'".to_string(),
            });
        }

        Ok(PaginationToken {
            timestamp_ms,
            event_id,
        })
    }

    /// Create token from event data
    pub fn from_event(event: &RoomEvent) -> Self {
        Self {
            timestamp_ms: event.origin_server_ts,
            event_id: event.event_id.clone(),
        }
    }
}
```

**Files to Modify**:
- `packages/server/src/_matrix/client/v3/rooms/by_room_id/messages.rs`

**Definition of Done**:
- PaginationToken struct defined with clear documentation
- encode() and decode() methods implemented
- from_event() constructor for creating tokens from events
- Proper error handling with descriptive messages

---

## SUBTASK 2: Replace TODO with Token Validation Logic

**Objective**: Remove TODO marker and implement actual validation.

**Location**: `packages/server/src/_matrix/client/v3/rooms/by_room_id/messages.rs` (around line 105-115)

**Current Code**:
```rust
} else {
    None
};

// TODO: Implement proper from/to token validation
// For now, we'll proceed with the query

// Get paginated messages from database
match state.room_operations.room_repo().get_room_messages_paginated(
```

**Required Implementation**:
```rust
} else {
    None
};

// Validate and parse pagination tokens
let from_token = if let Some(from_str) = query_params.from.as_ref() {
    match PaginationToken::decode(from_str) {
        Ok(token) => Some(token),
        Err(e) => {
            tracing::warn!("Invalid 'from' token: {}", from_str);
            return Err(e);
        }
    }
} else {
    None
};

let to_token = if let Some(to_str) = query_params.to.as_ref() {
    match PaginationToken::decode(to_str) {
        Ok(token) => Some(token),
        Err(e) => {
            tracing::warn!("Invalid 'to' token: {}", to_str);
            return Err(e);
        }
    }
} else {
    None
};

// Validate limit parameter
let limit = query_params.limit.unwrap_or(10);
if limit > 100 {
    return Err(ApiError::InvalidParam {
        param: "limit".to_string(),
        message: "Limit must not exceed 100".to_string(),
    });
}

// Get paginated messages from database
match state.room_operations.room_repo().get_room_messages_paginated(
    &room_id,
    from_token.as_ref(),
    to_token.as_ref(),
    &query_params.dir,
    limit,
).await {
```

**Files to Modify**:
- `packages/server/src/_matrix/client/v3/rooms/by_room_id/messages.rs` (lines 105-115)

**Definition of Done**:
- TODO comment completely removed
- from and to tokens validated before use
- Proper error handling with logging
- limit parameter validated (max 100 per Matrix spec)
- Validated tokens passed to database query

---

## SUBTASK 3: Update Database Query to Use Validated Tokens

**Objective**: Ensure the database layer properly uses the validated tokens.

**Location**: Repository method called by the handler

**Changes Required**:

1. Verify the repository method signature accepts optional PaginationToken:
```rust
pub async fn get_room_messages_paginated(
    &self,
    room_id: &str,
    from_token: Option<&PaginationToken>,
    to_token: Option<&PaginationToken>,
    dir: &str,
    limit: u32,
) -> Result<(Vec<RoomEvent>, Option<String>, Option<String>), RepositoryError> {
```

2. Update query logic to use token timestamp and event_id:
```rust
// Build query based on direction and tokens
let query = match dir {
    "b" => {
        // Backward (earlier messages)
        if let Some(from) = from_token {
            // Start from this token going backward in time
            format!(
                "SELECT * FROM room_events
                 WHERE room_id = $room_id
                 AND (origin_server_ts < $from_ts OR
                      (origin_server_ts = $from_ts AND event_id < $from_event_id))
                 ORDER BY origin_server_ts DESC, event_id DESC
                 LIMIT $limit"
            )
        } else {
            // No token - start from most recent
            format!(
                "SELECT * FROM room_events
                 WHERE room_id = $room_id
                 ORDER BY origin_server_ts DESC, event_id DESC
                 LIMIT $limit"
            )
        }
    }
    "f" => {
        // Forward (later messages)
        if let Some(from) = from_token {
            format!(
                "SELECT * FROM room_events
                 WHERE room_id = $room_id
                 AND (origin_server_ts > $from_ts OR
                      (origin_server_ts = $from_ts AND event_id > $from_event_id))
                 ORDER BY origin_server_ts ASC, event_id ASC
                 LIMIT $limit"
            )
        } else {
            // No token - start from oldest
            format!(
                "SELECT * FROM room_events
                 WHERE room_id = $room_id
                 ORDER BY origin_server_ts ASC, event_id ASC
                 LIMIT $limit"
            )
        }
    }
    _ => return Err(RepositoryError::InvalidDirection),
};
```

3. Generate next/prev tokens from results:
```rust
// Generate pagination tokens for response
let start_token = events.first().map(|e| PaginationToken::from_event(e).encode());
let end_token = events.last().map(|e| PaginationToken::from_event(e).encode());

Ok((events, start_token, end_token))
```

**Files to Modify**:
- Repository file containing `get_room_messages_paginated` method
- Likely in `packages/surrealdb/src/repository/room.rs` or similar

**Definition of Done**:
- Database query properly filters using token timestamp and event_id
- Query handles both forward and backward pagination
- Start and end tokens generated from actual result events
- Query uses indexed fields for performance

---

## SUBTASK 4: Add Error Handling for Edge Cases

**Objective**: Handle edge cases like invalid room IDs, missing permissions, empty results.

**Location**: `packages/server/src/_matrix/client/v3/rooms/by_room_id/messages.rs`

**Edge Cases to Handle**:

1. Room does not exist:
```rust
if !state.room_operations.room_repo().room_exists(&room_id).await? {
    return Err(ApiError::NotFound {
        message: format!("Room {} not found", room_id),
    });
}
```

2. User not in room:
```rust
let user_id = extract_user_id_from_auth(&auth_header)?;
if !state.room_operations.is_user_in_room(&user_id, &room_id).await? {
    return Err(ApiError::Forbidden {
        message: "User not in room".to_string(),
    });
}
```

3. Invalid direction parameter:
```rust
if query_params.dir != "b" && query_params.dir != "f" {
    return Err(ApiError::InvalidParam {
        param: "dir".to_string(),
        message: "Direction must be 'b' (backward) or 'f' (forward)".to_string(),
    });
}
```

4. Both from and to tokens provided (optional validation):
```rust
// Matrix spec allows both, but validate they're in correct order
if let (Some(from), Some(to)) = (&from_token, &to_token) {
    if query_params.dir == "f" && from.timestamp_ms > to.timestamp_ms {
        return Err(ApiError::InvalidParam {
            param: "tokens".to_string(),
            message: "from token must be before to token for forward pagination".to_string(),
        });
    }
}
```

**Files to Modify**:
- `packages/server/src/_matrix/client/v3/rooms/by_room_id/messages.rs`

**Definition of Done**:
- All edge cases handled with appropriate errors
- Error messages are clear and actionable
- Logging added for debugging
- No panics or unwraps on error paths

---

## CONSTRAINTS

⚠️ **NO TESTS**: Do not write unit tests, integration tests, or test fixtures. Test team handles all testing.

⚠️ **NO BENCHMARKS**: Do not write benchmark code. Performance team handles benchmarking.

⚠️ **FOCUS ON FUNCTIONALITY**: Only modify production code in ./src directories.

---

## DEPENDENCIES

**Matrix Specification**:
- Clone: https://github.com/matrix-org/matrix-spec
- Section: Client-Server API
- Endpoint: GET /_matrix/client/v3/rooms/{roomId}/messages

**Existing Code**:
- ApiError types (verify these exist or add as needed)
- RoomEvent struct
- Repository methods

---

## DEFINITION OF DONE

- [ ] PaginationToken struct defined with encode/decode methods
- [ ] TODO comment removed from messages.rs
- [ ] Token validation implemented before database query
- [ ] Database query updated to use validated tokens correctly
- [ ] Edge cases handled (invalid room, no permission, invalid direction)
- [ ] Limit parameter validated (max 100)
- [ ] Start and end tokens generated in responses
- [ ] No compilation errors
- [ ] No test code written
- [ ] No benchmark code written

---

## FILES TO MODIFY

1. `packages/server/src/_matrix/client/v3/rooms/by_room_id/messages.rs` (around lines 105-120)
2. Repository file with `get_room_messages_paginated` method (likely `packages/surrealdb/src/repository/room.rs`)

---

## NOTES

- Token format must be stable and consistent
- Tokens are opaque to clients - validation is server-side only
- Performance: Ensure queries use indexes on (room_id, origin_server_ts, event_id)
- Security: Validate user has permission to read room history
- Matrix spec allows both `from` and `to` tokens for bounded queries
