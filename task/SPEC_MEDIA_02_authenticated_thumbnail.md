# SPEC_MEDIA_02: Implement Authenticated Thumbnail Endpoint

## Status
**NOT IMPLEMENTED** - Current implementation is a non-functional stub returning JSON instead of binary image data.

## QA Review Rating: 2/10

### Critical Issues Found

1. **WRONG RESPONSE TYPE** - Returns `Json<Value>` instead of binary image data (`Response<Body>`)
2. **NO AUTHENTICATION** - Missing `AuthenticatedUser` parameter (security vulnerability!)
3. **NO ACCESS CONTROL** - No validation that user can access the requested media
4. **NO THUMBNAIL GENERATION** - Doesn't call `MediaService::generate_thumbnail()`
5. **MISSING QUERY PARAMETER** - `ThumbnailQuery` lacks `animated: Option<bool>` field per Matrix v1.11 spec
6. **HARDCODED DUMMY DATA** - Returns fake values, not actual thumbnails

## Required Implementation

### Task 1: Add `animated` Field to ThumbnailQuery

**File:** `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v3/thumbnail/mod.rs`

Add the missing field to the existing struct:

```rust
#[derive(Deserialize)]
pub struct ThumbnailQuery {
    pub width: u32,
    pub height: u32,
    #[serde(default = "default_method")]
    pub method: String,
    pub timeout_ms: Option<u64>,
    pub animated: Option<bool>,  // ← ADD THIS LINE
}
```

### Task 2: Implement v1 Authenticated Thumbnail Endpoint

**File:** `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v1/media/thumbnail/by_server_name/by_media_id.rs`

**Current stub (13 lines - DELETE THIS):**
```rust
use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v1/media/thumbnail/{serverName}/{mediaId}
pub async fn get(
    Path((_server_name, _media_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "content_type": "image/jpeg",
        "content_disposition": "attachment; filename=thumbnail.jpg"
    })))
}
```

**Replace with full implementation:**

```rust
use crate::AppState;
use crate::auth::authenticated_user::AuthenticatedUser;
use crate::utils::response_helpers::media_response;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Response,
};
use matryx_surrealdb::repository::{
    media::MediaRepository,
    media_service::MediaService,
    membership::MembershipRepository,
    room::RoomRepository,
};
use std::sync::Arc;

// Use shared ThumbnailQuery from v3
use super::super::super::super::super::media::v3::thumbnail::ThumbnailQuery;

/// GET /_matrix/client/v1/media/thumbnail/{serverName}/{mediaId}
/// 
/// Authenticated endpoint that returns binary thumbnail image data.
/// Requires Bearer token in Authorization header.
pub async fn get(
    user: AuthenticatedUser,  // Auto-validates Bearer token
    State(state): State<AppState>,
    Path((server_name, media_id)): Path<(String, String)>,
    Query(query): Query<ThumbnailQuery>,
) -> Result<Response<Body>, StatusCode> {
    // Validate thumbnail dimensions
    if query.width == 0 || query.height == 0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    // Matrix spec guidance: max 2048x2048 to prevent abuse
    if query.width > 2048 || query.height > 2048 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate method parameter
    let method = query.method.as_str();
    if !matches!(method, "crop" | "scale") {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Create MediaService instance
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
    let media_service = MediaService::new(media_repo, room_repo, membership_repo);

    // Validate user has access to this media
    let can_access = media_service
        .validate_media_access(&media_id, &server_name, &user.user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !can_access {
        return Err(StatusCode::FORBIDDEN);
    }

    // Generate thumbnail using MediaService
    let thumbnail_result = media_service
        .generate_thumbnail(&media_id, &server_name, query.width, query.height, method)
        .await
        .map_err(|e| {
            use matryx_surrealdb::repository::media_service::MediaError;
            match e {
                MediaError::NotFound => StatusCode::NOT_FOUND,
                MediaError::NotYetUploaded => StatusCode::GATEWAY_TIMEOUT,
                MediaError::TooLarge => StatusCode::REQUEST_ENTITY_TOO_LARGE,
                MediaError::UnsupportedFormat => StatusCode::BAD_REQUEST,
                MediaError::AccessDenied(_) => StatusCode::FORBIDDEN,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            }
        })?;

    // Determine filename from content type
    let filename = match thumbnail_result.content_type.as_str() {
        "image/png" => "thumbnail.png",
        "image/jpeg" => "thumbnail.jpg",
        "image/gif" => "thumbnail.gif",
        "image/webp" => "thumbnail.webp",
        _ => "thumbnail.png",
    };

    // Return binary image data with proper headers
    media_response(
        &thumbnail_result.content_type,
        thumbnail_result.thumbnail.len() as u64,
        Some(filename),
        Body::from(thumbnail_result.thumbnail),
    )
}
```

## Key Implementation Requirements

1. **Authentication**: MUST use `AuthenticatedUser` parameter for auto-validation
2. **Access Control**: MUST call `validate_media_access()` before returning media
3. **Binary Response**: MUST return `Response<Body>` with image bytes, NOT JSON
4. **Error Mapping**: MUST map `MediaError` to appropriate HTTP status codes
5. **Parameter Validation**: MUST validate width/height bounds and method
6. **Proper Headers**: MUST use `media_response()` helper for Content-Type, Content-Disposition, etc.

## Verification

The route is already registered at `/Volumes/samsung_t9/maxtryx/packages/server/src/main.rs:385`

Test with:
```bash
# 1. Login to get token
curl -X POST http://localhost:8008/_matrix/client/v3/login \
  -H "Content-Type: application/json" \
  -d '{"type":"m.login.password","identifier":{"type":"m.id.user","user":"test"},"password":"test"}'

# 2. Request authenticated thumbnail
curl -H "Authorization: Bearer <TOKEN>" \
  "http://localhost:8008/_matrix/client/v1/media/thumbnail/localhost/abc123?width=64&height=64&method=scale" \
  --output thumb.png

# 3. Verify it's a real image
file thumb.png
# Should output: thumb.png: PNG image data... or JPEG image data...

# 4. Verify WITHOUT auth returns 401
curl "http://localhost:8008/_matrix/client/v1/media/thumbnail/localhost/abc123?width=64&height=64"
# Should return 401 Unauthorized
```

## Definition of Done

- [ ] `ThumbnailQuery` has `animated: Option<bool>` field
- [ ] Endpoint uses `AuthenticatedUser` for authentication
- [ ] Endpoint validates media access with `validate_media_access()`
- [ ] Endpoint calls `MediaService::generate_thumbnail()`
- [ ] Returns binary image data (`Response<Body>`) NOT JSON
- [ ] Proper HTTP headers (Content-Type, Content-Disposition)
- [ ] Query parameters validated (bounds, method)
- [ ] Errors mapped to correct status codes (400, 401, 403, 404, 413, 504)
- [ ] Manual testing confirms binary image download works with auth
- [ ] Manual testing confirms 401 without auth token
