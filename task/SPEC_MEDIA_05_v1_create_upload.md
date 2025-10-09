# SPEC_MEDIA_05: Complete v1 Create and Upload Endpoints

## Status
Partial Implementation

## Description
The v1 media creation and upload endpoints need proper implementation. Create endpoint is a stub, and upload needs verification.

## Current State

### POST /_matrix/media/v1/create
**File**: `packages/server/src/_matrix/media/v1/create.rs`

**Current (STUB)**:
```rust
pub async fn post(Json(_payload): Json<Value>) -> Result<Json<Value>, StatusCode> {
    let config = ServerConfig::get()?;
    Ok(Json(json!({
        "content_uri": format!("mxc://{}/example", config.homeserver_name)
    })))
}
```

Returns a hardcoded "example" media ID. This is not functional.

### POST /_matrix/media/v3/upload
**File**: `packages/server/src/_matrix/media/v3/upload/mod.rs`

Appears implemented but needs verification against spec.

### PUT /_matrix/media/v3/upload/{serverName}/{mediaId}
**File**: `packages/server/src/_matrix/media/v3/upload/by_server_name/by_media_id.rs`

Appears implemented but needs verification.

## Spec Requirements

### POST /_matrix/media/v1/create (Added in v1.7)

**Purpose**: Create an `mxc://` URI without uploading content yet

**Request**: Empty JSON object or no body

**Response**:
```json
{
  "content_uri": "mxc://example.com/AQwafuaFswefuhsfAFAgsw",
  "unused_expires_at": 1647257217083
}
```

**Fields**:
- `content_uri` (string, required) - The mxc:// URI for future upload
- `unused_expires_at` (integer, optional) - Expiry timestamp in milliseconds

**Server Behavior**:
- Generate unique media ID
- Store as "pending upload" state
- Set expiration (recommended: 24 hours)
- Limit concurrent pending uploads per user
- Clean up expired pending uploads

**Rate Limiting**:
- Limit requests to create mxc:// URIs
- Return `429 M_LIMIT_EXCEEDED` if too many pending uploads

### PUT /_matrix/media/v3/upload/{serverName}/{mediaId} (Added in v1.7)

**Purpose**: Upload content to a previously created mxc:// URI

**Requirements**:
- serverName must match the one from create
- mediaId must be from a valid create call
- User must be the same who called create
- Media ID must not have expired
- Media ID must not already have content

**Request**:
- Headers: `Content-Type` (optional, default: application/octet-stream)
- Query: `filename` (optional)
- Body: Binary file data

**Response**:
```json
{}
```

Empty JSON object on success.

**Error Responses**:
- `403 M_FORBIDDEN` - Wrong user, quota exceeded, or not permitted
- `404 M_NOT_FOUND` - Invalid/expired media ID
- `409 M_CANNOT_OVERWRITE_MEDIA` - Media already uploaded
- `413 M_TOO_LARGE` - Content too large

### POST /_matrix/media/v3/upload

Standard upload endpoint - verify it matches spec:

**Request**:
- Headers: `Content-Type` (optional, default: application/octet-stream)
- Query: `filename` (optional)
- Body: Binary file data (multipart form or raw)

**Response**:
```json
{
  "content_uri": "mxc://example.com/AQwafuaFswefuhsfAFAgsw"
}
```

## Implementation Tasks

### 1. Fix POST /media/v1/create

```rust
use uuid::Uuid;
use chrono::{Utc, Duration};

pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    // Validate authentication
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

    // Generate unique media ID
    let media_id = Uuid::new_v4().to_string();
    
    // Set expiration (24 hours from now)
    let expires_at = (Utc::now() + Duration::hours(24))
        .timestamp_millis();

    // Store pending upload in database
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    media_repo
        .create_pending_upload(&media_id, &token_info.user_id, expires_at)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({
        "content_uri": format!("mxc://{}/{}", state.homeserver_name, media_id),
        "unused_expires_at": expires_at
    })))
}
```

### 2. Verify PUT /media/v3/upload/{serverName}/{mediaId}

Check implementation against spec requirements:
- Validates user matches creator
- Checks expiration
- Prevents overwriting
- Proper error codes

### 3. Verify POST /media/v3/upload

Ensure standard upload works correctly per spec.

### 4. Database Schema

Add pending uploads table/tracking:
```sql
-- Track pending media uploads
CREATE TABLE pending_media_uploads (
    media_id TEXT PRIMARY KEY,
    server_name TEXT,
    created_by TEXT,  -- user_id
    created_at TIMESTAMP,
    expires_at TIMESTAMP,
    status TEXT  -- 'pending', 'uploaded', 'expired'
);
```

### 5. Cleanup Task

Implement background task to clean up expired pending uploads.

## Testing

```bash
# Test create
curl -X POST -H "Authorization: Bearer <token>" \
  http://localhost:8008/_matrix/media/v1/create

# Response should have content_uri and unused_expires_at

# Test upload to created URI
curl -X PUT -H "Authorization: Bearer <token>" \
  -H "Content-Type: image/png" \
  --data-binary @image.png \
  "http://localhost:8008/_matrix/media/v3/upload/example.com/<media_id>"

# Test duplicate upload (should fail with 409)
curl -X PUT -H "Authorization: Bearer <token>" \
  -H "Content-Type: image/png" \
  --data-binary @image.png \
  "http://localhost:8008/_matrix/media/v3/upload/example.com/<media_id>"
```

## References
- Spec: `/tmp/matrix-spec/data/api/client-server/content-repo.yaml` lines 138-227
- Current: `packages/server/src/_matrix/media/v1/create.rs`
- Current: `packages/server/src/_matrix/media/v3/upload/by_server_name/by_media_id.rs`
