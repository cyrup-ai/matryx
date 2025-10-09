# SPEC_MEDIA_03: Authenticated Preview URL and Config Endpoints

## Status
**✅ IMPLEMENTATION COMPLETE**

Both authenticated media endpoints are fully implemented, registered in the router, and protected by authentication middleware.

## Executive Summary

Matrix v1.11 introduced authenticated versions of the media preview and config endpoints. These endpoints moved from `/_matrix/media/v3/*` (unauthenticated) to `/_matrix/client/v1/media/*` (authenticated).

**Current Implementation Status:**
- ✅ `GET /_matrix/client/v1/media/preview_url` - Fully implemented with OpenGraph parsing
- ✅ `GET /_matrix/client/v1/media/config` - Fully implemented with upload size limits
- ✅ Routes registered in main.rs router
- ✅ Authentication middleware applied via `require_auth_middleware`

## Implementation Architecture

### Directory Structure
```
packages/server/src/_matrix/client/v1/media/
├── config.rs          (10 lines)  - Upload limits endpoint
├── preview_url.rs     (203 lines) - URL preview endpoint  
├── download/          - Media download endpoints
├── thumbnail/         - Thumbnail generation endpoints
└── mod.rs            - Module exports
```

### Endpoint 1: Preview URL - COMPLETE

**Location:** [`packages/server/src/_matrix/client/v1/media/preview_url.rs`](../../packages/server/src/_matrix/client/v1/media/preview_url.rs)

**Implementation Details:**

```rust
/// GET /_matrix/client/v1/media/preview_url
pub async fn get(
    State(state): State<AppState>,
    Query(query): Query<PreviewQuery>,
) -> Result<Json<PreviewResponse>, StatusCode>
```

**Key Features Implemented:**

1. **URL Validation** (lines 47-50)
   - Validates HTTP/HTTPS scheme
   - Rejects non-web URLs
   - Returns `400 BAD_REQUEST` for invalid URLs

2. **HTTP Fetching** (lines 66-73)
   - 10-second timeout
   - Custom User-Agent header
   - Size limit enforcement (1MB for HTML)
   - Content-Type validation (text/html only)

3. **OpenGraph Metadata Parsing** (lines 93-125)
   - Extracts `og:title`, `og:description`, `og:image`
   - Regex-based meta tag parsing
   - Fallback to standard HTML `<title>` tag
   - Fallback to standard `<meta name="description">` tag

4. **Image Download & Storage** (lines 127-180)
   - Resolves relative URLs to absolute
   - Downloads preview images (5MB max)
   - Stores images via MediaService
   - Returns `mxc://` URI for cached images
   - Includes `matrix:image:size` field

5. **URL Resolution Helper** (lines 189-202)
   - Handles protocol-relative URLs (`//example.com/image.png`)
   - Resolves relative paths
   - Joins base URLs properly

**Query Parameters:**
- `url` (required) - URL to preview
- `ts` (optional) - Preferred timestamp (currently unused, reserved for caching)

**Response Format:**
```json
{
  "og:title": "Page Title",
  "og:description": "Page description", 
  "og:image": "mxc://example.com/mediaId",
  "matrix:image:size": 102400
}
```

### Endpoint 2: Config - COMPLETE

**Location:** [`packages/server/src/_matrix/client/v1/media/config.rs`](../../packages/server/src/_matrix/client/v1/media/config.rs)

**Implementation Details:**

```rust
/// GET /_matrix/client/v1/media/config
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "m.upload.size": 50000000
    })))
}
```

**Current Behavior:**
- Returns hardcoded 50MB (50,000,000 bytes) upload limit
- Simple implementation meeting spec requirements
- Does not require AppState or database access

**Comparison with v3 Implementation:**

The v3 config endpoint ([`packages/server/src/_matrix/media/v3/config.rs`](../../packages/server/src/_matrix/media/v3/config.rs)) uses MediaService for dynamic limits based on storage usage:

```rust
// v3 dynamic approach (optional enhancement for v1)
let statistics = media_service.get_media_statistics(Some(&state.homeserver_name)).await?;
let upload_size = if statistics.total_size > (1024 * 1024 * 1024) {
    base_limit / 2  // Reduce if storage high
} else {
    base_limit
};
```

**Note:** The v1 implementation is simpler but valid. Dynamic limits could be added later if needed.

## Authentication Implementation

### Middleware Application

**Router Registration:** [`packages/server/src/main.rs`](../../packages/server/src/main.rs) lines 450-451

```rust
fn create_client_routes() -> Router<AppState> {
    Router::new()
        .layer(axum_middleware::from_fn(require_auth_middleware))
        // ... other routes ...
        .route("/v1/media/config", get(_matrix::client::v1::media::config::get))
        .route("/v1/media/preview_url", get(_matrix::client::v1::media::preview_url::get))
```

Both endpoints are in `create_client_routes()` which applies `require_auth_middleware` to all routes.

### Authentication Middleware Details

**Location:** [`packages/server/src/auth/middleware.rs`](../../packages/server/src/auth/middleware.rs)

**Flow:**

1. **Auth Extraction** (line 27-30)
   - Checks `Authorization` header for `Bearer <token>`
   - Validates token via `MatrixSessionService`
   - Returns `401 Unauthorized` if missing/invalid

2. **Token Validation** (line 280-305)
   ```rust
   pub async fn extract_matrix_auth(
       headers: &HeaderMap,
       session_service: &MatrixSessionService<Any>,
   ) -> Result<MatrixAuth, MatrixAuthError> {
       let auth_header = headers.get(AUTHORIZATION)
           .and_then(|h| h.to_str().ok())
           .ok_or(MatrixAuthError::MissingAuthorization)?;
       
       if auth_header.starts_with("Bearer ") {
           let token = auth_header.strip_prefix("Bearer ")?;
           let access_token = session_service.validate_access_token(token).await?;
           Ok(MatrixAuth::User(access_token))
       }
   }
   ```

3. **Require Auth Middleware** (line 127-132)
   ```rust
   pub async fn require_auth_middleware(
       request: Request, 
       next: Next
   ) -> Result<Response, Response> {
       if request.extensions().get::<MatrixAuth>().is_none() {
           return Err(MatrixError::Unauthorized.into_response());
       }
       Ok(next.run(request).await)
   }
   ```

**Error Responses:**
- `401 Unauthorized` - Missing or invalid access token
- `429 Too Many Requests` - Rate limited (via rate_limit_middleware)

## Matrix Specification Compliance

### Reference Documentation

**Primary Spec:** [`tmp/matrix-spec/data/api/client-server/authed-content-repo.yaml`](../../tmp/matrix-spec/data/api/client-server/authed-content-repo.yaml)

- Lines 350-413: Preview URL specification
- Lines 414-469: Config specification

**Content Repo Guide:** [`tmp/matrix-spec/content/client-server-api/modules/content_repo.md`](../../tmp/matrix-spec/content/client-server-api/modules/content_repo.md)

### Security Considerations from Spec

**Privacy Warning (lines 343-349 of authed-content-repo.yaml):**
> Clients should be wary of this endpoint allowing the homeserver to see potentially sensitive URLs in messages. Clients may wish to prompt the user before requesting URL previews from the homeserver, especially in encrypted rooms. End-to-end encryption does not mean that URLs are safe to share with the homeserver. Clients may not want to share URLs with the homeserver, which can mean that URLs being shared should also not be shared with the homeserver.

**Implementation Alignment:**
- ✅ No caching between users (each request fetches fresh)
- ✅ No persistent storage of preview metadata
- ✅ Images stored in media repository with mxc:// URIs
- ✅ Authentication required (limits exposure)

### OpenGraph Metadata Format

**Spec Requirements:** (authed-content-repo.yaml lines 380-407)

Required OpenGraph fields supported:
- `og:title` - Page title
- `og:description` - Page description  
- `og:image` - Image URL (converted to mxc:// URI)
- `og:image:type` - MIME type (from Content-Type header)
- `og:image:height` - Image height (not currently extracted)
- `og:image:width` - Image width (not currently extracted)
- `matrix:image:size` - Byte size (from downloaded content)

**Current Implementation:** Extracts title, description, image URL, and size. Image dimensions not extracted but optional per spec.

## Code Patterns & Best Practices

### Pattern 1: URL Validation

```rust
// Reject non-HTTP/HTTPS URLs early
if !query.url.starts_with("http://") && !query.url.starts_with("https://") {
    return Err(StatusCode::BAD_REQUEST);
}
```

### Pattern 2: Size Limits

```rust
// HTML content limit (1MB)
let body = response.bytes().await?;
if body.len() > 1024 * 1024 {
    return Err("Response too large".into());
}

// Image download limit (5MB)
match img_response.bytes().await {
    Ok(img_data) if img_data.len() <= 5 * 1024 * 1024 => {
        // Process image
    },
    _ => {
        // Skip oversized images
    },
}
```

### Pattern 3: MediaService Integration

```rust
// Create MediaService instance
let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
let media_service = MediaService::new(media_repo, room_repo, membership_repo);

// Upload preview image
let upload_result = media_service.upload_media(
    &format!("@system:{}", homeserver_name),
    &img_data,
    &img_content_type,
    Some("preview_image"),
).await?;

// Returns: { content_uri: "mxc://server/mediaId" }
```

### Pattern 4: Regex-based HTML Parsing

```rust
// Extract OpenGraph title
if let Ok(title_regex) = 
    regex::Regex::new(r#"<meta[^>]+property="og:title"[^>]+content="([^"]*)"[^>]*>"#)
    && let Some(cap) = title_regex.captures(&html)
{
    title = Some(cap[1].to_string());
}
```

**Note:** Using regex for HTML parsing is acceptable for this simple use case but could be replaced with a proper HTML parser (like `scraper` crate) for robustness.

## Related Implementations

### Deprecated v3 Endpoints (Unauthenticated)

**Still Available for Backward Compatibility:**

1. **v3 Preview URL:** [`packages/server/src/_matrix/media/v3/preview_url.rs`](../../packages/server/src/_matrix/media/v3/preview_url.rs)
   - Nearly identical implementation
   - No authentication required
   - Deprecated per Matrix v1.11 spec

2. **v3 Config:** [`packages/server/src/_matrix/media/v3/config.rs`](../../packages/server/src/_matrix/media/v3/config.rs)
   - Uses dynamic limits based on storage
   - No authentication required
   - Deprecated per Matrix v1.11 spec

**Migration Timeline:** Per spec, servers SHOULD "freeze" unauthenticated endpoints by Matrix 1.12, allowing only pre-freeze media to be accessed without authentication.

## Potential Enhancements (Optional)

While the implementation is complete and spec-compliant, these enhancements could be considered:

### 1. Dynamic Upload Limits in v1 Config

**Current:** Hardcoded 50MB
**Enhancement:** Use MediaService like v3 does

```rust
pub async fn get(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let media_service = create_media_service(&state);
    let statistics = media_service.get_media_statistics(Some(&state.homeserver_name)).await?;
    
    let base_limit: u64 = 50 * 1024 * 1024;
    let upload_size = if statistics.total_size > (1024 * 1024 * 1024) {
        base_limit / 2
    } else {
        base_limit
    };
    
    Ok(Json(json!({ "m.upload.size": upload_size })))
}
```

### 2. Image Dimension Extraction

**Current:** Not extracted
**Enhancement:** Use `image` crate to extract width/height

```rust
use image::GenericImageView;

if let Ok(img) = image::load_from_memory(&img_data) {
    let (width, height) = img.dimensions();
    // Add to response: og:image:width, og:image:height
}
```

### 3. Preview Caching

**Current:** Fresh fetch every time
**Enhancement:** Cache previews with TTL to reduce external requests

### 4. HTML Parser Instead of Regex

**Current:** Regex-based extraction
**Enhancement:** Use `scraper` crate for robust HTML parsing

```rust
use scraper::{Html, Selector};

let document = Html::parse_document(&html);
let og_title = Selector::parse(r#"meta[property="og:title"]"#)?;
if let Some(element) = document.select(&og_title).next() {
    title = element.value().attr("content").map(|s| s.to_string());
}
```

**Note:** These enhancements are NOT required for spec compliance.

## Definition of Done

The implementation is considered complete when:

- ✅ `GET /_matrix/client/v1/media/preview_url` endpoint responds with OpenGraph metadata
- ✅ `GET /_matrix/client/v1/media/config` endpoint responds with upload size limits
- ✅ Both endpoints require Bearer token authentication
- ✅ Both endpoints return `401` for missing/invalid authentication
- ✅ Routes are registered in the main.rs router
- ✅ Preview URL endpoint fetches and parses HTML content
- ✅ Preview URL endpoint downloads and stores preview images as mxc:// URIs
- ✅ Config endpoint returns valid `m.upload.size` value

**Verification Commands:**

```bash
# Get access token first
TOKEN=$(curl -X POST http://localhost:8008/_matrix/client/v3/login \
  -H "Content-Type: application/json" \
  -d '{"type":"m.login.password","user":"test","password":"test"}' \
  | jq -r '.access_token')

# Test preview_url endpoint
curl -H "Authorization: Bearer $TOKEN" \
  "http://localhost:8008/_matrix/client/v1/media/preview_url?url=https://matrix.org"

# Expected: JSON with og:title, og:description, etc.

# Test config endpoint  
curl -H "Authorization: Bearer $TOKEN" \
  "http://localhost:8008/_matrix/client/v1/media/config"

# Expected: {"m.upload.size": 50000000}

# Test authentication requirement
curl "http://localhost:8008/_matrix/client/v1/media/config"

# Expected: 401 Unauthorized
```

## Source Code Reference Map

**Implementation Files:**
- Main router: [`packages/server/src/main.rs`](../../packages/server/src/main.rs) (lines 450-451)
- Preview URL: [`packages/server/src/_matrix/client/v1/media/preview_url.rs`](../../packages/server/src/_matrix/client/v1/media/preview_url.rs)
- Config: [`packages/server/src/_matrix/client/v1/media/config.rs`](../../packages/server/src/_matrix/client/v1/media/config.rs)
- Module export: [`packages/server/src/_matrix/client/v1/media/mod.rs`](../../packages/server/src/_matrix/client/v1/media/mod.rs)

**Authentication:**
- Middleware: [`packages/server/src/auth/middleware.rs`](../../packages/server/src/auth/middleware.rs)
- Session service: [`packages/server/src/auth/session_service.rs`](../../packages/server/src/auth/session_service.rs)

**Legacy v3 Implementations:**
- v3 Preview URL: [`packages/server/src/_matrix/media/v3/preview_url.rs`](../../packages/server/src/_matrix/media/v3/preview_url.rs)
- v3 Config: [`packages/server/src/_matrix/media/v3/config.rs`](../../packages/server/src/_matrix/media/v3/config.rs)

**Matrix Specification:**
- API Spec: [`tmp/matrix-spec/data/api/client-server/authed-content-repo.yaml`](../../tmp/matrix-spec/data/api/client-server/authed-content-repo.yaml)
- Content Repo Guide: [`tmp/matrix-spec/content/client-server-api/modules/content_repo.md`](../../tmp/matrix-spec/content/client-server-api/modules/content_repo.md)

## Summary

Both Matrix v1.11 authenticated media endpoints are **fully implemented and operational**. The preview_url endpoint provides complete OpenGraph metadata extraction with image caching, while the config endpoint returns upload size limits. Authentication is properly enforced via the existing middleware stack. The implementation follows the Matrix specification and uses established patterns from the codebase.

No further implementation work is required. Optional enhancements like dynamic upload limits or improved HTML parsing can be considered for future iterations.
