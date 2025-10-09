# SPEC_MEDIA_02: Implement Authenticated Thumbnail Endpoint

## Status
Missing Implementation

## Description
The Matrix v1.11 specification introduced authenticated thumbnail endpoint at `/_matrix/client/v1/media/thumbnail/{serverName}/{mediaId}`. This replaces the deprecated unauthenticated v3 endpoint.

## Current State
- No v1 authenticated thumbnail endpoint exists
- v3 thumbnail endpoint exists but returns JSON instead of binary image data
- Current implementation in `packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs` is a stub

## Spec Requirements

### Endpoint: `GET /_matrix/client/v1/media/thumbnail/{serverName}/{mediaId}`
**Path**: `packages/server/src/_matrix/client/v1/media/thumbnail/by_server_name/by_media_id.rs`

**Required Query Parameters**:
- `width` (integer, required) - Desired width in pixels
- `height` (integer, required) - Desired height in pixels
- `method` (string, optional) - Either "crop" or "scale" (default: "scale")
- `timeout_ms` (integer, optional) - Timeout in milliseconds (default: 20000)
- `animated` (boolean, optional, v1.11+) - Request animated thumbnail

**Response**:
- Returns BINARY image data (not JSON)
- Must include `Content-Type` header:
  - `image/jpeg`
  - `image/png` (may be APNG if animated)
  - `image/apng` (v1.11+)
  - `image/gif` (v1.11+)
  - `image/webp` (v1.11+, preferred for animation)
- Must include `Content-Disposition: inline; filename="thumbnail.png"` (v1.12+)

**Thumbnail Generation Rules**:
- "scale": Maintains aspect ratio, returns image where width OR height ≤ requested
- "crop": Returns image close to requested aspect ratio and size
- MUST NOT upscale images
- MUST NOT return smaller than requested (unless original is smaller)
- If original is smaller than requested, return original content

**Recommended Standard Sizes**:
- 32x32, crop
- 96x96, crop
- 320x240, scale
- 640x480, scale
- 800x600, scale

**Animation Support (v1.11+)**:
- When `animated=true`: Return animated thumbnail if possible
- When `animated=false`: MUST NOT return animated thumbnail
- When not specified: SHOULD NOT return animated thumbnail
- Prefer `image/webp` for animated thumbnails
- If source cannot be animated (JPEG, PDF), treat as `animated=false`

## Implementation Tasks

1. Create directory structure:
   - `packages/server/src/_matrix/client/v1/media/thumbnail/`
   - `packages/server/src/_matrix/client/v1/media/thumbnail/by_server_name/`

2. Implement authenticated thumbnail handler:
   - Extract and validate access token
   - Parse and validate query parameters
   - Call MediaService.generate_thumbnail() with proper parameters
   - Return BINARY image data (not JSON)
   - Set proper Content-Type based on generated format
   - Add Content-Disposition header
   - Support animation preference

3. Update MediaService.generate_thumbnail():
   - Currently returns ThumbnailResult with metadata
   - Should return actual binary thumbnail data
   - Implement proper scaling/cropping logic
   - Support animated thumbnails (webp, apng, gif)
   - Respect size constraints from spec

4. Register routes in main.rs

## Error Responses Required
- `400` - Invalid dimensions (non-integer, negative, etc.)
- `401` - Unauthorized
- `413` - Local content too large to thumbnail
- `429` - Rate limited
- `502` - Remote content too large to thumbnail
- `504` - Content not yet uploaded

## Current Stub Code Issue
```rust
// Current stub returns JSON - WRONG
Ok(Json(json!({
    "content_type": thumbnail_result.content_type,
    "width": thumbnail_result.width,
    "height": thumbnail_result.height
})))

// Should return binary image data:
Ok(Response::builder()
    .header(CONTENT_TYPE, thumbnail_result.content_type)
    .header(CONTENT_DISPOSITION, "inline; filename=\"thumbnail.png\"")
    .body(Body::from(thumbnail_result.data))?)
```

## Verification
```bash
# Test authenticated thumbnail
curl -H "Authorization: Bearer <token>" \
  "http://localhost:8008/_matrix/client/v1/media/thumbnail/example.com/abc123?width=64&height=64&method=scale" \
  --output thumb.png

# Test animated thumbnail
curl -H "Authorization: Bearer <token>" \
  "http://localhost:8008/_matrix/client/v1/media/thumbnail/example.com/abc123?width=200&height=200&animated=true" \
  --output animated_thumb.webp

# Verify image was generated correctly
file thumb.png
```

## References
- Spec: `/tmp/matrix-spec/data/api/client-server/authed-content-repo.yaml` lines 148-358
- Spec: `/tmp/matrix-spec/content/client-server-api/modules/content_repo.md` lines 80-126
