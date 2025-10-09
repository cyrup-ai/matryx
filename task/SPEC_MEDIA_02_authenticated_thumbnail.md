# SPEC_MEDIA_02: Implement Authenticated Thumbnail Endpoint

## Status
Missing Implementation - **Most Code Already Exists, Just Needs Wiring**

## Core Objective

Implement the Matrix v1.11 authenticated thumbnail endpoint at `/_matrix/client/v1/media/thumbnail/{serverName}/{mediaId}` that returns **binary image data** (not JSON) with proper authentication.

This is primarily a **wiring task** - the thumbnail generation logic, authentication infrastructure, and response helpers are already fully implemented. We just need to connect them properly.

## What Already Exists (DO NOT DUPLICATE)

### 1. Thumbnail Generation Logic - FULLY IMPLEMENTED ✅

Location: [`packages/surrealdb/src/repository/media_service.rs`](../packages/surrealdb/src/repository/media_service.rs)

The `MediaService::generate_thumbnail()` method (lines 267-329) is **completely functional**:

```rust
pub async fn generate_thumbnail(
    &self,
    media_id: &str,
    server_name: &str,
    width: u32,
    height: u32,
    method: &str,
) -> Result<ThumbnailResult, MediaError> {
    // ✅ Checks for cached thumbnail
    // ✅ Validates media exists and is an image
    // ✅ Processes thumbnail using image crate
    // ✅ Handles crop vs scale methods
    // ✅ Stores generated thumbnail
    // ✅ Returns ThumbnailResult { thumbnail: Vec<u8>, content_type, width, height }
}
```

The `process_thumbnail()` helper (lines 556-608) implements actual image processing:
- Uses `image` crate with Lanczos3 filtering
- Handles crop (resize_to_fill) vs scale (resize) methods
- Encodes output as JPEG
- Validates input parameters

### 2. Authentication Infrastructure - FULLY IMPLEMENTED ✅

Location: [`packages/server/src/auth/authenticated_user.rs`](../packages/server/src/auth/authenticated_user.rs)

The `AuthenticatedUser` extractor (lines 113-196) automatically:
- Extracts Bearer token from Authorization header
- Validates token via session_service
- Verifies user exists and is active
- Returns user_id, device_id, access_token

**Usage pattern:**
```rust
use crate::auth::authenticated_user::AuthenticatedUser;

pub async fn handler(
    user: AuthenticatedUser,  // ← Axum auto-validates
    // ... other params
) -> Result<Response, StatusCode> {
    let user_id = &user.user_id;  // Use authenticated user
    // ...
}
```

### 3. Binary Response Helpers - FULLY IMPLEMENTED ✅

Location: [`packages/server/src/utils/response_helpers.rs`](../packages/server/src/utils/response_helpers.rs)

The `media_response()` function (lines 34-54) creates proper binary responses:

```rust
pub fn media_response(
    content_type: &str,
    content_length: u64,
    filename: Option<&str>,
    body: Body,
) -> Result<Response<Body>, StatusCode> {
    // ✅ Sets Content-Type header
    // ✅ Sets Content-Disposition: inline; filename="..."
    // ✅ Sets security headers (CSP, CORS, etc.)
    // ✅ Returns Response<Body> with binary data
}
```

### 4. Query Parameter Struct - EXISTS, Needs `animated` Field

Location: [`packages/server/src/_matrix/media/v3/thumbnail/mod.rs`](../packages/server/src/_matrix/media/v3/thumbnail/mod.rs)

Current ThumbnailQuery (lines 6-17):
```rust
#[derive(Deserialize)]
pub struct ThumbnailQuery {
    pub width: u32,
    pub height: u32,
    #[serde(default = "default_method")]
    pub method: String,
    pub timeout_ms: Option<u64>,
}
```

**Needs:** Add `pub animated: Option<bool>` field per Matrix v1.11 spec.

### 5. v1 Stub Endpoint - EXISTS, Needs Implementation

Location: [`packages/server/src/_matrix/client/v1/media/thumbnail/by_server_name/by_media_id.rs`](../packages/server/src/_matrix/client/v1/media/thumbnail/by_server_name/by_media_id.rs)

Current stub (13 lines) - returns JSON (wrong):
```rust
pub async fn get(
    Path((_server_name, _media_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "content_type": "image/jpeg",
        "content_disposition": "attachment; filename=thumbnail.jpg"
    })))
}
```

### 6. v3 Endpoint Reference Implementation

Location: [`packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs`](../packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs)

Shows the pattern (lines 18-42) but returns JSON instead of binary:
```rust
pub async fn get(
    State(state): State<AppState>,
    Path((server_name, media_id)): Path<(String, String)>,
    Query(query): Query<ThumbnailQuery>,
) -> Result<Json<Value>, StatusCode> {
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
    let media_service = MediaService::new(media_repo, room_repo, membership_repo);

    let thumbnail_result = media_service
        .generate_thumbnail(&media_id, &server_name, query.width, query.height, &query.method)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // ❌ WRONG - returns JSON instead of binary
    Ok(Json(json!({
        "content_type": thumbnail_result.content_type,
        "width": thumbnail_result.width,
        "height": thumbnail_result.height
    })))
}
```

## What Needs to Change

### Change 1: Add `animated` Field to ThumbnailQuery

**File:** `packages/server/src/_matrix/media/v3/thumbnail/mod.rs`

**Action:** Add one field to existing struct:

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

### Change 2: Implement v1 Authenticated Thumbnail Endpoint

**File:** `packages/server/src/_matrix/client/v1/media/thumbnail/by_server_name/by_media_id.rs`

**Complete Implementation:**

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

// Use shared ThumbnailQuery from parent module
use super::super::super::super::super::media::v3::thumbnail::ThumbnailQuery;

/// GET /_matrix/client/v1/media/thumbnail/{serverName}/{mediaId}
/// 
/// Authenticated endpoint that returns binary thumbnail image data.
/// Requires Bearer token in Authorization header.
pub async fn get(
    user: AuthenticatedUser,  // ← Auto-validates authentication
    State(state): State<AppState>,
    Path((server_name, media_id)): Path<(String, String)>,
    Query(query): Query<ThumbnailQuery>,
) -> Result<Response<Body>, StatusCode> {
    // Validate thumbnail dimensions
    if query.width == 0 || query.height == 0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    // Reasonable maximum to prevent abuse (2048x2048 per Matrix spec guidance)
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

    // Check media access permissions using authenticated user
    let can_access = media_service
        .validate_media_access(&media_id, &server_name, &user.user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !can_access {
        return Err(StatusCode::FORBIDDEN);
    }

    // Generate thumbnail using fully-implemented MediaService method
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

    // Determine filename based on content type
    let filename = match thumbnail_result.content_type.as_str() {
        "image/png" => "thumbnail.png",
        "image/jpeg" => "thumbnail.jpg",
        "image/gif" => "thumbnail.gif",
        "image/webp" => "thumbnail.webp",
        _ => "thumbnail.png",
    };

    // Return binary image data using helper function
    media_response(
        &thumbnail_result.content_type,
        thumbnail_result.thumbnail.len() as u64,
        Some(filename),
        Body::from(thumbnail_result.thumbnail),
    )
}
```

**Key Changes Explained:**

1. **Authentication**: Added `AuthenticatedUser` parameter - Axum automatically validates Bearer token
2. **Access Control**: Call `media_service.validate_media_access()` with user_id
3. **Validation**: Check width/height bounds and method parameter
4. **Error Mapping**: Map MediaError variants to proper HTTP status codes
5. **Binary Response**: Use `media_response()` helper to return binary data with proper headers

### Change 3: Register Route (If Not Already Registered)

**File:** `packages/server/src/main.rs` (or wherever routes are registered)

**Ensure route is registered:**

```rust
.route(
    "/_matrix/client/v1/media/thumbnail/:server_name/:media_id",
    get(crate::_matrix::client::v1::media::thumbnail::by_server_name::by_media_id::get)
)
```

Check if this route already exists in the routing configuration. If not, add it.

## Matrix Spec Requirements Met

### Endpoint Specification

- ✅ **Path**: `GET /_matrix/client/v1/media/thumbnail/{serverName}/{mediaId}`
- ✅ **Authentication**: Requires Bearer token (via `AuthenticatedUser`)
- ✅ **Query Parameters**:
  - `width` (u32, required) - Validated > 0, ≤ 2048
  - `height` (u32, required) - Validated > 0, ≤ 2048
  - `method` (string, optional, default "scale") - Validated "crop" or "scale"
  - `timeout_ms` (u64, optional) - Accepted
  - `animated` (bool, optional) - Accepted (v1.11+)

### Response Headers

Via `media_response()` helper:
- ✅ `Content-Type`: image/jpeg, image/png, etc.
- ✅ `Content-Disposition`: inline; filename="thumbnail.png" (v1.12+)
- ✅ `Content-Length`: Actual byte size
- ✅ Security headers: CSP, CORS, CORP

### Thumbnail Generation Rules

Via `MediaService::generate_thumbnail()`:
- ✅ **"scale"**: Maintains aspect ratio (uses `image::resize()`)
- ✅ **"crop"**: Returns image at requested aspect ratio (uses `resize_to_fill()`)
- ✅ **No upscaling**: Validated in MediaService
- ✅ **Caching**: Thumbnails cached in database
- ✅ **Size limits**: 20MB source limit for thumbnailing

### Error Responses

Properly mapped in the implementation:
- ✅ `400 BAD_REQUEST` - Invalid dimensions or method
- ✅ `401 UNAUTHORIZED` - Missing/invalid token (handled by AuthenticatedUser)
- ✅ `403 FORBIDDEN` - User lacks access to media
- ✅ `404 NOT_FOUND` - Media doesn't exist
- ✅ `413 REQUEST_ENTITY_TOO_LARGE` - Local content too large
- ✅ `504 GATEWAY_TIMEOUT` - Content not yet uploaded

## Animation Support (v1.11)

The `animated` query parameter is now accepted. Current MediaService returns JPEG thumbnails. Future enhancement could:
- Check `query.animated` value
- Return WebP/APNG for animated content when `animated=true`
- This is optional for initial implementation

For now, the parameter is accepted and ignored (acceptable per spec - server decides format).

## Source File References

All relative to `/Volumes/samsung_t9/maxtryx/`:

1. **MediaService** (thumbnail generation logic): [`packages/surrealdb/src/repository/media_service.rs`](../packages/surrealdb/src/repository/media_service.rs)
2. **AuthenticatedUser** (authentication): [`packages/server/src/auth/authenticated_user.rs`](../packages/server/src/auth/authenticated_user.rs)
3. **Response helpers**: [`packages/server/src/utils/response_helpers.rs`](../packages/server/src/utils/response_helpers.rs)
4. **ThumbnailQuery struct**: [`packages/server/src/_matrix/media/v3/thumbnail/mod.rs`](../packages/server/src/_matrix/media/v3/thumbnail/mod.rs)
5. **v1 endpoint stub**: [`packages/server/src/_matrix/client/v1/media/thumbnail/by_server_name/by_media_id.rs`](../packages/server/src/_matrix/client/v1/media/thumbnail/by_server_name/by_media_id.rs)
6. **v3 endpoint reference**: [`packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs`](../packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs)
7. **Federation thumbnail** (multipart response example): [`packages/server/src/_matrix/federation/v1/media/thumbnail/by_media_id.rs`](../packages/server/src/_matrix/federation/v1/media/thumbnail/by_media_id.rs)

## Matrix Spec References

Spec files in `/Volumes/samsung_t9/maxtryx/tmp/matrix-spec/`:

1. **API Spec**: `data/api/client-server/authed-content-repo.yaml` (lines 148-358)
2. **Thumbnails Guide**: `content/client-server-api/modules/content_repo.md` (lines 80-126)

## Definition of Done

This task is complete when:

1. ✅ `ThumbnailQuery` has `animated: Option<bool>` field added
2. ✅ v1 authenticated thumbnail endpoint returns **binary image data** (not JSON)
3. ✅ Endpoint validates authentication using `AuthenticatedUser`
4. ✅ Endpoint validates media access permissions
5. ✅ Response includes proper headers (Content-Type, Content-Disposition)
6. ✅ Query parameters validated (width/height > 0, ≤ 2048, method in ["crop", "scale"])
7. ✅ Errors mapped to correct HTTP status codes
8. ✅ Route registered in main.rs (if not already)

**Manual Verification:**

```bash
# 1. Get access token by logging in
curl -X POST http://localhost:8008/_matrix/client/v3/login \
  -H "Content-Type: application/json" \
  -d '{"type":"m.login.password","identifier":{"type":"m.id.user","user":"test"},"password":"test"}'

# 2. Request authenticated thumbnail (replace TOKEN)
curl -H "Authorization: Bearer <TOKEN>" \
  "http://localhost:8008/_matrix/client/v1/media/thumbnail/localhost/abc123?width=64&height=64&method=scale" \
  --output thumb.png

# 3. Verify it's a valid image file
file thumb.png
# Should output: thumb.png: PNG image data... or JPEG image data...

# 4. Verify response headers
curl -I -H "Authorization: Bearer <TOKEN>" \
  "http://localhost:8008/_matrix/client/v1/media/thumbnail/localhost/abc123?width=64&height=64"
# Should show:
#   Content-Type: image/jpeg (or image/png)
#   Content-Disposition: inline; filename="thumbnail.jpg"
```

**Success criteria:** Binary image file is downloaded, not JSON text.

## Summary

This is a **small wiring task** - most complexity already exists:

- **MediaService.generate_thumbnail()** - Already implements all thumbnail logic
- **AuthenticatedUser** - Already handles authentication
- **media_response()** - Already builds binary responses
- **ThumbnailQuery** - Already exists, just needs one field

**Total code changes:** ~80 lines in one file + 1 line in ThumbnailQuery + route registration

The task primarily connects existing, tested functionality rather than implementing new features.