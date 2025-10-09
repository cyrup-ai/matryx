# SPEC_MEDIA_05: Complete v1 Create and Upload Endpoints

## Status: NEEDS IMPLEMENTATION - Rating 1/10

## QA Review Summary

**Current State:** Only 1 of 3 required endpoints is correctly implemented. The database and service layers for pending uploads are completely missing (0% complete).

**What Works:**
- ✅ POST /_matrix/media/v3/upload - Multipart upload with authentication

**What's Missing/Broken:**
- ❌ POST /_matrix/media/v1/create - Stub returning hardcoded "example"
- ❌ PUT /_matrix/media/v3/upload/{serverName}/{mediaId} - Wrong implementation (accepts JSON not binary)
- ❌ MediaRepository - NO pending upload support at all
- ❌ MediaService - NO async upload methods

---

## CRITICAL: Database Layer Implementation Required

**File:** `packages/surrealdb/src/repository/media.rs`

### Add PendingUpload Support

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingUpload {
    pub media_id: String,
    pub server_name: String,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub status: PendingUploadStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PendingUploadStatus {
    Pending,
    Completed,
    Expired,
}
```

### Required Methods for MediaRepository

1. **create_pending_upload()** - Store pending upload with expiration
2. **get_pending_upload()** - Retrieve pending upload by media_id + server_name
3. **count_user_pending_uploads()** - For rate limiting (max 10 per user)
4. **mark_pending_upload_completed()** - Mark as uploaded
5. **cleanup_expired_pending_uploads()** - Delete expired entries

See spec lines 170-320 for complete implementation.

---

## CRITICAL: Service Layer Implementation Required

**File:** `packages/surrealdb/src/repository/media_service.rs`

### Required Methods for MediaService

1. **create_pending_upload()** - Create reservation with UUID, validate rate limit
2. **validate_pending_upload()** - Check expiration, ownership, not-completed
3. **upload_to_pending()** - Upload binary content to reserved media_id

See spec lines 322-450 for complete implementation with error handling.

---

## CRITICAL: Endpoint Fixes Required

### 1. POST /_matrix/media/v1/create (REWRITE COMPLETELY)

**File:** `packages/server/src/_matrix/media/v1/create.rs`

**Current Problem:** Returns hardcoded "example" instead of real media_id

**Required Changes:**
- Extract Bearer token from Authorization header
- Validate token with session_service
- Call MediaService.create_pending_upload()
- Return real mxc:// URI with UUID media_id
- Include unused_expires_at timestamp

See spec lines 452-530 for complete implementation.

### 2. PUT /_matrix/media/v3/upload/{serverName}/{mediaId} (REWRITE COMPLETELY)

**File:** `packages/server/src/_matrix/media/v3/upload/by_server_name/by_media_id.rs`

**Current Problem:** Accepts `Json(payload): Json<Value>` instead of binary body

**Critical Fix:**
```rust
// WRONG (current):
Json(payload): Json<Value>

// CORRECT (required):
body: Bytes
```

**Required Changes:**
- Accept binary body not JSON payload
- Validate pending upload exists and not expired
- Verify ownership (user who created vs user uploading)
- Prevent overwrite if already completed
- Call MediaService.upload_to_pending()
- Return empty `{}` per spec

See spec lines 532-620 for complete implementation.

---

## Error Handling Requirements

### Required Matrix Error Codes

1. **M_LIMIT_EXCEEDED (429)** - Too many pending uploads (>10)
2. **M_NOT_FOUND (404)** - Media ID expired or doesn't exist
3. **M_FORBIDDEN (403)** - Wrong user trying to upload
4. **M_CANNOT_OVERWRITE_MEDIA (409)** - Already uploaded

---

## Definition of Done

Implementation is complete when ALL of these pass:

1. ✅ MediaRepository has PendingUpload struct and 5 required methods
2. ✅ MediaService has create_pending_upload, validate_pending_upload, upload_to_pending
3. ✅ POST /_matrix/media/v1/create returns real UUID-based mxc:// URIs
4. ✅ PUT /_matrix/media/v3/upload/{serverName}/{mediaId} accepts binary body and uploads to pending
5. ✅ Rate limiting prevents >10 concurrent pending uploads per user
6. ✅ Expired media IDs return 404 M_NOT_FOUND
7. ✅ Already-uploaded media IDs return 409 M_CANNOT_OVERWRITE_MEDIA
8. ✅ Wrong user returns 403 M_FORBIDDEN
9. ✅ Server name validation prevents foreign server uploads
10. ✅ POST /_matrix/media/v3/upload continues working (no regression)

---

## Implementation Priority

1. **FIRST:** Database layer (PendingUpload struct + MediaRepository methods)
2. **SECOND:** Service layer (MediaService async upload methods)
3. **THIRD:** Rewrite POST /_matrix/media/v1/create endpoint
4. **FOURTH:** Rewrite PUT /_matrix/media/v3/upload/{serverName}/{mediaId} endpoint
5. **FIFTH:** Add proper Matrix error codes

---

## Key Technical Requirements

### Binary Body Handling in Axum
```rust
use axum::body::Bytes;

pub async fn put(
    body: Bytes,  // NOT Json<Value>
) -> Result<Json<Value>, StatusCode> {
    let file_data: &[u8] = &body;
    // Process binary content
}
```

### Pending Upload Expiration
- Default: 24 hours from creation
- Check expires_at < Utc::now() → return 404

### Rate Limiting
- Max 10 concurrent pending uploads per user
- Count with status='pending' in WHERE clause

---

## Spec References

**Matrix 1.7 Async Upload Flow:**
- POST /media/v1/create: Lines 170-230 in content-repo.yaml
- PUT /media/v3/upload/{serverName}/{mediaId}: Lines 80-170 in content-repo.yaml

**Critical Spec Quote:**
> "The endpoint permits uploading content to an mxc:// URI that was created earlier via POST /_matrix/media/v1/create. The request body is the content to be uploaded."

This confirms the PUT endpoint MUST accept binary body, not JSON.
