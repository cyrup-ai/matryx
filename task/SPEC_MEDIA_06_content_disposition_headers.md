# SPEC_MEDIA_06: Add Required Content-Disposition Headers

## Status
**INCOMPLETE** - 2/10 Implementation Quality

## Critical Issues Found

### 1. Core Utility Function Missing
**File**: `packages/server/src/utils/response_helpers.rs`

The `calculate_content_disposition()` function specified in the implementation plan **DOES NOT EXIST**.

**Required**: Add function after line 88 that:
- Takes `content_type` and optional `filename` as parameters
- Returns `"inline"` only for safe types (uses existing `is_safe_inline_content_type()`)
- Returns `"attachment"` for all other types
- Sanitizes filename per RFC 6266 (removes quotes, backslashes, path separators, percent signs)
- Returns string like `"inline; filename=\"sanitized_name\""` or `"attachment; filename=\"sanitized_name\""`

### 2. Download Endpoints Have Security Vulnerability
**Files**: 
- `packages/server/src/_matrix/media/v1/download.rs` (lines 47-49)
- `packages/server/src/_matrix/media/v3/download/by_server_name/by_media_id/by_file_name.rs` (lines 50-54)

**Current Code** (WRONG):
```rust
if let Some(filename) = download_result.filename {
    response = response
        .header(header::CONTENT_DISPOSITION, format!("inline; filename=\"{}\"", filename));
}
```

**Problem**: Always uses `inline` regardless of content type. This allows XSS attacks via malicious HTML/JS files.

**Fix Required**:
1. Import: `use crate::utils::response_helpers::calculate_content_disposition;`
2. Replace the if-block with:
```rust
let content_disposition = calculate_content_disposition(
    &download_result.content_type,
    download_result.filename.as_deref()
);
response = response.header(header::CONTENT_DISPOSITION, content_disposition);
```

### 3. Thumbnail Endpoint Returns JSON Instead of Binary Image
**File**: `packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs`

**Current Code** (CRITICAL BUG):
```rust
Ok(Json(json!({
    "content_type": thumbnail_result.content_type,
    "width": thumbnail_result.width,
    "height": thumbnail_result.height
})))
```

**Problem**: Returns JSON metadata instead of actual image bytes. Clients cannot display thumbnails.

**Fix Required**: Complete rewrite to:
- Return `Response<Body>` instead of `Json<Value>`
- Use `Body::from(thumbnail_result.thumbnail)` for response body
- Add proper headers: Content-Type, Content-Disposition (using calculate_content_disposition)
- Add security headers (CSP, CORS, etc.)
- Accept `AuthenticatedUser` parameter for access control

### 4. Client v1 Download Endpoint Is Non-Functional Stub
**File**: `packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/by_file_name.rs`

**Current Code** (CRITICAL BUG):
```rust
Ok(Json(json!({
    "content_type": "image/jpeg",
    "content_disposition": "attachment; filename=example.jpg"
})))
```

**Problem**: Returns hardcoded JSON stub instead of actual media content.

**Fix Required**: Complete rewrite to:
- Accept `State<AppState>` and `AuthenticatedUser` parameters
- Create MediaService with federation support
- Call `media_service.download_media()` with authentication
- Return binary content with proper headers using `calculate_content_disposition()`

### 5. Client v1 Thumbnail Endpoint Is Non-Functional Stub
**File**: `packages/server/src/_matrix/client/v1/media/thumbnail/by_server_name/by_media_id.rs`

**Current Code** (CRITICAL BUG):
```rust
Ok(Json(json!({
    "content_type": "image/jpeg",
    "content_disposition": "attachment; filename=thumbnail.jpg"
})))
```

**Problem**: Returns hardcoded JSON stub instead of actual thumbnail image.

**Fix Required**: Complete rewrite to:
- Accept `State<AppState>`, `AuthenticatedUser`, and `Query<ThumbnailQuery>` parameters
- Create MediaService
- Call `media_service.generate_thumbnail()`
- Return binary image with proper headers using `calculate_content_disposition()`

## Definition of Done

Task is complete ONLY when ALL of the following are true:

1. ✅ `calculate_content_disposition()` function exists and works correctly
   - Uses `is_safe_inline_content_type()` to determine disposition
   - Sanitizes filenames (removes quotes, backslashes, path separators, percent signs)
   - Returns proper header value string

2. ✅ Download endpoints use content-type-aware disposition
   - v1/download.rs uses `calculate_content_disposition()`
   - v3/download/by_file_name.rs uses `calculate_content_disposition()`
   - Safe types (images, videos) get `inline`
   - Dangerous types (HTML, JS) get `attachment`

3. ✅ All thumbnail endpoints return binary image data
   - v3/thumbnail returns `Response<Body>` with image bytes
   - v1/thumbnail returns `Response<Body>` with image bytes
   - Both include Content-Disposition header

4. ✅ Client v1 endpoints fully implemented
   - v1/download returns actual media content (not JSON)
   - v1/thumbnail returns actual thumbnail image (not JSON)
   - Both use MediaService for data retrieval
   - Both include authentication

5. ✅ Code compiles successfully
   - `cargo build -p matryx_server` completes without errors

## Current Compilation Status

Code does NOT compile due to unrelated error in presence streams. This task's changes must not introduce additional compilation errors.

## Implementation Priority

1. **FIRST**: Add `calculate_content_disposition()` function to response_helpers.rs
2. **SECOND**: Fix the two download endpoints to use the new function
3. **THIRD**: Fix v3/thumbnail to return binary image
4. **FOURTH**: Implement v1/download endpoint
5. **FIFTH**: Implement v1/thumbnail endpoint
6. **VERIFY**: Test that safe types get inline, dangerous types get attachment

## References

- Matrix v1.12 Content Repository: `packages/server/tmp/matrix-spec/content/client-server-api/modules/content_repo.md` (lines 166-211)
- RFC 6266 Filename Escaping: https://www.rfc-editor.org/rfc/rfc6266.html#appendix-D
- Existing safe type checker: `packages/server/src/utils/response_helpers.rs:59-88`
