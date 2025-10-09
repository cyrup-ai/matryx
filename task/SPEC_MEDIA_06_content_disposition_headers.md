# SPEC_MEDIA_06: Add Required Content-Disposition Headers

## Status
Missing - Matrix v1.12 Spec Compliance Issue

## Description
Matrix v1.12 requires `Content-Disposition` headers on all download and thumbnail responses. Current implementation has the header but **always uses `inline`** regardless of content type, which violates the spec's XSS prevention requirements. Additionally, some endpoints return JSON stubs instead of actual binary media.

## Research Summary

### Matrix Specification Requirements
Per [Matrix v1.12 Content Repository Module](../../packages/server/tmp/matrix-spec/content/client-server-api/modules/content_repo.md#L166-L211):

- **Content-Disposition header is REQUIRED** on all media download and thumbnail endpoints (added in v1.12)
- Must be `inline` ONLY for safe content types (specified list below)
- Must be `attachment` for all other types to prevent XSS attacks
- Must include filename parameter when available
- Servers MAY always use `attachment` for extra safety
- Clients SHOULD NOT rely on receiving `inline` vs `attachment`

### RFC 6266 Filename Escaping
Per [RFC 6266 Appendix D](https://www.rfc-editor.org/rfc/rfc6266.html#appendix-D):

- **Avoid backslash `\`** - escaping not universally implemented by user agents
- **Avoid percent encoding `%XX`** - interpreted differently across browsers
- **Avoid non-ASCII characters** in `filename` parameter (use `filename*` for UTF-8)
- **Remove path separators** (`/`, `\`) to prevent directory traversal

Safest approach: Sanitize filename to remove problematic characters.

## Current Codebase State

### ✅ Already Implemented
1. **Safe content type checker EXISTS**: [`packages/server/src/utils/response_helpers.rs:59-88`](../../packages/server/src/utils/response_helpers.rs)
   ```rust
   pub fn is_safe_inline_content_type(content_type: &str) -> bool
   ```
   This function already implements the exact safe type list from the Matrix spec!

2. **MediaDownloadResult has filename field**: [`packages/surrealdb/src/repository/media_service.rs:59-64`](../../packages/surrealdb/src/repository/media_service.rs)
   ```rust
   pub struct MediaDownloadResult {
       pub content: Vec<u8>,
       pub content_type: String,
       pub content_length: u64,
       pub filename: Option<String>,  // ✓ Already exists
   }
   ```

3. **Federation endpoints are correct**: Use `build_multipart_media_response()` which properly handles Content-Disposition

### ❌ Issues to Fix

#### Issue 1: Download endpoints hardcode "inline"
**File**: [`packages/server/src/_matrix/media/v1/download.rs:47-51`](../../packages/server/src/_matrix/media/v1/download.rs)
```rust
// CURRENT (WRONG):
if let Some(filename) = download_result.filename {
    response = response
        .header(header::CONTENT_DISPOSITION, format!("inline; filename=\"{}\"", filename));
}
```
**Problem**: Always uses `inline`, doesn't check content type safety.

**Same issue in**:
- [`packages/server/src/_matrix/media/v3/download/by_server_name/by_media_id/by_file_name.rs:50-54`](../../packages/server/src/_matrix/media/v3/download/by_server_name/by_media_id/by_file_name.rs)

#### Issue 2: Thumbnail endpoint returns JSON instead of binary
**File**: [`packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs`](../../packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs)
```rust
// CURRENT (COMPLETELY WRONG):
Ok(Json(json!({
    "content_type": thumbnail_result.content_type,
    "width": thumbnail_result.width,
    "height": thumbnail_result.height
})))
```
**Problem**: Returns JSON metadata instead of binary image data with proper headers.

#### Issue 3: Client v1 endpoints are stubs
**Files**:
- [`packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/by_file_name.rs`](../../packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/by_file_name.rs) - Returns JSON stub
- [`packages/server/src/_matrix/client/v1/media/thumbnail/by_server_name/by_media_id.rs`](../../packages/server/src/_matrix/client/v1/media/thumbnail/by_server_name/by_media_id.rs) - Returns JSON stub

## Implementation Plan

### Step 1: Add Content-Disposition Calculation Function

**File**: [`packages/server/src/utils/response_helpers.rs`](../../packages/server/src/utils/response_helpers.rs)

**Location**: Add after the existing `is_safe_inline_content_type()` function (after line 88)

```rust
/// Calculate Content-Disposition header value per Matrix v1.12 spec and RFC 6266
///
/// # Arguments
/// * `content_type` - MIME type of the content
/// * `filename` - Optional filename to include in header
///
/// # Returns
/// Content-Disposition header value: either "inline" or "attachment" with optional filename
///
/// # Security
/// - Uses `inline` only for safe content types per Matrix spec (prevents XSS)
/// - Sanitizes filename per RFC 6266 Appendix D (removes problematic characters)
/// - Always uses `attachment` for potentially dangerous content types
///
/// # References
/// - Matrix v1.12 Content Repository: packages/server/tmp/matrix-spec/content/client-server-api/modules/content_repo.md
/// - RFC 6266: https://www.rfc-editor.org/rfc/rfc6266.html
pub fn calculate_content_disposition(
    content_type: &str,
    filename: Option<&str>,
) -> String {
    // Extract base content type (strip parameters like "charset=utf-8")
    let base_type = content_type.split(';').next().unwrap_or("").trim();
    
    // Determine disposition based on content type safety
    let disposition = if is_safe_inline_content_type(base_type) {
        "inline"
    } else {
        "attachment"
    };

    // Add filename if provided
    if let Some(name) = filename {
        // RFC 6266 Appendix D: Sanitize filename to avoid escaping issues
        // - Remove quotes (cause parsing issues)
        // - Remove backslashes (not universally escaped)
        // - Replace path separators with underscore (security)
        // - Remove percent signs (interpreted as encoding by some UAs)
        let sanitized = name
            .replace('\"', "")
            .replace('\\', "")
            .replace('/', "_")
            .replace('%', "");
        
        format!("{}; filename=\"{}\"", disposition, sanitized)
    } else {
        disposition.to_string()
    }
}
```

### Step 2: Update Existing Download Endpoints

#### 2.1: Fix `_matrix/media/v1/download.rs`

**File**: [`packages/server/src/_matrix/media/v1/download.rs`](../../packages/server/src/_matrix/media/v1/download.rs)

**Line 1**: Add import (after existing imports)
```rust
use crate::utils::response_helpers::calculate_content_disposition;
```

**Lines 47-51**: Replace the Content-Disposition logic
```rust
// REPLACE THIS:
if let Some(filename) = download_result.filename {
    response = response
        .header(header::CONTENT_DISPOSITION, format!("inline; filename=\"{}\"", filename));
}

// WITH THIS:
let content_disposition = calculate_content_disposition(
    &download_result.content_type,
    download_result.filename.as_deref()
);
response = response.header(header::CONTENT_DISPOSITION, content_disposition);
```

#### 2.2: Fix `_matrix/media/v3/download/.../by_file_name.rs`

**File**: [`packages/server/src/_matrix/media/v3/download/by_server_name/by_media_id/by_file_name.rs`](../../packages/server/src/_matrix/media/v3/download/by_server_name/by_media_id/by_file_name.rs)

**Line 1**: Add import
```rust
use crate::utils::response_helpers::calculate_content_disposition;
```

**Lines 50-54**: Replace the Content-Disposition logic (same as above)
```rust
let content_disposition = calculate_content_disposition(
    &download_result.content_type,
    download_result.filename.as_deref()
);
response = response.header(header::CONTENT_DISPOSITION, content_disposition);
```

### Step 3: Fix Thumbnail Endpoint to Return Binary

**File**: [`packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs`](../../packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs)

**Complete rewrite** - Replace entire file content:

```rust
use crate::AppState;
use crate::auth::authenticated_user::AuthenticatedUser;
use crate::utils::response_helpers::calculate_content_disposition;
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
pub async fn get(
    State(state): State<AppState>,
    Path((server_name, media_id)): Path<(String, String)>,
    Query(query): Query<ThumbnailQuery>,
    user: AuthenticatedUser,
) -> Result<Response<Body>, StatusCode> {
    // Create MediaService instance
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));

    let media_service = MediaService::new(media_repo, room_repo, membership_repo);

    // Generate thumbnail using MediaService
    let thumbnail_result = media_service
        .generate_thumbnail(&media_id, &server_name, query.width, query.height, &query.method)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Create response body from thumbnail data
    let body = Body::from(thumbnail_result.thumbnail);

    // Thumbnails always use "inline" (they're always safe image types)
    let content_disposition = calculate_content_disposition(
        &thumbnail_result.content_type,
        Some("thumbnail.png")
    );

    // Build response with proper headers
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, thumbnail_result.content_type)
        .header(header::CONTENT_DISPOSITION, content_disposition)
        .header(header::CONTENT_SECURITY_POLICY,
            "sandbox; default-src 'none'; script-src 'none'; plugin-types application/pdf; style-src 'unsafe-inline'; object-src 'self';")
        .header("Cross-Origin-Resource-Policy", "cross-origin")
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
        .header("Access-Control-Allow-Headers", "X-Requested-With, Content-Type, Authorization")
        .body(body)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
```

### Step 4: Implement Client v1 Download Endpoint

**File**: [`packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/by_file_name.rs`](../../packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/by_file_name.rs)

**Complete rewrite** - Replace entire file:

```rust
use crate::auth::authenticated_user::AuthenticatedUser;
use crate::AppState;
use crate::utils::response_helpers::calculate_content_disposition;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{StatusCode, header},
    response::Response,
};
use matryx_surrealdb::repository::{
    media::MediaRepository, media_service::MediaService, membership::MembershipRepository,
    room::RoomRepository,
};
use std::sync::Arc;

/// GET /_matrix/client/v1/media/download/{serverName}/{mediaId}/{fileName}
pub async fn get(
    State(state): State<AppState>,
    Path((server_name, media_id, _file_name)): Path<(String, String, String)>,
    user: AuthenticatedUser,
) -> Result<Response<Body>, StatusCode> {
    // Create MediaService instance with federation support
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));

    let media_service = MediaService::new(media_repo, room_repo, membership_repo)
        .with_federation_client(
            state.federation_media_client.clone(),
            state.homeserver_name.clone(),
        );

    // Download media using MediaService with authenticated user
    let download_result = media_service
        .download_media(&media_id, &server_name, &user.user_id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // Create response body from content
    let body = Body::from(download_result.content);

    // Calculate proper Content-Disposition based on content type
    let content_disposition = calculate_content_disposition(
        &download_result.content_type,
        download_result.filename.as_deref()
    );

    // Build response with appropriate headers and security headers
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, download_result.content_type)
        .header(header::CONTENT_LENGTH, download_result.content_length.to_string())
        .header(header::CONTENT_DISPOSITION, content_disposition)
        .header(header::CONTENT_SECURITY_POLICY,
            "sandbox; default-src 'none'; script-src 'none'; plugin-types application/pdf; style-src 'unsafe-inline'; object-src 'self';")
        .header("Cross-Origin-Resource-Policy", "cross-origin")
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
        .header("Access-Control-Allow-Headers", "X-Requested-With, Content-Type, Authorization")
        .body(body)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
```

### Step 5: Implement Client v1 Thumbnail Endpoint

**File**: [`packages/server/src/_matrix/client/v1/media/thumbnail/by_server_name/by_media_id.rs`](../../packages/server/src/_matrix/client/v1/media/thumbnail/by_server_name/by_media_id.rs)

**Complete rewrite** - Replace entire file:

```rust
use crate::auth::authenticated_user::AuthenticatedUser;
use crate::AppState;
use crate::utils::response_helpers::calculate_content_disposition;
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
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct ThumbnailQuery {
    width: u32,
    height: u32,
    method: String,
}

/// GET /_matrix/client/v1/media/thumbnail/{serverName}/{mediaId}
pub async fn get(
    State(state): State<AppState>,
    Path((server_name, media_id)): Path<(String, String)>,
    Query(query): Query<ThumbnailQuery>,
    user: AuthenticatedUser,
) -> Result<Response<Body>, StatusCode> {
    // Create MediaService instance
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));

    let media_service = MediaService::new(media_repo, room_repo, membership_repo);

    // Generate thumbnail using MediaService
    let thumbnail_result = media_service
        .generate_thumbnail(&media_id, &server_name, query.width, query.height, &query.method)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Create response body from thumbnail data
    let body = Body::from(thumbnail_result.thumbnail);

    // Thumbnails always use "inline" (they're always safe image types)
    let content_disposition = calculate_content_disposition(
        &thumbnail_result.content_type,
        Some("thumbnail.png")
    );

    // Build response with proper headers
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, thumbnail_result.content_type)
        .header(header::CONTENT_DISPOSITION, content_disposition)
        .header(header::CONTENT_SECURITY_POLICY,
            "sandbox; default-src 'none'; script-src 'none'; plugin-types application/pdf; style-src 'unsafe-inline'; object-src 'self';")
        .header("Cross-Origin-Resource-Policy", "cross-origin")
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
        .header("Access-Control-Allow-Headers", "X-Requested-With, Content-Type, Authorization")
        .body(body)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
```

## Safe Content Types List (Matrix v1.12)

The following content types are safe for `inline` disposition (all others MUST use `attachment`):

```
text/css
text/plain
text/csv
application/json
application/ld+json
image/jpeg
image/gif
image/png
image/apng
image/webp
image/avif
video/mp4
video/webm
video/ogg
video/quicktime
audio/mp4
audio/webm
audio/aac
audio/mpeg
audio/ogg
audio/wave
audio/wav
audio/x-wav
audio/x-pn-wav
audio/flac
audio/x-flac
```

These are already implemented in `is_safe_inline_content_type()` in [`packages/server/src/utils/response_helpers.rs`](../../packages/server/src/utils/response_helpers.rs).

## Files Modified Summary

### Modified Files (6 total)
1. [`packages/server/src/utils/response_helpers.rs`](../../packages/server/src/utils/response_helpers.rs) - Add `calculate_content_disposition()` function
2. [`packages/server/src/_matrix/media/v1/download.rs`](../../packages/server/src/_matrix/media/v1/download.rs) - Use new function (lines 47-51)
3. [`packages/server/src/_matrix/media/v3/download/by_server_name/by_media_id/by_file_name.rs`](../../packages/server/src/_matrix/media/v3/download/by_server_name/by_media_id/by_file_name.rs) - Use new function (lines 50-54)
4. [`packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs`](../../packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs) - Return binary with headers
5. [`packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/by_file_name.rs`](../../packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/by_file_name.rs) - Full implementation
6. [`packages/server/src/_matrix/client/v1/media/thumbnail/by_server_name/by_media_id.rs`](../../packages/server/src/_matrix/client/v1/media/thumbnail/by_server_name/by_media_id.rs) - Full implementation

### Unchanged (Already Correct)
- **Federation endpoints**: Already use `build_multipart_media_response()` which handles Content-Disposition correctly
  - [`packages/server/src/_matrix/federation/v1/media/download/by_media_id.rs`](../../packages/server/src/_matrix/federation/v1/media/download/by_media_id.rs)
  - [`packages/server/src/_matrix/federation/v1/media/thumbnail/by_media_id.rs`](../../packages/server/src/_matrix/federation/v1/media/thumbnail/by_media_id.rs)

## Definition of Done

This task is complete when:

1. ✅ **New utility function added**: `calculate_content_disposition()` exists in `response_helpers.rs` and correctly:
   - Returns `inline` only for safe content types
   - Returns `attachment` for all other types
   - Sanitizes filename per RFC 6266
   - Includes filename parameter when provided

2. ✅ **Download endpoints return correct headers**:
   - Image files (JPEG, PNG, etc.) get `Content-Disposition: inline; filename="..."`
   - HTML/JS/unknown files get `Content-Disposition: attachment; filename="..."`
   - Filenames with quotes/backslashes/paths are sanitized

3. ✅ **Thumbnail endpoints return binary data**:
   - v3 thumbnail returns binary image with headers (not JSON)
   - v1 thumbnail returns binary image with headers (not JSON)
   - Both include `Content-Disposition: inline; filename="thumbnail.png"`

4. ✅ **Client v1 endpoints fully implemented**:
   - v1 download endpoint returns actual media content
   - v1 thumbnail endpoint returns actual thumbnail image
   - Both follow same patterns as v3 endpoints

5. ✅ **Code compiles successfully**: `cargo build -p matryx_server` completes without errors

## References

### Matrix Specification
- **Content Repository Module**: [packages/server/tmp/matrix-spec/content/client-server-api/modules/content_repo.md](../../packages/server/tmp/matrix-spec/content/client-server-api/modules/content_repo.md) (lines 166-211)
- **v1.12 Changelog**: [packages/server/tmp/matrix-spec/content/changelog/v1.12.md](../../packages/server/tmp/matrix-spec/content/changelog/v1.12.md)

### RFC Standards
- **RFC 6266** (Content-Disposition): https://www.rfc-editor.org/rfc/rfc6266.html
- **RFC 6266 Appendix D** (Filename Parameter): https://www.rfc-editor.org/rfc/rfc6266.html#appendix-D

### Existing Code
- **Safe type checker**: [`packages/server/src/utils/response_helpers.rs:59-88`](../../packages/server/src/utils/response_helpers.rs)
- **MediaDownloadResult**: [`packages/surrealdb/src/repository/media_service.rs:59-64`](../../packages/surrealdb/src/repository/media_service.rs)
- **Federation multipart response**: [`packages/server/src/utils/response_helpers.rs:131-151`](../../packages/server/src/utils/response_helpers.rs)
