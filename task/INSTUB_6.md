# INSTUB_6: Guest Access Authorization Implementation - FINAL ITEM

**Priority**: MEDIUM  
**Estimated Effort**: 15 minutes  
**Category**: Room Authorization

---

## OBJECTIVE

Complete the guest access validation implementation by adding the missing check to the read_markers endpoint.

**STATUS**: 95% Complete - One endpoint missing guest access check

---

## OUTSTANDING ITEM

### Apply Guest Access Check to Read Markers Endpoint

**WHAT**: Add guest access authorization to the read markers endpoint.

**WHERE**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v3/rooms/by_room_id/read_markers.rs`

**ISSUE**: The POST `/_matrix/client/v3/rooms/{roomId}/read_markers` endpoint allows users to update read markers and send read receipts without checking guest access rules. This is a room content access operation that should be protected.

**FIX REQUIRED**: Add guest access check after authentication extraction (around line 54, after extracting user_id).

**CODE TO ADD** (after line 54):
```rust
// Get session to check if user is a guest
let session_repo = matryx_surrealdb::repository::SessionRepository::new(state.db.clone());
let session = session_repo
    .get_by_access_token(&/* token from auth */)
    .await
    .map_err(|e| {
        error!("Failed to get session: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

let is_guest = session.map(|s| s.is_guest).unwrap_or(false);

// Check guest access before allowing read marker updates
let room_repo = matryx_surrealdb::repository::RoomRepository::new(state.db.clone());
crate::room::authorization::require_room_access(&room_repo, &room_id, &user_id, is_guest)
    .await?;
```

**NOTE**: You'll need to extract the token from the auth object to get the session. Follow the pattern used in `messages.rs` (lines 95-111) and `context/by_event_id.rs` (lines 45-58).

**DEFINITION OF DONE**:
- ✅ Guest access checked before processing read markers
- ✅ Follows same pattern as messages.rs and context endpoints
- ✅ Code compiles without errors in this file

---

## COMPLETED ITEMS

✅ **Subtask 1**: `check_guest_access()` method implemented in RoomRepository  
✅ **Subtask 2**: `is_guest` field available in Session struct  
✅ **Subtask 3**: Room state endpoint checks guest access  
✅ **Subtask 4**: Event retrieval endpoints check guest access (messages, context) - **ONE ENDPOINT MISSING: read_markers**  
✅ **Subtask 5**: Message sending respects guest restrictions  
✅ **Subtask 7**: `require_room_access()` helper method implemented  
✅ **Subtask 8**: Code compiles (guest access implementation is clean)

---

## RELATED FILES

- `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v3/rooms/by_room_id/read_markers.rs` - NEEDS FIX
- `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v3/rooms/by_room_id/messages.rs` - REFERENCE PATTERN
- `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v3/rooms/by_room_id/context/by_event_id.rs` - REFERENCE PATTERN
- `/Volumes/samsung_t9/maxtryx/packages/server/src/room/authorization.rs` - Helper method
- `/Volumes/samsung_t9/maxtryx/packages/surrealdb/src/repository/room.rs` - check_guest_access() implementation
