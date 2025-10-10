# INSTUB_6: Guest Access Authorization Implementation

**Priority**: MEDIUM  
**Estimated Effort**: 1 session  
**Category**: Room Authorization

---

## OBJECTIVE

Implement guest access validation in room authorization to properly enforce Matrix guest access rules based on `m.room.guest_access` state events.

**WHY**: Guest access control is not verified in room operations, meaning guests may have incorrect access to rooms. Rooms configured with `can_join` should allow guest access, while `forbidden` should require membership.

---

## BACKGROUND

**Current Issue**: Test code shows guest access is not verified:
```rust
// In a full implementation, we would test guest access to this specific room
// For now, we verify the room was created with the intended guest access configuration
```

**What We Need**:
- Check `m.room.guest_access` state event
- Allow/deny access based on guest_access value
- Apply checks to relevant room operations

**Matrix Spec**: Guest access controls whether unauthenticated or guest users can access room content.

**Guest Access Values**:
- `can_join` - Guests can access room
- `forbidden` - Only members can access (default)

---

## SUBTASK 1: Create Guest Access Authorization Method

**WHAT**: Implement method to check if a user (including guests) can access a room.

**WHERE**: [`packages/surrealdb/src/repository/room.rs`](../packages/surrealdb/src/repository/room.rs)

**ADD METHOD**:
```rust
/// Result of guest access check
pub enum GuestAccessResult {
    Allowed,
    Forbidden,
    RequiresMembership,
}

impl RoomRepository {
    /// Check if a user can access a room based on guest access rules
    pub async fn check_guest_access(
        &self,
        room_id: &str,
        user_id: Option<&str>,
        is_guest: bool,
    ) -> Result<GuestAccessResult, RepositoryError> {
        // Get room's guest access state event
        let state_events = self.event_repo
            .get_state_events(room_id)
            .await?;
        
        let guest_access_event = state_events
            .into_iter()
            .find(|e| e.event_type == "m.room.guest_access" && e.state_key == Some("".to_string()));
        
        // Extract guest_access value from event content
        let guest_access = if let Some(event) = guest_access_event {
            if let EventContent::Unknown(ref content) = event.content {
                content.get("guest_access")
                    .and_then(|v| v.as_str())
                    .unwrap_or("forbidden")
            } else {
                "forbidden"
            }
        } else {
            "forbidden"  // Default per Matrix spec
        };
        
        match guest_access {
            "can_join" => {
                // Guests can access this room
                Ok(GuestAccessResult::Allowed)
            }
            "forbidden" | _ => {
                // Check if user is a member
                if let Some(uid) = user_id {
                    let membership = self.membership_repo
                        .get_by_room_user(room_id, uid)
                        .await?;
                    
                    match membership {
                        Some(m) if m.membership == MembershipState::Join => {
                            Ok(GuestAccessResult::Allowed)
                        }
                        _ => Ok(GuestAccessResult::RequiresMembership)
                    }
                } else {
                    // No user_id and guest access forbidden
                    Ok(GuestAccessResult::Forbidden)
                }
            }
        }
    }
}
```

**DEPENDENCIES**: Ensure RoomRepository has access to:
- `event_repo: EventRepository`
- `membership_repo: MembershipRepository`

**ADD IF MISSING**:
```rust
pub struct RoomRepository {
    db: Surreal<Any>,
    event_repo: EventRepository,
    membership_repo: MembershipRepository,
}

impl RoomRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self {
            db: db.clone(),
            event_repo: EventRepository::new(db.clone()),
            membership_repo: MembershipRepository::new(db.clone()),
        }
    }
}
```

**DEFINITION OF DONE**:
- ✅ `check_guest_access()` method implemented
- ✅ Returns appropriate result based on guest_access state
- ✅ Checks membership for forbidden rooms
- ✅ Handles missing state events (defaults to forbidden)

---

## SUBTASK 2: Add Guest Flag to Session/Authentication

**WHAT**: Determine how to identify guest users in the authentication system.

**WHERE**: Check session management in [`packages/server/src/security/`](../packages/server/src/security/) or [`packages/surrealdb/src/repository/session.rs`](../packages/surrealdb/src/repository/session.rs)

**OPTIONS**:

**Option A**: Add `is_guest` field to Session:
```rust
pub struct Session {
    pub user_id: String,
    pub access_token: String,
    pub is_guest: bool,  // ADD THIS
    // ... other fields
}
```

**Option B**: Check user type from user_id format:
```rust
fn is_guest_user(user_id: &str) -> bool {
    // Matrix spec: guest user IDs may have special format
    // Or check against user database for guest flag
    user_id.contains("_guest_") || check_user_guest_flag(user_id)
}
```

**Option C**: Query user repository:
```rust
let user = user_repo.get_user(user_id).await?;
let is_guest = user.is_guest;
```

**CHOOSE** the option that fits your authentication system best.

**DEFINITION OF DONE**:
- ✅ Can determine if a user is a guest
- ✅ Works with existing authentication flow
- ✅ Guest flag accessible in endpoint handlers

---

## SUBTASK 3: Apply Guest Access Checks to Room State Endpoint

**WHAT**: Add authorization check to room state retrieval.

**WHERE**: Find room state endpoint, likely [`packages/server/src/_matrix/client/v3/rooms/by_room_id/state.rs`](../packages/server/src/_matrix/client/v3/rooms/by_room_id/state.rs) or similar

**FIND**: Handler for `GET /_matrix/client/v3/rooms/{roomId}/state`

**ADD CHECK** (at beginning of handler):
```rust
pub async fn get_room_state(
    State(state): State<Arc<AppState>>,
    Path(room_id): Path<String>,
    Extension(session): Extension<Session>,
) -> Result<Json<Vec<Event>>, MatrixError> {
    // Check guest access
    let access_result = state.room_repo
        .check_guest_access(
            &room_id,
            Some(&session.user_id),
            session.is_guest,
        )
        .await?;
    
    match access_result {
        GuestAccessResult::Allowed => {
            // Proceed with normal logic
        }
        GuestAccessResult::Forbidden | GuestAccessResult::RequiresMembership => {
            return Err(MatrixError::Forbidden {
                error: "M_FORBIDDEN".to_string(),
                message: "Guest access not allowed in this room".to_string(),
            });
        }
    }
    
    // Continue with existing state retrieval logic
    let state_events = state.event_repo
        .get_state_events(&room_id)
        .await?;
    
    Ok(Json(state_events))
}
```

**DEFINITION OF DONE**:
- ✅ Guest access checked before state retrieval
- ✅ Proper error returned for unauthorized access
- ✅ Authorized users can still access state

---

## SUBTASK 4: Apply Guest Access Checks to Event Retrieval

**WHAT**: Add authorization to event/message retrieval endpoints.

**WHERE**: Message/event endpoints like:
- `GET /_matrix/client/v3/rooms/{roomId}/messages`
- `GET /_matrix/client/v3/rooms/{roomId}/event/{eventId}`

**PATTERN** (apply to each):
```rust
pub async fn get_room_messages(
    State(state): State<Arc<AppState>>,
    Path(room_id): Path<String>,
    Extension(session): Extension<Session>,
    // ... other params
) -> Result<Json<MessagesResponse>, MatrixError> {
    // Check authorization
    let access_result = state.room_repo
        .check_guest_access(&room_id, Some(&session.user_id), session.is_guest)
        .await?;
    
    if matches!(access_result, GuestAccessResult::Forbidden | GuestAccessResult::RequiresMembership) {
        return Err(MatrixError::Forbidden {
            error: "M_FORBIDDEN".to_string(),
            message: "You do not have access to this room".to_string(),
        });
    }
    
    // Continue with existing logic
    // ...
}
```

**FIND ALL ENDPOINTS** that access room content:
```bash
# Search for room content endpoints
grep -r "rooms.*{roomId}" packages/server/src/_matrix/client/v3/rooms/
```

**APPLY TO**: State, messages, events, context endpoints

**DEFINITION OF DONE**:
- ✅ All room content endpoints check guest access
- ✅ Consistent error responses
- ✅ Authorized access still works

---

## SUBTASK 5: Apply Guest Access Checks to Message Sending

**WHAT**: Ensure guests cannot send messages to rooms where guest access is forbidden.

**WHERE**: Message send endpoint, likely [`packages/server/src/_matrix/client/v3/rooms/by_room_id/send.rs`](../packages/server/src/_matrix/client/v3/rooms/by_room_id/send.rs)

**ADD CHECK**:
```rust
pub async fn send_message(
    State(state): State<Arc<AppState>>,
    Path((room_id, event_type, txn_id)): Path<(String, String, String)>,
    Extension(session): Extension<Session>,
    Json(content): Json<Value>,
) -> Result<Json<SendEventResponse>, MatrixError> {
    // Check if guest can post
    let access_result = state.room_repo
        .check_guest_access(&room_id, Some(&session.user_id), session.is_guest)
        .await?;
    
    // Guests typically cannot send messages even in can_join rooms
    // unless they're also members
    if session.is_guest {
        let membership = state.membership_repo
            .get_by_room_user(&room_id, &session.user_id)
            .await?;
        
        if !matches!(membership, Some(m) if m.membership == MembershipState::Join) {
            return Err(MatrixError::Forbidden {
                error: "M_GUEST_ACCESS_FORBIDDEN".to_string(),
                message: "Guests must join rooms before sending messages".to_string(),
            });
        }
    }
    
    // Continue with message sending
    // ...
}
```

**NOTE**: Matrix spec typically requires guests to join rooms before sending, even if guest_access is `can_join`.

**DEFINITION OF DONE**:
- ✅ Guest message sending properly restricted
- ✅ Guests must join before sending
- ✅ Regular members unaffected

---

## SUBTASK 6: Handle Room Preview (Optional Enhancement)

**WHAT**: Allow guests to preview room state in `can_join` rooms without joining.

**WHERE**: Room preview/peek endpoints

**CONSIDER**: Matrix has special "peeking" functionality for guests to view room state without joining.

**IF IMPLEMENTING**:
```rust
pub async fn peek_room(
    State(state): State<Arc<AppState>>,
    Path(room_id): Path<String>,
    Extension(session): Extension<Session>,
) -> Result<Json<RoomStatePreview>, MatrixError> {
    // Guest access must be can_join for peeking
    let access_result = state.room_repo
        .check_guest_access(&room_id, Some(&session.user_id), session.is_guest)
        .await?;
    
    if !matches!(access_result, GuestAccessResult::Allowed) {
        return Err(MatrixError::Forbidden {
            error: "M_FORBIDDEN".to_string(),
            message: "Cannot peek this room".to_string(),
        });
    }
    
    // Return limited state for preview
    // ...
}
```

**NOTE**: This is optional - basic implementation can skip room peeking.

**DEFINITION OF DONE** (if implementing):
- ✅ Guests can peek can_join rooms
- ✅ Peek returns limited state info
- ✅ Forbidden rooms cannot be peeked

---

## SUBTASK 7: Add Helper Method for Common Check

**WHAT**: Create reusable authorization helper to reduce code duplication.

**WHERE**: [`packages/server/src/security/authorization.rs`](../packages/server/src/security/authorization.rs) or similar

**CREATE**:
```rust
/// Check if user has access to room content
pub async fn require_room_access(
    room_repo: &RoomRepository,
    room_id: &str,
    user_id: &str,
    is_guest: bool,
) -> Result<(), MatrixError> {
    let access_result = room_repo
        .check_guest_access(room_id, Some(user_id), is_guest)
        .await
        .map_err(|e| MatrixError::InternalServerError {
            message: format!("Failed to check room access: {}", e),
        })?;
    
    match access_result {
        GuestAccessResult::Allowed => Ok(()),
        _ => Err(MatrixError::Forbidden {
            error: "M_FORBIDDEN".to_string(),
            message: "You do not have access to this room".to_string(),
        }),
    }
}
```

**USE IN ENDPOINTS**:
```rust
// Simplified endpoint code
require_room_access(&state.room_repo, &room_id, &session.user_id, session.is_guest).await?;
```

**DEFINITION OF DONE**:
- ✅ Helper method reduces boilerplate
- ✅ Consistent error handling
- ✅ Easy to use across endpoints

---

## SUBTASK 8: Verify Compilation and Integration

**WHAT**: Ensure all changes compile and integrate.

**WHERE**: Run from workspace root

**HOW**:
```bash
# Build packages
cargo build --package matryx_surrealdb
cargo build --package matryx_server

# Check for errors
cargo check --workspace
```

**VERIFY**: No compilation errors, all imports work.

**DEFINITION OF DONE**:
- ✅ Code compiles successfully
- ✅ No type mismatches
- ✅ All imports resolved

---

## RESEARCH NOTES

### Matrix Guest Access Specification
Location: [`./spec/client/05_advanced_features.md`](../spec/client/05_advanced_features.md)

**m.room.guest_access State Event**:
```json
{
  "type": "m.room.guest_access",
  "state_key": "",
  "content": {
    "guest_access": "can_join"
  }
}
```

**Values**:
- `can_join` - Guests can access room (read state/messages, may join)
- `forbidden` - Only members can access (default)

**Default Behavior**: If no `m.room.guest_access` event, assume `forbidden`.

**Guest Restrictions**:
- Guests typically cannot send messages until they join
- Guests may have limited state access
- Some servers restrict guest registration

### Related State Events
- `m.room.history_visibility` - Controls what history guests can see
- `m.room.join_rules` - Controls who can join (works with guest_access)

### Room Authorization Flow
```
Request → Authentication → Guest Access Check → Membership Check → Allow/Deny
```

### Error Codes
- `M_FORBIDDEN` - Generic access denied
- `M_GUEST_ACCESS_FORBIDDEN` - Guest-specific denial

---

## DEFINITION OF DONE

**Task complete when**:
- ✅ `check_guest_access()` method implemented in RoomRepository
- ✅ Guest flag available in authentication/session system
- ✅ Room state endpoints check guest access
- ✅ Event retrieval endpoints check guest access
- ✅ Message sending respects guest restrictions
- ✅ Proper Matrix error codes returned
- ✅ Code compiles successfully
- ✅ Guest access control works per Matrix spec

**ACCEPTABLE TO DEFER**:
- Room peeking/preview functionality (optional)
- Advanced guest restrictions
- Fine-grained permissions

**NO REQUIREMENTS FOR**:
- ❌ Unit tests
- ❌ Integration tests
- ❌ Benchmarks
- ❌ Documentation (beyond code comments)

---

## RELATED FILES

- [`packages/surrealdb/src/repository/room.rs`](../packages/surrealdb/src/repository/room.rs) - Add check_guest_access()
- [`packages/surrealdb/src/repository/event.rs`](../packages/surrealdb/src/repository/event.rs) - State event queries
- [`packages/surrealdb/src/repository/membership.rs`](../packages/surrealdb/src/repository/membership.rs) - Membership checks
- [`packages/server/src/_matrix/client/v3/rooms/`](../packages/server/src/_matrix/client/v3/rooms/) - Room endpoints to modify
- [`packages/server/src/security/`](../packages/server/src/security/) - Session/auth system
- [`./spec/client/05_advanced_features.md`](../spec/client/05_advanced_features.md) - Guest access spec
