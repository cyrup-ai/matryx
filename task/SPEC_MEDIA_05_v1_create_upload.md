# SPEC_MEDIA_05: Complete v1 Create and Upload Endpoints

## Status
Ready for Implementation

## Core Objective
Implement the Matrix 1.7 asynchronous media upload flow by completing the v1 create endpoint and fixing the v3 upload-to-MXC endpoint. This enables clients to reserve media IDs before uploading content, supporting use cases like slow connections and resumable uploads.

## Scope

This task implements three interconnected Matrix media endpoints:

1. **POST /_matrix/media/v1/create** - Create mxc:// URI without content (INCOMPLETE - currently stub)
2. **PUT /_matrix/media/v3/upload/{serverName}/{mediaId}** - Upload to reserved URI (INCORRECT - needs rewrite)
3. **POST /_matrix/media/v3/upload** - Standard immediate upload (VERIFY ONLY - appears complete)

## What This Task Does NOT Include

- No unit tests, functional tests, or integration tests
- No benchmarking or performance testing
- No extensive documentation generation
- No changes to download or thumbnail endpoints
- No federation media handling changes

## Matrix Specification Reference

**Primary Spec Files:**
- [`/tmp/matrix-spec/data/api/client-server/content-repo.yaml`](../tmp/matrix-spec/data/api/client-server/content-repo.yaml) - Lines 1-230
  - POST /media/v1/create (lines 170-230)
  - PUT /media/v3/upload/{serverName}/{mediaId} (lines 80-170)
  - POST /media/v3/upload (lines 18-80)

## Current Implementation Analysis

### Existing Media Infrastructure

**MediaRepository** ([`packages/surrealdb/src/repository/media.rs`](../packages/surrealdb/src/repository/media.rs))
- ✅ Full media_info table support (store/get)
- ✅ media_content storage with binary data
- ✅ Thumbnail generation and storage
- ✅ Content deduplication via SHA256 hashing
- ✅ User storage quota tracking
- ✅ Media-room associations
- ✅ Quarantine support
- ❌ NO pending upload tracking
- ❌ NO expiration-based cleanup for pending uploads

**MediaService** ([`packages/surrealdb/src/repository/media_service.rs`](../packages/surrealdb/src/repository/media_service.rs))
- ✅ upload_media() - creates media_id, stores content
- ✅ download_media() - retrieves with access control
- ✅ validate_media_upload() - quota/type checking
- ✅ validate_media_access() - room membership checks
- ❌ NO create_pending_upload()
- ❌ NO validate_pending_upload()
- ❌ NO consume_pending_upload()

**AppState** ([`packages/server/src/state.rs`](../packages/server/src/state.rs))
- ✅ db: Surreal&lt;Any&gt; available
- ✅ session_service for authentication
- ✅ homeserver_name for mxc:// URI generation

### Current Endpoint Status

#### 1. POST /_matrix/media/v1/create
**File:** [`packages/server/src/_matrix/media/v1/create.rs`](../packages/server/src/_matrix/media/v1/create.rs)

**Current Implementation:**
```rust
pub async fn post(Json(_payload): Json<Value>) -> Result<Json<Value>, StatusCode> {
    let config = ServerConfig::get()?;
    Ok(Json(json!({
        "content_uri": format!("mxc://{}/example", config.homeserver_name)
    })))
}
```

**Problems:**
- Returns hardcoded "example" media ID
- No database persistence
- No expiration tracking
- No authentication
- No rate limiting

#### 2. PUT /_matrix/media/v3/upload/{serverName}/{mediaId}
**File:** [`packages/server/src/_matrix/media/v3/upload/by_server_name/by_media_id.rs`](../packages/server/src/_matrix/media/v3/upload/by_server_name/by_media_id.rs)

**Current Implementation:**
```rust
pub async fn put(
    State(state): State<AppState>,
    Path((server_name, media_id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(payload): Json<Value>,  // ❌ WRONG - should be binary body
) -> Result<Json<Value>, StatusCode>
```

**Problems:**
- Accepts JSON payload instead of binary file data (violates spec)
- Updates metadata instead of uploading content
- Doesn't validate pending upload state
- Doesn't check media ID expiration
- Doesn't prevent overwrite

#### 3. POST /_matrix/media/v3/upload
**File:** [`packages/server/src/_matrix/media/v3/upload/mod.rs`](../packages/server/src/_matrix/media/v3/upload/mod.rs)

**Status:** ✅ Appears correctly implemented
- Handles multipart upload
- Authenticates user
- Uses MediaService.upload_media()
- Returns content_uri

## Implementation Requirements

### Phase 1: Database Layer

#### Add Pending Upload Support to MediaRepository

**File:** `packages/surrealdb/src/repository/media.rs`

**Add struct:**
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PendingUploadStatus {
    Pending,
    Completed,
    Expired,
}
```

**Add methods to MediaRepository:**

```rust
/// Create a pending media upload reservation
pub async fn create_pending_upload(
    &self,
    media_id: &str,
    server_name: &str,
    user_id: &str,
    expires_at: DateTime<Utc>,
) -> Result<(), RepositoryError> {
    let query = "
        CREATE pending_media_uploads SET
            media_id = $media_id,
            server_name = $server_name,
            created_by = $user_id,
            created_at = time::now(),
            expires_at = $expires_at,
            status = 'pending'
    ";
    
    self.db
        .query(query)
        .bind(("media_id", media_id.to_string()))
        .bind(("server_name", server_name.to_string()))
        .bind(("user_id", user_id.to_string()))
        .bind(("expires_at", expires_at))
        .await?;
    
    Ok(())
}

/// Get pending upload info
pub async fn get_pending_upload(
    &self,
    media_id: &str,
    server_name: &str,
) -> Result<Option<PendingUpload>, RepositoryError> {
    let query = "
        SELECT * FROM pending_media_uploads
        WHERE media_id = $media_id AND server_name = $server_name
        LIMIT 1
    ";
    
    let mut result = self.db
        .query(query)
        .bind(("media_id", media_id.to_string()))
        .bind(("server_name", server_name.to_string()))
        .await?;
    
    let pending: Vec<PendingUpload> = result.take(0)?;
    Ok(pending.into_iter().next())
}

/// Count pending uploads for a user
pub async fn count_user_pending_uploads(
    &self,
    user_id: &str,
) -> Result<u64, RepositoryError> {
    let query = "
        SELECT VALUE count() FROM pending_media_uploads
        WHERE created_by = $user_id AND status = 'pending'
        GROUP ALL
    ";
    
    let mut result = self.db
        .query(query)
        .bind(("user_id", user_id.to_string()))
        .await?;
    
    let counts: Vec<Option<u64>> = result.take(0)?;
    Ok(counts.first().and_then(|v| *v).unwrap_or(0))
}

/// Mark pending upload as completed
pub async fn mark_pending_upload_completed(
    &self,
    media_id: &str,
    server_name: &str,
) -> Result<(), RepositoryError> {
    let query = "
        UPDATE pending_media_uploads SET
            status = 'completed'
        WHERE media_id = $media_id AND server_name = $server_name
    ";
    
    self.db
        .query(query)
        .bind(("media_id", media_id.to_string()))
        .bind(("server_name", server_name.to_string()))
        .await?;
    
    Ok(())
}

/// Cleanup expired pending uploads
pub async fn cleanup_expired_pending_uploads(
    &self,
) -> Result<u64, RepositoryError> {
    let query = "
        DELETE pending_media_uploads
        WHERE status = 'pending' AND expires_at < time::now()
        RETURN BEFORE
    ";
    
    let mut result = self.db.query(query).await?;
    let deleted: Vec<PendingUpload> = result.take(0)?;
    
    Ok(deleted.len() as u64)
}
```

#### Add Service-Level Methods to MediaService

**File:** `packages/surrealdb/src/repository/media_service.rs`

**Add to MediaService impl:**

```rust
/// Create a pending media upload with expiration
pub async fn create_pending_upload(
    &self,
    user_id: &str,
    expires_in_hours: i64,
) -> Result<(String, DateTime<Utc>), MediaError> {
    // Rate limiting - check pending upload count
    const MAX_PENDING_UPLOADS: u64 = 10;
    let pending_count = self.media_repo
        .count_user_pending_uploads(user_id)
        .await
        .map_err(MediaError::from)?;
    
    if pending_count >= MAX_PENDING_UPLOADS {
        return Err(MediaError::AccessDenied(
            "Too many pending uploads".to_string()
        ));
    }
    
    // Generate media ID
    let media_id = Uuid::new_v4().to_string();
    
    // Extract server name from user_id
    let server_name = user_id
        .split(':')
        .nth(1)
        .unwrap_or(&self.homeserver_name);
    
    // Calculate expiration (default 24 hours)
    let expires_at = Utc::now() + chrono::Duration::hours(expires_in_hours);
    
    // Store pending upload
    self.media_repo
        .create_pending_upload(&media_id, server_name, user_id, expires_at)
        .await
        .map_err(MediaError::from)?;
    
    Ok((media_id, expires_at))
}

/// Validate and consume a pending upload
pub async fn validate_pending_upload(
    &self,
    media_id: &str,
    server_name: &str,
    user_id: &str,
) -> Result<(), MediaError> {
    // Get pending upload
    let pending = self.media_repo
        .get_pending_upload(media_id, server_name)
        .await
        .map_err(MediaError::from)?
        .ok_or(MediaError::NotFound)?;
    
    // Check if already completed (prevent overwrite)
    if pending.status == PendingUploadStatus::Completed {
        return Err(MediaError::InvalidOperation(
            "Media already uploaded".to_string()
        ));
    }
    
    // Check expiration
    if Utc::now() > pending.expires_at {
        return Err(MediaError::NotFound);
    }
    
    // Check ownership
    if pending.created_by != user_id {
        return Err(MediaError::AccessDenied(
            "User does not own this media ID".to_string()
        ));
    }
    
    Ok(())
}

/// Upload content to a pending media ID
pub async fn upload_to_pending(
    &self,
    media_id: &str,
    server_name: &str,
    user_id: &str,
    content: &[u8],
    content_type: &str,
    filename: Option<&str>,
) -> Result<(), MediaError> {
    // Validate pending upload
    self.validate_pending_upload(media_id, server_name, user_id).await?;
    
    // Validate content
    self.validate_media_upload(user_id, content_type, content.len() as u64)
        .await
        .map_err(MediaError::from)?;
    
    // Store media content
    self.media_repo
        .store_media_content(media_id, server_name, content, content_type)
        .await
        .map_err(MediaError::from)?;
    
    // Store media info
    let media_info = MediaInfo {
        media_id: media_id.to_string(),
        server_name: server_name.to_string(),
        content_type: content_type.to_string(),
        content_length: content.len() as u64,
        upload_name: filename.map(|s| s.to_string()),
        uploaded_by: user_id.to_string(),
        created_at: Utc::now(),
        expires_at: None,
        quarantined: None,
        quarantined_by: None,
        quarantine_reason: None,
        quarantined_at: None,
    };
    
    self.media_repo
        .store_media_info(media_id, server_name, &media_info)
        .await
        .map_err(MediaError::from)?;
    
    // Mark as completed
    self.media_repo
        .mark_pending_upload_completed(media_id, server_name)
        .await
        .map_err(MediaError::from)?;
    
    Ok(())
}
```

**Add to use statements:**
```rust
use uuid::Uuid;
```

**Add PendingUploadStatus to imports in media.rs:**
```rust
pub use media::{MediaInfo, MediaRepository, PendingUpload, PendingUploadStatus};
```

### Phase 2: Endpoint Implementation

#### 1. Rewrite POST /_matrix/media/v1/create

**File:** `packages/server/src/_matrix/media/v1/create.rs`

**Complete replacement:**

```rust
//! POST /_matrix/media/v1/create endpoint
//!
//! Creates an mxc:// URI without uploading content (Matrix 1.7)
//! Content is uploaded later via PUT /_matrix/media/v3/upload/{serverName}/{mediaId}

use crate::AppState;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Json,
};
use matryx_surrealdb::repository::{
    media::MediaRepository,
    media_service::MediaService,
    membership::MembershipRepository,
    room::RoomRepository,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Serialize, Deserialize)]
pub struct CreateMediaResponse {
    pub content_uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unused_expires_at: Option<i64>,
}

/// POST /_matrix/media/v1/create
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<CreateMediaResponse>, StatusCode> {
    // Extract and validate access token
    let access_token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate token and get user context
    let token_info = state
        .session_service
        .validate_access_token(access_token)
        .await
        .map_err(|e| {
            tracing::warn!("Token validation failed: {:?}", e);
            StatusCode::UNAUTHORIZED
        })?;

    // Create MediaService
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));

    let media_service = MediaService::new(media_repo, room_repo, membership_repo)
        .with_federation_client(
            state.federation_media_client.clone(),
            state.homeserver_name.clone(),
        );

    // Create pending upload (24 hour expiration)
    let (media_id, expires_at) = media_service
        .create_pending_upload(&token_info.user_id, 24)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create pending upload: {:?}", e);
            match e {
                matryx_surrealdb::repository::media_service::MediaError::AccessDenied(_) => {
                    StatusCode::from_u16(429).unwrap() // M_LIMIT_EXCEEDED
                },
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            }
        })?;

    Ok(Json(CreateMediaResponse {
        content_uri: format!("mxc://{}/{}", state.homeserver_name, media_id),
        unused_expires_at: Some(expires_at.timestamp_millis()),
    }))
}
```

#### 2. Rewrite PUT /_matrix/media/v3/upload/{serverName}/{mediaId}

**File:** `packages/server/src/_matrix/media/v3/upload/by_server_name/by_media_id.rs`

**Complete replacement:**

```rust
//! PUT /_matrix/media/v3/upload/{serverName}/{mediaId}
//!
//! Upload content to a previously created mxc:// URI

use crate::AppState;
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use matryx_surrealdb::repository::{
    media::MediaRepository,
    media_service::MediaService,
    membership::MembershipRepository,
    room::RoomRepository,
};
use serde_json::{json, Value};
use std::sync::Arc;

/// PUT /_matrix/media/v3/upload/{serverName}/{mediaId}
pub async fn put(
    State(state): State<AppState>,
    Path((server_name, media_id)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, StatusCode> {
    // Extract and validate access token
    let access_token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate token and get user context
    let token_info = state
        .session_service
        .validate_access_token(access_token)
        .await
        .map_err(|e| {
            tracing::warn!("Token validation failed: {:?}", e);
            StatusCode::UNAUTHORIZED
        })?;

    // Validate server name matches
    if server_name != state.homeserver_name {
        tracing::warn!(
            "Server name mismatch: {} != {}",
            server_name,
            state.homeserver_name
        );
        return Err(StatusCode::NOT_FOUND);
    }

    // Get content type from header
    let content_type = headers
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("application/octet-stream");

    // Get filename from query parameter (Axum extracts this from URL)
    let filename = headers
        .get("x-upload-filename")
        .and_then(|h| h.to_str().ok());

    // Create MediaService
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));

    let media_service = MediaService::new(media_repo, room_repo, membership_repo)
        .with_federation_client(
            state.federation_media_client.clone(),
            state.homeserver_name.clone(),
        );

    // Upload to pending media ID
    media_service
        .upload_to_pending(
            &media_id,
            &server_name,
            &token_info.user_id,
            &body,
            content_type,
            filename,
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to upload to pending media: {:?}", e);
            use matryx_surrealdb::repository::media_service::MediaError;
            match e {
                MediaError::NotFound => StatusCode::NOT_FOUND,
                MediaError::AccessDenied(_) => StatusCode::FORBIDDEN,
                MediaError::InvalidOperation(msg) if msg.contains("already uploaded") => {
                    StatusCode::CONFLICT // M_CANNOT_OVERWRITE_MEDIA
                },
                MediaError::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            }
        })?;

    // Return empty JSON object per spec
    Ok(Json(json!({})))
}
```

**Note:** The filename parameter handling requires adding query parameter extraction. Update the function signature if needed:

```rust
use axum::extract::Query;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct UploadQuery {
    filename: Option<String>,
}

// Then in function:
// Query(query): Query<UploadQuery>
// And use: query.filename.as_deref()
```

#### 3. Verify POST /_matrix/media/v3/upload

**File:** `packages/server/src/_matrix/media/v3/upload/mod.rs`

**Action:** Code review only - implementation appears correct

**Verification checklist:**
- ✅ Accepts multipart form data
- ✅ Authenticates with Bearer token
- ✅ Extracts content-type from headers
- ✅ Extracts filename from multipart field
- ✅ Calls MediaService.upload_media()
- ✅ Returns content_uri in response

**No changes needed unless verification reveals issues.**

### Phase 3: Router Registration

Verify endpoints are registered in the router:

**File:** `packages/server/src/_matrix/media/v1/mod.rs`

```rust
pub mod create;

use axum::{routing::post, Router};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/create", post(create::post))
}
```

**File:** `packages/server/src/_matrix/media/v3/upload/by_server_name/mod.rs`

```rust
pub mod by_media_id;

use axum::{routing::put, Router};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/:media_id", put(by_media_id::put))
}
```

### Phase 4: Error Response Types

Ensure proper Matrix error codes are returned:

**Add to error handling:**

```rust
// For M_LIMIT_EXCEEDED (429)
#[derive(Serialize)]
struct ErrorResponse {
    errcode: String,
    error: String,
}

// Usage in create endpoint when rate limited:
return Err((
    StatusCode::TOO_MANY_REQUESTS,
    Json(ErrorResponse {
        errcode: "M_LIMIT_EXCEEDED".to_string(),
        error: "Too many concurrent pending uploads".to_string(),
    })
));

// For M_CANNOT_OVERWRITE_MEDIA (409)
return Err((
    StatusCode::CONFLICT,
    Json(ErrorResponse {
        errcode: "M_CANNOT_OVERWRITE_MEDIA".to_string(),
        error: "Media already uploaded".to_string(),
    })
));
```

**Note:** This requires changing return types to use tuple responses. Alternatively, create a custom error type that implements IntoResponse.

## Database Schema Requirements

**Table:** `pending_media_uploads`

```sql
-- SurrealDB table definition (created automatically via queries)
-- Fields:
media_id: string         -- UUID v4 generated media identifier
server_name: string      -- Homeserver name from user_id
created_by: string       -- User ID who created the reservation
created_at: datetime     -- When the reservation was created
expires_at: datetime     -- When the reservation expires (24h default)
status: string           -- 'pending', 'completed', or 'expired'
```

**Indexes recommended:**
```sql
DEFINE INDEX pending_media_lookup ON pending_media_uploads FIELDS media_id, server_name;
DEFINE INDEX pending_user_count ON pending_media_uploads FIELDS created_by, status;
DEFINE INDEX pending_expiration ON pending_media_uploads FIELDS expires_at, status;
```

## Definition of Done

Implementation is complete when:

1. **POST /_matrix/media/v1/create** returns valid mxc:// URIs with expiration timestamps
2. **PUT /_matrix/media/v3/upload/{serverName}/{mediaId}** accepts binary file uploads to previously created URIs
3. Pending upload records are created in the database with 24-hour expiration
4. User can create max 10 concurrent pending uploads (rate limiting works)
5. Attempting to upload to expired/invalid media IDs returns 404 M_NOT_FOUND
6. Attempting to upload to already-completed media IDs returns 409 M_CANNOT_OVERWRITE_MEDIA  
7. Attempting to upload with wrong user returns 403 M_FORBIDDEN
8. Server name validation prevents uploading to foreign servers
9. Content type and quota validation works correctly
10. POST /_matrix/media/v3/upload continues to work as before

## Implementation Notes

### Authentication Pattern
All endpoints use the same authentication pattern:
```rust
let access_token = headers
    .get("authorization")
    .and_then(|h| h.to_str().ok())
    .and_then(|h| h.strip_prefix("Bearer "))
    .ok_or(StatusCode::UNAUTHORIZED)?;

let token_info = state
    .session_service
    .validate_access_token(access_token)
    .await
    .map_err(|_| StatusCode::UNAUTHORIZED)?;
```

### MediaService Instantiation Pattern
```rust
let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));

let media_service = MediaService::new(media_repo, room_repo, membership_repo)
    .with_federation_client(
        state.federation_media_client.clone(),
        state.homeserver_name.clone(),
    );
```

### Binary Body Handling in Axum
```rust
use axum::body::Bytes;

pub async fn handler(body: Bytes) -> Result<Json<Value>, StatusCode> {
    // body is Vec<u8> equivalent
    let file_data: &[u8] = &body;
    // Process binary data
}
```

### Query Parameter Extraction
```rust
use axum::extract::Query;
use serde::Deserialize;

#[derive(Deserialize)]
struct UploadParams {
    filename: Option<String>,
}

pub async fn handler(Query(params): Query<UploadParams>) {
    let filename = params.filename.as_deref();
}
```

## Cleanup Task (Optional Future Work)

A background task should periodically clean up expired pending uploads:

```rust
// Add to server startup
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(3600)); // 1 hour
    loop {
        interval.tick().await;
        let media_repo = MediaRepository::new(db.clone());
        match media_repo.cleanup_expired_pending_uploads().await {
            Ok(count) => tracing::info!("Cleaned up {} expired pending uploads", count),
            Err(e) => tracing::error!("Failed to cleanup expired uploads: {}", e),
        }
    }
});
```

**This is not required for this task** but should be noted for future implementation.

## Key Differences from Original Stub

**Before (Stub):**
- Hardcoded "example" media ID
- No database interaction
- No authentication
- No expiration

**After (Full Implementation):**
- UUID-based media IDs
- Database-persisted pending uploads
- Full authentication and authorization
- 24-hour expiration with cleanup
- Rate limiting (10 concurrent pending uploads per user)
- Proper Matrix error codes

## Related Files

**Database Layer:**
- [`packages/surrealdb/src/repository/media.rs`](../packages/surrealdb/src/repository/media.rs) - MediaRepository with pending upload methods
- [`packages/surrealdb/src/repository/media_service.rs`](../packages/surrealdb/src/repository/media_service.rs) - MediaService with async upload logic
- [`packages/surrealdb/src/repository/error.rs`](../packages/surrealdb/src/repository/error.rs) - Error types

**Server Endpoints:**
- [`packages/server/src/_matrix/media/v1/create.rs`](../packages/server/src/_matrix/media/v1/create.rs) - Create endpoint
- [`packages/server/src/_matrix/media/v3/upload/by_server_name/by_media_id.rs`](../packages/server/src/_matrix/media/v3/upload/by_server_name/by_media_id.rs) - PUT upload endpoint
- [`packages/server/src/_matrix/media/v3/upload/mod.rs`](../packages/server/src/_matrix/media/v3/upload/mod.rs) - POST upload endpoint

**Infrastructure:**
- [`packages/server/src/state.rs`](../packages/server/src/state.rs) - AppState with dependencies

## Spec Citations

**Matrix Spec - Async Upload Flow:**
> "Creates a new mxc:// URI, independently of the content being uploaded. The content must be provided later via PUT /_matrix/media/v3/upload/{serverName}/{mediaId}"
> 
> "The server may optionally enforce a maximum age for unused IDs... The recommended default expiration is 24 hours"
>
> "As well as limiting the rate of requests to create mxc:// URIs, the server should limit the number of concurrent pending media uploads a given user can have"

Source: [`/tmp/matrix-spec/data/api/client-server/content-repo.yaml`](../tmp/matrix-spec/data/api/client-server/content-repo.yaml) lines 170-230

**PUT Endpoint Requirements:**
> "This endpoint permits uploading content to an mxc:// URI that was created earlier via POST /_matrix/media/v1/create"
>
> Error cases:
> - "The request comes from a different user than the one that called POST /_matrix/media/v1/create" → 403 M_FORBIDDEN
> - "The MXC ID has expired" → 404 M_NOT_FOUND  
> - "The endpoint was called with a media ID that already has content" → 409 M_CANNOT_OVERWRITE_MEDIA

Source: [`/tmp/matrix-spec/data/api/client-server/content-repo.yaml`](../tmp/matrix-spec/data/api/client-server/content-repo.yaml) lines 80-170
