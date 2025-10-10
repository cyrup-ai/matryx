# MEDIAFIX_1: Add Test Helper for Expired Media Uploads

**Status**: Ready for Implementation  
**Priority**: MEDIUM  
**Estimated Effort**: 2-4 hours  
**Package**: packages/surrealdb

---

## OBJECTIVE

Replace "for now" workaround in media service test with proper test helper method for creating expired uploads, enabling proper testing of expiration cleanup logic.

---

## PROBLEM DESCRIPTION

Media service test has incomplete test infrastructure for testing expired upload cleanup:

**Location**: [`packages/surrealdb/src/repository/media_service_test.rs:433-440`](../packages/surrealdb/src/repository/media_service_test.rs)

```rust
// Access the database directly to set expires_at to the past
// Note: This requires direct database manipulation
// For now, we'll test with a known expired ID
let expired_media_id = "expired-media-id-12345";

let result = media_service
    .upload_to_pending(expired_media_id, server_name, user_id, b"content", "text/plain")
    .await;
```

**Current Issues**:
- Test doesn't actually create expired uploads - just uses a fake ID
- Direct database manipulation mentioned but not implemented  
- Cleanup testing is incomplete - doesn't verify cleanup actually works
- Expiration logic may not be properly validated

---

## CODEBASE RESEARCH FINDINGS

### Existing Infrastructure (GOOD NEWS!)

**1. PendingUpload Structure** - ✅ **ALREADY EXISTS**

Location: [`packages/surrealdb/src/repository/media.rs:24-31`](../packages/surrealdb/src/repository/media.rs)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingUpload {
    pub media_id: String,
    pub server_name: String,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,  // ← We need to set this to the past
    pub status: PendingUploadStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PendingUploadStatus {
    Pending,
    Completed,
    Expired,
}
```

**2. Cleanup Method** - ✅ **ALREADY EXISTS IN REPOSITORY**

Location: [`packages/surrealdb/src/repository/media.rs:643-667`](../packages/surrealdb/src/repository/media.rs)

```rust
/// Cleanup expired pending uploads
pub async fn cleanup_expired_pending_uploads(&self) -> Result<u64, RepositoryError> {
    let now = Utc::now();
    let query = "
        SELECT media_id, server_name FROM pending_uploads
        WHERE expires_at < $now AND status = 'Pending'
    ";
    let mut result = self.db.query(query).bind(("now", now)).await?;
    let expired_uploads: Vec<serde_json::Value> = result.take(0)?;

    let mut deleted_count = 0;
    for upload in expired_uploads {
        if let (Some(media_id), Some(server_name)) = (
            upload.get("media_id").and_then(|v| v.as_str()),
            upload.get("server_name").and_then(|v| v.as_str()),
        ) {
            let delete_query = "
                DELETE pending_uploads
                WHERE media_id = $media_id AND server_name = $server_name
            ";
            if self
                .db
                .query(delete_query)
                .bind(("media_id", media_id.to_string()))
                .bind(("server_name", server_name.to_string()))
                .await
                .is_ok()
            {
                deleted_count += 1;
            }
        }
    }

    Ok(deleted_count)
}
```

**3. MediaService Structure**

Location: [`packages/surrealdb/src/repository/media_service.rs:103-109`](../packages/surrealdb/src/repository/media_service.rs)

```rust
pub struct MediaService<C: Connection> {
    media_repo: Arc<MediaRepository<C>>,  // ← Has access to cleanup method
    room_repo: Arc<RoomRepository>,
    membership_repo: Arc<MembershipRepository>,
    federation_media_client: Option<Arc<dyn FederationMediaClientTrait>>,
    homeserver_name: String,
}
```

### What's Missing

1. ❌ MediaService doesn't expose `cleanup_expired_uploads` publicly
2. ❌ MediaService doesn't have `create_expired_upload` test helper  
3. ❌ Test uses workaround instead of proper expired upload creation

---

## IMPLEMENTATION STRATEGY

Since cleanup logic already exists in MediaRepository, this task is **simpler than originally scoped**:

1. Add **wrapper method** in MediaService to expose repository cleanup
2. Add **test helper** with `#[cfg(test)]` to create expired uploads
3. **Update test** to use helper instead of workaround
4. Add **comprehensive test cases** for edge cases

---

## SUBTASK 1: Add cleanup_expired_uploads to MediaService

**Objective**: Expose repository cleanup method through MediaService.

**Location**: [`packages/surrealdb/src/repository/media_service.rs`](../packages/surrealdb/src/repository/media_service.rs)

**Add to `impl<C: Connection> MediaService<C>`** (around line 750, after `upload_to_pending`):

```rust
/// Clean up expired pending uploads
///
/// Removes pending uploads where expires_at is in the past. Should be
/// called periodically to free storage and maintain database hygiene.
///
/// # Returns
/// The number of uploads that were deleted
///
/// # Example
/// ```ignore
/// let deleted = media_service.cleanup_expired_uploads().await?;
/// tracing::info!("Cleaned up {} expired uploads", deleted);
/// ```
pub async fn cleanup_expired_uploads(&self) -> Result<u64, MediaError> {
    self.media_repo
        .cleanup_expired_pending_uploads()
        .await
        .map_err(MediaError::from)
}
```

**Why This Works**:
- `media_repo` already has the cleanup implementation
- MediaService just wraps it with proper error conversion
- Simple delegation pattern used elsewhere in MediaService

**Files to Modify**:
- `packages/surrealdb/src/repository/media_service.rs` (add ~15 lines)

---

## SUBTASK 2: Add Test Helper Method to MediaService

**Objective**: Create `create_expired_upload` method for testing.

**Location**: [`packages/surrealdb/src/repository/media_service.rs`](../packages/surrealdb/src/repository/media_service.rs)

**Add to `impl<C: Connection> MediaService<C>`** (after `cleanup_expired_uploads`):

```rust
/// Create an expired upload for testing purposes
///
/// This method is only available in test builds and allows creation
/// of uploads with past expiration times to test cleanup logic.
///
/// # Arguments
/// * `media_id` - Unique identifier for the media
/// * `server_name` - Server hosting the media
/// * `user_id` - User who "uploaded" the media  
/// * `expired_seconds_ago` - How many seconds ago the upload expired
///
/// # Example
/// ```ignore
/// // Create upload that expired 1 hour ago
/// media_service.create_expired_upload(
///     "test-media-id",
///     "homeserver.com",
///     "@user:homeserver.com",
///     3600
/// ).await?;
/// ```
#[cfg(test)]
pub async fn create_expired_upload(
    &self,
    media_id: &str,
    server_name: &str,
    user_id: &str,
    expired_seconds_ago: i64,
) -> Result<(), MediaError> {
    use chrono::Duration;

    let now = Utc::now();
    let expired_at = now - Duration::seconds(expired_seconds_ago);

    // Use existing repository method with past expiration time
    self.media_repo
        .create_pending_upload(media_id, server_name, user_id, expired_at)
        .await
        .map_err(MediaError::from)
}
```

**Why This Works**:
- Uses existing `media_repo.create_pending_upload` with past date
- `#[cfg(test)]` ensures it's only compiled in test builds
- Simple and leverages existing infrastructure

**Files to Modify**:
- `packages/surrealdb/src/repository/media_service.rs` (add ~30 lines)

---

## SUBTASK 3: Update Test to Use New Helper

**Objective**: Replace workaround with proper helper method call.

**Location**: [`packages/surrealdb/src/repository/media_service_test.rs:433-440`](../packages/surrealdb/src/repository/media_service_test.rs)

**Current Code** (REMOVE):
```rust
// Access the database directly to set expires_at to the past
// Note: This requires direct database manipulation
// For now, we'll test with a known expired ID
let expired_media_id = "expired-media-id-12345";

let result = media_service
    .upload_to_pending(expired_media_id, server_name, user_id, b"content", "text/plain")
    .await;
```

**New Code** (REPLACE WITH):
```rust
// Create an upload that expired 1 hour ago using test helper
let expired_media_id = "expired-media-id-12345";

media_service
    .create_expired_upload(
        expired_media_id,
        server_name,
        user_id,
        3600 // Expired 1 hour ago
    )
    .await
    .expect("Failed to create expired upload for testing");

// Now test that cleanup removes the expired upload
let cleanup_count = media_service
    .cleanup_expired_uploads()
    .await
    .expect("Failed to cleanup expired uploads");

assert_eq!(cleanup_count, 1, "Should have cleaned up 1 expired upload");

// Verify the expired upload was actually deleted
let pending = media_service
    .media_repo
    .get_pending_upload(expired_media_id, server_name)
    .await
    .expect("Failed to query pending upload");

assert!(pending.is_none(), "Expired upload should have been deleted");
```

**Why This Works**:
- Actually creates an expired upload (not just a fake ID)
- Tests cleanup functionality end-to-end
- Verifies upload is deleted after cleanup

**Files to Modify**:
- `packages/surrealdb/src/repository/media_service_test.rs` (replace lines 433-440)

---

## SUBTASK 4: Add Comprehensive Expiration Tests

**Objective**: Add test cases for expiration edge cases.

**Location**: [`packages/surrealdb/src/repository/media_service_test.rs`](../packages/surrealdb/src/repository/media_service_test.rs)

**Add these new test functions** (after existing tests, around line 500):

### Test 1: Multiple Expired Uploads

```rust
#[tokio::test]
async fn test_cleanup_multiple_expired_uploads() {
    let media_service = create_media_service().await;
    let server_name = "example.com";
    let user_id = "@test:example.com";

    // Create 5 expired uploads with staggered expiration times
    for i in 0..5 {
        media_service
            .create_expired_upload(
                &format!("expired-{}", i),
                server_name,
                user_id,
                3600 + (i * 60) // 1 hour + i minutes ago
            )
            .await
            .expect("Failed to create expired upload");
    }

    // Cleanup should remove all 5
    let cleanup_count = media_service
        .cleanup_expired_uploads()
        .await
        .expect("Cleanup failed");

    assert_eq!(cleanup_count, 5, "Should cleanup all 5 expired uploads");
}
```

### Test 2: Mix of Expired and Active Uploads

```rust
#[tokio::test]
async fn test_cleanup_preserves_active_uploads() {
    let media_service = create_media_service().await;
    let server_name = "example.com";
    let user_id = "@test:example.com";

    // Create 3 expired uploads
    for i in 0..3 {
        media_service
            .create_expired_upload(
                &format!("expired-{}", i),
                server_name,
                user_id,
                3600 // Expired 1 hour ago
            )
            .await
            .expect("Failed to create expired upload");
    }

    // Create 2 active uploads (normal way with future expiration)
    for i in 0..2 {
        media_service
            .create_pending_upload(user_id, server_name)
            .await
            .expect("Failed to create active upload");
    }

    // Cleanup should only remove expired ones
    let cleanup_count = media_service
        .cleanup_expired_uploads()
        .await
        .expect("Cleanup failed");

    assert_eq!(cleanup_count, 3, "Should only cleanup expired uploads");

    // Verify active uploads still exist by counting pending uploads
    let active_count = media_service
        .media_repo
        .count_user_pending_uploads(user_id)
        .await
        .expect("Failed to count pending uploads");

    assert_eq!(active_count, 2, "Active uploads should still exist");
}
```

### Test 3: No Expired Uploads

```rust
#[tokio::test]
async fn test_cleanup_with_no_expired_uploads() {
    let media_service = create_media_service().await;

    // Run cleanup on empty database
    let cleanup_count = media_service
        .cleanup_expired_uploads()
        .await
        .expect("Cleanup failed");

    assert_eq!(cleanup_count, 0, "Should cleanup nothing when no expired uploads");
}
```

### Test 4: Edge Case - Just Expired

```rust
#[tokio::test]
async fn test_cleanup_just_expired_upload() {
    let media_service = create_media_service().await;
    let server_name = "example.com";
    let user_id = "@test:example.com";

    // Create upload that expired 1 second ago
    media_service
        .create_expired_upload(
            "just-expired",
            server_name,
            user_id,
            1 // Expired 1 second ago
        )
        .await
        .expect("Failed to create expired upload");

    // Should still be cleaned up
    let cleanup_count = media_service
        .cleanup_expired_uploads()
        .await
        .expect("Cleanup failed");

    assert_eq!(cleanup_count, 1, "Should cleanup upload expired 1 second ago");
}
```

**Files to Modify**:
- `packages/surrealdb/src/repository/media_service_test.rs` (add ~100 lines)

---

## IMPLEMENTATION NOTES

### Why #[cfg(test)] is Used

```rust
#[cfg(test)]  // ← Only compiles in test builds
pub async fn create_expired_upload(...) { ... }
```

**Benefits**:
- No production code bloat
- Clear separation of test utilities
- Standard Rust testing pattern
- No risk of accidental use in production

### Access Pattern for Repository in Tests

Tests can access `media_repo` directly through MediaService:

```rust
media_service.media_repo.get_pending_upload(...)  // ✅ Works in tests
```

This allows verification that cleanup actually deleted records.

### Error Handling Pattern

Follow existing MediaService pattern:

```rust
self.media_repo
    .cleanup_expired_pending_uploads()
    .await
    .map_err(MediaError::from)  // ← Convert RepositoryError to MediaError
```

---

## CODE LOCATION REFERENCE

### Files to Read/Understand

1. **PendingUpload Structure**  
   [`packages/surrealdb/src/repository/media.rs:24-31`](../packages/surrealdb/src/repository/media.rs)

2. **Existing Cleanup Implementation**  
   [`packages/surrealdb/src/repository/media.rs:643-667`](../packages/surrealdb/src/repository/media.rs)

3. **MediaService Structure**  
   [`packages/surrealdb/src/repository/media_service.rs:103-109`](../packages/surrealdb/src/repository/media_service.rs)

4. **Current Workaround**  
   [`packages/surrealdb/src/repository/media_service_test.rs:433-440`](../packages/surrealdb/src/repository/media_service_test.rs)

### Files to Modify

1. **`packages/surrealdb/src/repository/media_service.rs`**
   - Add `cleanup_expired_uploads` method (~15 lines)
   - Add `create_expired_upload` test helper with `#[cfg(test)]` (~30 lines)
   - Total additions: ~45 lines

2. **`packages/surrealdb/src/repository/media_service_test.rs`**  
   - Update test at lines 433-440 (~20 lines replacement)
   - Add 4 new test functions (~100 lines)
   - Total changes: ~120 lines

---

## DEPENDENCIES

### Required Crates (Already in Cargo.toml)

```toml
chrono = { version = "0.4", features = ["serde"] }  # DateTime handling
surrealdb = "3.0"                                    # Database
tokio = { version = "1", features = ["full"] }       # Async runtime
```

### Existing Code Dependencies

- ✅ `PendingUpload` struct exists
- ✅ `MediaRepository::cleanup_expired_pending_uploads` exists
- ✅ `MediaRepository::create_pending_upload` exists  
- ✅ `MediaRepository::get_pending_upload` exists
- ✅ Test infrastructure (`create_media_service`) exists

**No new dependencies needed!** Everything required already exists in the codebase.

---

## DEFINITION OF DONE

### Code Changes Complete

- [ ] `cleanup_expired_uploads` method added to MediaService
- [ ] `create_expired_upload` test helper added with `#[cfg(test)]`
- [ ] Test at line 433-440 updated to use helper
- [ ] "For now" and workaround comments removed
- [ ] 4 comprehensive test cases added:
  - Multiple expired uploads
  - Mix of expired and active uploads  
  - No expired uploads (empty case)
  - Just expired (1 second ago edge case)

### Quality Checks

- [ ] All tests pass: `cargo test -p matryx_surrealdb`
- [ ] No compilation errors
- [ ] No clippy warnings
- [ ] Code follows existing MediaService patterns
- [ ] Test helper only available in test builds

### Verification

Run these commands to verify:

```bash
# Build succeeds
cargo build -p matryx_surrealdb

# Tests pass
cargo test -p matryx_surrealdb test_upload_to_pending_expired
cargo test -p matryx_surrealdb test_cleanup_multiple_expired_uploads
cargo test -p matryx_surrealdb test_cleanup_preserves_active_uploads
cargo test -p matryx_surrealdb test_cleanup_with_no_expired_uploads
cargo test -p matryx_surrealdb test_cleanup_just_expired_upload

# No warnings
cargo clippy -p matryx_surrealdb
```

---

## NOTES

### Production Considerations

After this task, consider adding:
- Scheduled cleanup job (cron or tokio interval)
- Monitoring/metrics for expired upload cleanup
- Admin API to manually trigger cleanup

**But these are OUT OF SCOPE for this task.**

### Why This Matters

- **Better test coverage** - Actually tests expiration logic
- **No workarounds** - Proper test infrastructure  
- **Production ready** - Cleanup method exposed for scheduled jobs
- **Maintainable** - Clear separation of test and production code

### Time Estimate Breakdown

- Subtask 1: 30 minutes (simple wrapper method)
- Subtask 2: 30 minutes (test helper)  
- Subtask 3: 30 minutes (update existing test)
- Subtask 4: 1 hour (4 new test cases)
- Testing/verification: 30 minutes

**Total: 2-4 hours**
