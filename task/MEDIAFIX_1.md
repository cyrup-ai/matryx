# MEDIAFIX_1: Add Test Helper for Expired Media Uploads

**Status**: Ready for Implementation
**Priority**: MEDIUM
**Estimated Effort**: 1 day
**Package**: packages/surrealdb

---

## OBJECTIVE

Replace "for now" workaround in media service with proper test helper method for creating expired uploads, enabling proper testing of expiration cleanup logic.

---

## PROBLEM DESCRIPTION

Media service test has a "for now" comment indicating incomplete test infrastructure:

File: `packages/surrealdb/src/repository/media_service_test.rs:433-440`
```rust
// Access the database directly to set expires_at to the past
// Note: This requires direct database manipulation
// For now, we'll test with a known expired ID
let expired_media_id = "expired-media-id-12345";

let result = media_service
    .upload_to_pending(expired_media_id, server_name, user_id, b"content", "text/plain")
    .await;
```

**Issues**:
- Test doesn't actually create expired uploads
- Direct database manipulation mentioned but not implemented
- Cleanup testing is incomplete
- Expiration logic may not be properly validated

---

## RESEARCH NOTES

**Media Upload Expiration**:
- Pending uploads have a time-to-live (typically 1 hour)
- Expired uploads should be cleaned up to free storage
- Need to test cleanup logic works correctly
- Can't wait real-time for expiration in tests

**Test-Only Methods**:
- Using `#[cfg(test)]` attribute makes methods only available in test builds
- Allows internal access for testing without exposing to production
- Common pattern for testability in Rust

---

## SUBTASK 1: Define PendingUpload Structure

**Objective**: Ensure the data structure for pending uploads is well-defined.

**Location**: `packages/surrealdb/src/repository/media_service.rs` or related file

**Verify/Define Structure**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingUpload {
    /// Unique media identifier
    pub media_id: String,

    /// Server name where media is hosted
    pub server_name: String,

    /// User who uploaded the media
    pub user_id: String,

    /// When the upload was created
    pub created_at: DateTime<Utc>,

    /// When the upload expires and should be cleaned up
    pub expires_at: DateTime<Utc>,

    /// Current status of the upload
    pub status: UploadStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UploadStatus {
    Pending,
    Completed,
    Failed,
}
```

If this structure doesn't exist or differs, update accordingly.

**Files to Review/Modify**:
- `packages/surrealdb/src/repository/media_service.rs`
- `packages/entity/src/types/media.rs` (if types are defined there)

**Definition of Done**:
- PendingUpload struct exists and is documented
- All required fields present (especially expires_at)
- Proper DateTime types used (chrono::DateTime<Utc>)

---

## SUBTASK 2: Add Test Helper Method to MediaService

**Objective**: Create `create_expired_upload` method for testing.

**Location**: `packages/surrealdb/src/repository/media_service.rs`

**Implementation**:

Add method to MediaService impl block:
```rust
impl MediaService {
    // ... existing methods ...

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
    /// ```
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
    ) -> Result<(), RepositoryError> {
        use chrono::{Duration, Utc};

        let now = Utc::now();
        let expired_at = now - Duration::seconds(expired_seconds_ago);

        // Create upload with past expiration time
        let upload = PendingUpload {
            media_id: media_id.to_string(),
            server_name: server_name.to_string(),
            user_id: user_id.to_string(),
            created_at: expired_at - Duration::hours(1), // Created before expiration
            expires_at: expired_at,
            status: UploadStatus::Pending,
        };

        // Insert directly into database
        self.db
            .create("pending_uploads")
            .content(upload)
            .await
            .map_err(|e| RepositoryError::DatabaseError(format!(
                "Failed to create expired upload for testing: {}",
                e
            )))?;

        Ok(())
    }
}
```

**Files to Modify**:
- `packages/surrealdb/src/repository/media_service.rs`

**Definition of Done**:
- Method has `#[cfg(test)]` attribute (only in test builds)
- Takes expired_seconds_ago parameter for flexibility
- Properly creates upload with past expiration time
- Error handling with descriptive messages
- Documentation with example usage

---

## SUBTASK 3: Update Test to Use New Helper

**Objective**: Replace workaround in test with proper helper method call.

**Location**: `packages/surrealdb/src/repository/media_service_test.rs` (around line 433)

**Current Code**:
```rust
// Access the database directly to set expires_at to the past
// Note: This requires direct database manipulation
// For now, we'll test with a known expired ID
let expired_media_id = "expired-media-id-12345";

let result = media_service
    .upload_to_pending(expired_media_id, server_name, user_id, b"content", "text/plain")
    .await;
```

**Updated Implementation**:
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
    .get_pending_upload(expired_media_id)
    .await
    .expect("Failed to query pending upload");

assert!(pending.is_none(), "Expired upload should have been deleted");
```

**Files to Modify**:
- `packages/surrealdb/src/repository/media_service_test.rs` (lines 433-440)

**Definition of Done**:
- Workaround comment removed
- Test uses create_expired_upload helper
- Test verifies cleanup actually works
- Test confirms upload is deleted after cleanup
- No compilation errors

---

## SUBTASK 4: Add Comprehensive Expiration Tests

**Objective**: Add more test cases for expiration edge cases.

**Location**: `packages/surrealdb/src/repository/media_service_test.rs`

**New Test Cases**:

1. Test multiple expired uploads:
```rust
#[tokio::test]
async fn test_cleanup_multiple_expired_uploads() {
    let media_service = create_test_media_service().await;
    let server_name = "homeserver.com";
    let user_id = "@user:homeserver.com";

    // Create 5 expired uploads
    for i in 0..5 {
        media_service
            .create_expired_upload(
                &format!("expired-{}", i),
                server_name,
                user_id,
                3600 + (i * 60) // Stagger expiration times
            )
            .await
            .expect("Failed to create expired upload");
    }

    // Cleanup should remove all 5
    let cleanup_count = media_service
        .cleanup_expired_uploads()
        .await
        .expect("Cleanup failed");

    assert_eq!(cleanup_count, 5);
}
```

2. Test mix of expired and non-expired:
```rust
#[tokio::test]
async fn test_cleanup_preserves_active_uploads() {
    let media_service = create_test_media_service().await;
    let server_name = "homeserver.com";
    let user_id = "@user:homeserver.com";

    // Create 3 expired uploads
    for i in 0..3 {
        media_service
            .create_expired_upload(
                &format!("expired-{}", i),
                server_name,
                user_id,
                3600
            )
            .await
            .expect("Failed to create expired upload");
    }

    // Create 2 active uploads (normal way)
    for i in 0..2 {
        media_service
            .upload_to_pending(
                &format!("active-{}", i),
                server_name,
                user_id,
                b"content",
                "text/plain"
            )
            .await
            .expect("Failed to create active upload");
    }

    // Cleanup should only remove expired ones
    let cleanup_count = media_service
        .cleanup_expired_uploads()
        .await
        .expect("Cleanup failed");

    assert_eq!(cleanup_count, 3, "Should only cleanup expired uploads");

    // Verify active uploads still exist
    for i in 0..2 {
        let pending = media_service
            .get_pending_upload(&format!("active-{}", i))
            .await
            .expect("Query failed");

        assert!(pending.is_some(), "Active upload should still exist");
    }
}
```

3. Test cleanup with no expired uploads:
```rust
#[tokio::test]
async fn test_cleanup_with_no_expired_uploads() {
    let media_service = create_test_media_service().await;

    let cleanup_count = media_service
        .cleanup_expired_uploads()
        .await
        .expect("Cleanup failed");

    assert_eq!(cleanup_count, 0, "Should cleanup nothing when no expired uploads");
}
```

**Files to Modify**:
- `packages/surrealdb/src/repository/media_service_test.rs`

**Definition of Done**:
- Multiple test cases cover different scenarios
- Tests verify cleanup works correctly
- Tests verify non-expired uploads are preserved
- Edge cases covered (no expired uploads, all expired, mixed)

---

## SUBTASK 5: Ensure cleanup_expired_uploads Method Exists

**Objective**: Verify or implement the cleanup method that the tests rely on.

**Location**: `packages/surrealdb/src/repository/media_service.rs`

**Expected Method**:
```rust
impl MediaService {
    /// Clean up expired pending uploads
    ///
    /// Removes uploads where expires_at is in the past. This should be
    /// called periodically to free storage and maintain database hygiene.
    ///
    /// # Returns
    /// The number of uploads that were deleted
    pub async fn cleanup_expired_uploads(&self) -> Result<usize, RepositoryError> {
        use chrono::Utc;

        let now = Utc::now();

        // Query for expired uploads
        let query = r#"
            DELETE pending_uploads
            WHERE status = 'Pending'
            AND expires_at < $now
            RETURN BEFORE
        "#;

        let mut result = self.db
            .query(query)
            .bind(("now", now))
            .await
            .map_err(|e| RepositoryError::DatabaseError(format!(
                "Failed to cleanup expired uploads: {}",
                e
            )))?;

        // Count deleted records
        let deleted: Vec<PendingUpload> = result
            .take(0)
            .map_err(|e| RepositoryError::DatabaseError(format!(
                "Failed to parse cleanup results: {}",
                e
            )))?;

        let count = deleted.len();

        tracing::info!("Cleaned up {} expired uploads", count);

        Ok(count)
    }
}
```

If this method doesn't exist, implement it. If it exists but differs, verify it works correctly with the tests.

**Files to Review/Modify**:
- `packages/surrealdb/src/repository/media_service.rs`

**Definition of Done**:
- cleanup_expired_uploads method exists and works
- Returns count of deleted uploads
- Only deletes expired uploads (expires_at < now)
- Proper error handling
- Logging for operations

---

## CONSTRAINTS

⚠️ **NO TESTS**: Do not write additional test infrastructure beyond what's specified. Test team handles comprehensive test coverage.

⚠️ **NO BENCHMARKS**: Do not write benchmark code. Performance team handles benchmarking.

⚠️ **FOCUS ON FUNCTIONALITY**: Only modify production and test code as specified.

---

## DEPENDENCIES

**Rust Crates** (likely already in dependencies):
- chrono (for DateTime handling)
- surrealdb (database operations)
- tokio (async runtime for tests)

**Existing Code**:
- PendingUpload struct
- MediaService struct
- Test utilities (create_test_media_service)

---

## DEFINITION OF DONE

- [ ] create_expired_upload test helper method added with #[cfg(test)]
- [ ] Original test updated to use new helper
- [ ] "For now" comment removed
- [ ] Additional test cases added (multiple expired, mixed, none)
- [ ] cleanup_expired_uploads method verified/implemented
- [ ] All tests properly verify cleanup behavior
- [ ] No compilation errors
- [ ] No benchmark code written

---

## FILES TO MODIFY

1. `packages/surrealdb/src/repository/media_service.rs` (add test helper + verify cleanup method)
2. `packages/surrealdb/src/repository/media_service_test.rs` (lines 433-440 + new tests)

---

## NOTES

- Using `#[cfg(test)]` ensures test helpers don't bloat production builds
- Expired uploads should be cleaned up periodically (cron job, etc.)
- Test helpers improve test quality without compromising production code
- DateTime manipulation in tests is common pattern
- Consider adding a scheduled cleanup task in production code
- This enables proper testing of time-based logic without waiting real time
