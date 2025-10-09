# SPEC_MEDIA_04: Fix v3 Thumbnail to Return Binary Data

## Status
**BUG** - Incorrect Response Format

## Priority
Medium - Endpoint exists but returns wrong response type

## Description
The v3 thumbnail endpoint currently returns JSON metadata instead of binary image data, violating the Matrix specification. The endpoint must return the actual thumbnail image bytes with proper HTTP headers.

---

## Current Implementation Analysis

### Affected File
**Primary**: [`packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs`](../../packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs)

### Current Code (Lines 36-41)
```rust
Ok(Json(json!({
    "content_type": thumbnail_result.content_type,
    "width": thumbnail_result.width,
    "height": thumbnail_result.height
})))
```

**Problem**: Returns JSON with metadata fields instead of binary image data.

### What's Already Working ✅

1. **MediaService is FULLY IMPLEMENTED** - [`packages/surrealdb/src/repository/media_service.rs`](../../packages/surrealdb/src/repository/media_service.rs)
   - Real thumbnail generation using the `image` crate (lines 253-315)
   - Supports both `crop` and `scale` resize methods (lines 565-634)
   - Returns `ThumbnailResult` with actual binary data in `thumbnail: Vec<u8>` field (line 68)
   - Caches generated thumbnails in database
   - Handles JPEG encoding with Lanczos3 filtering for quality

2. **Dependencies are in place** - [`packages/server/Cargo.toml`](../../packages/server/Cargo.toml)
   ```toml
   image = { version = "0.25.8", features = ["png", "jpeg", "gif", "webp"] }
   ```

3. **Response helpers exist** - [`packages/server/src/utils/response_helpers.rs`](../../packages/server/src/utils/response_helpers.rs)
   - `media_response()` function (lines 27-54) creates binary responses with security headers
   - Handles Content-Type, Content-Disposition, Content-Security-Policy, CORS

4. **Query parameters defined** - [`packages/server/src/_matrix/media/v3/thumbnail/mod.rs`](../../packages/server/src/_matrix/media/v3/thumbnail/mod.rs)
   - `ThumbnailQuery` struct with width, height, method, timeout_ms (lines 5-19)

---

## Matrix Specification Requirements

### Endpoint
`GET /_matrix/media/v3/thumbnail/{serverName}/{mediaId}`

**Status**: Deprecated in v1.11, superseded by v1 authenticated endpoint (but must work until v1.12 freeze)

### Specification Reference
Source: [`tmp/matrix-spec/data/api/client-server/content-repo.yaml`](../../tmp/matrix-spec/data/api/client-server/content-repo.yaml) lines 383-460

### Query Parameters
- `width` (required, integer) - Desired thumbnail width
- `height` (required, integer) - Desired thumbnail height  
- `method` (optional, enum) - Resize method: `"crop"` or `"scale"` (default: `"scale"`)
- `timeout_ms` (optional, integer) - Max wait time in milliseconds (default: 20000, max: 120000)
- `allow_remote` (optional, boolean) - Allow fetching from remote servers (default: true)
- `allow_redirect` (optional, boolean) - Allow 307/308 redirects (default: false)
- `animated` (optional, boolean) - Prefer animated thumbnails if available (v1.11+)

### Response Requirements

#### Success (200 OK)
**MUST return binary image data**, NOT JSON

**Required Headers**:
1. `Content-Type` (REQUIRED as of v1.12)
   - Valid values: `image/jpeg`, `image/png`, `image/apng`, `image/gif`, `image/webp`
   
2. `Content-Disposition` (REQUIRED as of v1.12)
   - MUST be `inline`
   - SHOULD include filename: `inline; filename="thumbnail.png"`
   - See [MDN Content-Disposition](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Disposition)

**Body**: Raw image bytes (JPEG, PNG, WebP, GIF, or APNG)

#### Resize Methods
From [`tmp/matrix-spec/data/api/client-server/content-repo.yaml`](../../tmp/matrix-spec/data/api/client-server/content-repo.yaml) lines 419-421:

- **`scale`**: Maintain aspect ratio, fit within requested dimensions
- **`crop`**: Crop to exact aspect ratio of requested dimensions

#### Error Responses
- `400 M_UNKNOWN` - Invalid dimensions (non-integer, negative)
- `413 M_TOO_LARGE` - Local content too large to thumbnail
- `429` - Rate limited
- `502 M_TOO_LARGE` - Remote content too large to thumbnail  
- `504 M_NOT_YET_UPLOADED` - Content not available (timeout)

---

## Implementation Steps

### STEP 1: Change Return Type and Imports

**File**: `packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs`

**Current imports** (lines 1-14):
```rust
use crate::AppState;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
```

**Change to**:
```rust
use crate::AppState;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::Response,
};
```

**Remove**: `Json` import
**Add**: `body::Body`, `header` from `axum::http`, and `Response`

### STEP 2: Update Function Signature

**Current** (line 18):
```rust
pub async fn get(
    State(state): State<AppState>,
    Path((server_name, media_id)): Path<(String, String)>,
    Query(query): Query<ThumbnailQuery>,
) -> Result<Json<Value>, StatusCode> {
```

**Change to**:
```rust
pub async fn get(
    State(state): State<AppState>,
    Path((server_name, media_id)): Path<(String, String)>,
    Query(query): Query<ThumbnailQuery>,
) -> Result<Response<Body>, StatusCode> {
```

**Also remove**: `use serde_json::{Value, json};` import (line 12) - no longer needed

### STEP 3: Replace Response Construction

**Current** (lines 36-41):
```rust
Ok(Json(json!({
    "content_type": thumbnail_result.content_type,
    "width": thumbnail_result.width,
    "height": thumbnail_result.height
})))
```

**Replace with**:
```rust
// Return binary thumbnail data per Matrix spec
let body = Body::from(thumbnail_result.thumbnail); // Use the Vec<u8> field

Response::builder()
    .status(StatusCode::OK)
    .header(header::CONTENT_TYPE, thumbnail_result.content_type)
    .header(header::CONTENT_DISPOSITION, "inline; filename=\"thumbnail.png\"")
    .header("Cross-Origin-Resource-Policy", "cross-origin")
    .header("Access-Control-Allow-Origin", "*")
    .body(body)
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
```

**Key Changes**:
- Use `thumbnail_result.thumbnail` (the `Vec<u8>` field) instead of metadata
- Build `Response<Body>` with binary data
- Add required `Content-Type` header from `thumbnail_result.content_type`
- Add required `Content-Disposition: inline; filename="thumbnail.png"` header
- Include CORS headers for client compatibility

---

## Complete Fixed Implementation

**File**: `packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs`

```rust
use crate::AppState;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::Response,
};
use matryx_surrealdb::repository::{
    media::MediaRepository, media_service::MediaService, membership::MembershipRepository,
    room::RoomRepository,
};
use std::sync::Arc;

// Use shared ThumbnailQuery from parent module
use super::super::ThumbnailQuery;

/// GET /_matrix/media/v3/thumbnail/{serverName}/{mediaId}
/// 
/// Returns a thumbnail of the requested media as binary image data.
/// Deprecated in Matrix v1.11 - superseded by authenticated v1 endpoint.
pub async fn get(
    State(state): State<AppState>,
    Path((server_name, media_id)): Path<(String, String)>,
    Query(query): Query<ThumbnailQuery>,
) -> Result<Response<Body>, StatusCode> {
    // Create MediaService instance
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));

    let media_service = MediaService::new(media_repo, room_repo, membership_repo);

    // Generate thumbnail using MediaService
    // This performs actual image processing: decode -> resize (crop/scale) -> encode
    let thumbnail_result = media_service
        .generate_thumbnail(&media_id, &server_name, query.width, query.height, &query.method)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Return binary thumbnail data per Matrix spec (v1.12)
    let body = Body::from(thumbnail_result.thumbnail); // Vec<u8> of encoded image

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, thumbnail_result.content_type) // e.g., "image/jpeg"
        .header(header::CONTENT_DISPOSITION, "inline; filename=\"thumbnail.png\"")
        .header("Cross-Origin-Resource-Policy", "cross-origin")
        .header("Access-Control-Allow-Origin", "*")
        .body(body)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
```

---

## Alternative: Using Existing Helper Function

You can also leverage the existing `media_response()` helper:

**Add import**:
```rust
use crate::utils::response_helpers::media_response;
```

**Replace response construction** (lines 36-end):
```rust
media_response(
    &thumbnail_result.content_type,
    thumbnail_result.thumbnail.len() as u64,
    Some("thumbnail.png"),
    Body::from(thumbnail_result.thumbnail)
)
```

This helper (defined in [`packages/server/src/utils/response_helpers.rs:27-54`](../../packages/server/src/utils/response_helpers.rs)) automatically adds:
- Content-Security-Policy headers
- CORS headers
- Content-Length header
- Content-Disposition with filename

---

## Related Files Reference

### Source Files
- **Endpoint Handler**: [`packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs`](../../packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs)
- **MediaService**: [`packages/surrealdb/src/repository/media_service.rs`](../../packages/surrealdb/src/repository/media_service.rs)
  - `ThumbnailResult` struct (line 68)
  - `generate_thumbnail()` method (lines 253-315)
  - `process_thumbnail()` image processing (lines 565-634)
- **Response Helpers**: [`packages/server/src/utils/response_helpers.rs`](../../packages/server/src/utils/response_helpers.rs)
  - `media_response()` function (lines 27-54)
- **Query Types**: [`packages/server/src/_matrix/media/v3/thumbnail/mod.rs`](../../packages/server/src/_matrix/media/v3/thumbnail/mod.rs)
  - `ThumbnailQuery` struct (lines 8-15)

### Specification Files
- **OpenAPI Spec**: [`tmp/matrix-spec/data/api/client-server/content-repo.yaml`](../../tmp/matrix-spec/data/api/client-server/content-repo.yaml)
  - Thumbnail endpoint definition: lines 383-460
  - Query parameters: lines 391-444
  - Response format: lines 445-460

---

## Definition of Done

The task is complete when:

1. ✅ The endpoint returns binary image data (not JSON)
2. ✅ `Content-Type` header is set to the image MIME type (e.g., `image/jpeg`)
3. ✅ `Content-Disposition: inline; filename="thumbnail.png"` header is present
4. ✅ CORS headers are included for client compatibility
5. ✅ The response body contains the actual thumbnail bytes from `thumbnail_result.thumbnail`
6. ✅ The function signature returns `Result<Response<Body>, StatusCode>`
7. ✅ A client request like `curl http://localhost:8008/_matrix/media/v3/thumbnail/localhost/abc123?width=64&height=64 --output test.png` produces a valid image file

**Verification**: The downloaded file should be a valid image that opens correctly, not a JSON file with metadata.

---

## Notes

- **No MediaService changes needed** - Thumbnail generation is already fully implemented with real image processing
- **No new dependencies needed** - The `image` crate is already in Cargo.toml with all required features
- **No database changes needed** - Thumbnail caching is already implemented in MediaRepository
- **Simple fix** - Only the endpoint response format needs to change from JSON to binary Response
- The v1 authenticated endpoint should be implemented separately following the same pattern
