# SPEC_MEDIA_04: Fix v3 Thumbnail to Return Binary Data

## Status
**BUG** - Critical Implementation Error

## Priority
HIGH - Endpoint violates Matrix specification

## QA Rating: 1/10

**Critical Issues:**
- ❌ Returns JSON metadata instead of binary image data
- ❌ Wrong function return type: `Json<Value>` instead of `Response<Body>`
- ❌ Missing all required HTTP headers (Content-Type, Content-Disposition, CORS)
- ❌ Ignores the actual thumbnail binary data in `thumbnail_result.thumbnail`
- ❌ Would break all Matrix clients attempting to display thumbnails

---

## Required Fix

### File: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs`

### STEP 1: Fix Imports (Lines 1-12)

**Remove:**
```rust
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde_json::{Value, json};
```

**Replace with:**
```rust
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::Response,
};
```

### STEP 2: Fix Function Signature (Line 18)

**Change:**
```rust
) -> Result<Json<Value>, StatusCode> {
```

**To:**
```rust
) -> Result<Response<Body>, StatusCode> {
```

### STEP 3: Fix Response Construction (Lines 36-41)

**Remove:**
```rust
Ok(Json(json!({
    "content_type": thumbnail_result.content_type,
    "width": thumbnail_result.width,
    "height": thumbnail_result.height
})))
```

**Replace with:**
```rust
// Return binary thumbnail data per Matrix spec
let body = Body::from(thumbnail_result.thumbnail);

Response::builder()
    .status(StatusCode::OK)
    .header(header::CONTENT_TYPE, thumbnail_result.content_type)
    .header(header::CONTENT_DISPOSITION, "inline; filename=\"thumbnail.png\"")
    .header("Cross-Origin-Resource-Policy", "cross-origin")
    .header("Access-Control-Allow-Origin", "*")
    .body(body)
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
```

---

## Alternative: Use Existing Helper

Add import:
```rust
use crate::utils::response_helpers::media_response;
```

Replace response (lines 36-41):
```rust
media_response(
    &thumbnail_result.content_type,
    thumbnail_result.thumbnail.len() as u64,
    Some("thumbnail.png"),
    Body::from(thumbnail_result.thumbnail)
)
```

The helper at `/Volumes/samsung_t9/maxtryx/packages/server/src/utils/response_helpers.rs:27-54` automatically adds all required headers.

---

## Verification

After fix, test with:
```bash
curl http://localhost:8008/_matrix/media/v3/thumbnail/localhost/abc123?width=64&height=64 --output test.png
```

The file `test.png` MUST be a valid image, NOT a JSON file.

---

## Definition of Done

1. ✅ Returns binary image data (not JSON)
2. ✅ Content-Type header set to image MIME type
3. ✅ Content-Disposition: inline header present
4. ✅ CORS headers included
5. ✅ Response body contains `thumbnail_result.thumbnail` bytes
6. ✅ Function returns `Result<Response<Body>, StatusCode>`
7. ✅ curl test produces valid image file
