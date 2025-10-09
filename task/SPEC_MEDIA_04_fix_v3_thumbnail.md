# SPEC_MEDIA_04: Fix v3 Thumbnail to Return Binary Data

## Status
Bug - Incorrect Implementation

## Description
The v3 thumbnail endpoint currently returns JSON metadata instead of binary image data, violating the Matrix specification.

## Current State
**File**: `packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs`

**Current (WRONG)**:
```rust
Ok(Json(json!({
    "content_type": thumbnail_result.content_type,
    "width": thumbnail_result.width,
    "height": thumbnail_result.height
})))
```

This returns JSON with metadata. The spec requires binary image data.

## Spec Requirements

### Endpoint: `GET /_matrix/media/v3/thumbnail/{serverName}/{mediaId}`
**Status**: Deprecated in v1.11 (but must still work until v1.12 freeze)

**Response**:
- MUST return binary image data (not JSON)
- MUST include `Content-Type` header (image/jpeg, image/png, etc.)
- MUST include `Content-Disposition: inline; filename="thumbnail.png"` (v1.12+)
- Headers must match the content being returned

**Supported Content Types**:
- `image/jpeg`
- `image/png` (may be APNG if animated)
- `image/apng` (v1.11+)
- `image/gif` (v1.11+)
- `image/webp` (v1.11+)

## Implementation Fix

```rust
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::Response,
};

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
    let thumbnail_result = media_service
        .generate_thumbnail(&media_id, &server_name, query.width, query.height, &query.method)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Return BINARY data, not JSON
    let body = Body::from(thumbnail_result.data); // Get actual bytes

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, thumbnail_result.content_type)
        .header(header::CONTENT_DISPOSITION, "inline; filename=\"thumbnail.png\"")
        .body(body)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
```

## MediaService Changes Required

The `generate_thumbnail()` method needs to return actual binary data:

```rust
pub struct ThumbnailResult {
    pub content_type: String,
    pub data: Vec<u8>,  // Binary thumbnail data
    pub width: u32,
    pub height: u32,
}
```

Currently it likely just returns metadata. Need to implement actual thumbnail generation:
- Use image processing library (e.g., `image` crate)
- Load original media
- Resize according to method (scale/crop)
- Encode to appropriate format (JPEG, PNG, WebP)
- Return binary data

## Implementation Tasks

1. Update ThumbnailResult struct to include `data: Vec<u8>`

2. Implement thumbnail generation in MediaService:
   - Load media from storage
   - Decode image
   - Resize based on method:
     - "scale": Maintain aspect ratio
     - "crop": Crop to exact aspect ratio
   - Encode to output format
   - Return binary data

3. Fix v3/thumbnail endpoint to return binary Response

4. Fix v1 thumbnail endpoint (when implemented) to use same logic

## Libraries Needed
```toml
[dependencies]
image = "0.24"  # Image processing
```

## Testing
```bash
# Should return image file, not JSON
curl "http://localhost:8008/_matrix/media/v3/thumbnail/example.com/abc123?width=64&height=64" \
  --output test_thumb.png

# Verify it's actually an image
file test_thumb.png
# Should output: test_thumb.png: PNG image data, 64 x 64, ...
```

## References
- Spec: `/tmp/matrix-spec/data/api/client-server/content-repo.yaml` lines 461-622
- Current: `packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs`
- Thumbnail spec: `/tmp/matrix-spec/content/client-server-api/modules/content_repo.md` lines 80-126
