# SPEC_MEDIA_01: Implement Authenticated Download Endpoints

## Status
**Implementation Required** - Stub handlers exist, need replacement with full implementation

## Overview
The Matrix v1.11 specification introduced authenticated media download endpoints at `/_matrix/client/v1/media/*` to replace deprecated unauthenticated endpoints. These endpoints require authentication and include enhanced security headers to protect against XSS attacks.

## Current State

### Existing Infrastructure
- ✅ Directory structure exists: `packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/`
- ✅ Routes registered in [main.rs](../packages/server/src/main.rs#L477-L478)
- ✅ MediaService available at [packages/surrealdb/src/repository/media_service.rs](../packages/surrealdb/src/repository/media_service.rs)
- ✅ AuthenticatedUser extractor at [packages/server/src/auth/authenticated_user.rs](../packages/server/src/auth/authenticated_user.rs)
- ✅ Working reference implementation at [packages/server/src/_matrix/media/v1/download.rs](../packages/server/src/_matrix/media/v1/download.rs)

### Current Stub Implementations
**File**: `packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/by_media_id.rs` (14 lines)
```rust
// Returns stub JSON response - needs replacement with binary content + headers
```

**File**: `packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/by_file_name.rs` (13 lines)
```rust
// Returns stub JSON response - needs replacement with binary content + headers  
```

## Specification Requirements

### Matrix v1.11+ Download Endpoints

Based on [Matrix Spec: authed-content-repo.yaml](../tmp/matrix-spec/data/api/client-server/authed-content-repo.yaml#L18-L147):

#### Endpoint 1: `GET /_matrix/client/v1/media/download/{serverName}/{mediaId}`

**Authentication**: Required (Bearer token via AuthenticatedUser extractor)

**Query Parameters**:
- `timeout_ms` (optional): Maximum wait time in milliseconds (default: 20000, max: 120000)

**Response Headers** (REQUIRED):
- `Content-Type`: Original upload type or reasonably close (see spec §downloadContentType)
- `Content-Disposition`: `inline` or `attachment` based on content type + filename if available (REQUIRED in v1.12+)
- `Content-Security-Policy`: `sandbox; default-src 'none'; script-src 'none'; plugin-types application/pdf; style-src 'unsafe-inline'; object-src 'self';`
- `Cross-Origin-Resource-Policy`: `cross-origin` (added in Matrix v1.4)

**Response Body**: Binary content (NOT JSON, NOT multipart)

#### Endpoint 2: `GET /_matrix/client/v1/media/download/{serverName}/{mediaId}/{fileName}`

Same as Endpoint 1, except:
- **MUST** include the `{fileName}` parameter in `Content-Disposition` header (overrides stored filename)

### Content-Disposition Logic

Per [Matrix Spec: content_repo.md](../tmp/matrix-spec/content/client-server-api/modules/content_repo.md#L168-L211):

**Inline Content Types** (35 allowed types):
```rust
const INLINE_CONTENT_TYPES: &[&str] = &[
    "text/css",
    "text/plain",
    "text/csv",
    "application/json",
    "application/ld+json",
    "image/jpeg",
    "image/gif",
    "image/png",
    "image/apng",
    "image/webp",
    "image/avif",
    "video/mp4",
    "video/webm",
    "video/ogg",
    "video/quicktime",
    "audio/mp4",
    "audio/webm",
    "audio/aac",
    "audio/mpeg",
    "audio/ogg",
    "audio/wave",
    "audio/wav",
    "audio/x-wav",
    "audio/x-pn-wav",
    "audio/flac",
    "audio/x-flac",
];
```

**Decision Tree**:
1. Check if `content_type` is in `INLINE_CONTENT_TYPES`
2. If YES: `Content-Disposition: inline; filename="..."`
3. If NO: `Content-Disposition: attachment; filename="..."`
4. Include filename from: `{fileName}` parameter > stored `upload_name` > omit if none

### Security Headers

Per [Matrix Spec: content_repo.md](../tmp/matrix-spec-relationships/content/client-server-api/modules/content_repo.md#L15-L21):

```rust
// Required security headers for all media downloads
headers.insert(
    "Content-Security-Policy",
    "sandbox; default-src 'none'; script-src 'none'; plugin-types application/pdf; style-src 'unsafe-inline'; object-src 'self';"
);
headers.insert(
    "Cross-Origin-Resource-Policy",
    "cross-origin"
);
```

## Implementation Plan

### Files to Modify

#### 1. `packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/by_media_id.rs`

**Current**: 14-line stub returning JSON  
**Action**: Replace with full authenticated binary download handler

**Implementation Pattern**:
```rust
use crate::auth::authenticated_user::AuthenticatedUser;
use crate::AppState;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::Response,
};
use matryx_surrealdb::repository::{
    media::MediaRepository, media_service::MediaService, 
    membership::MembershipRepository, room::RoomRepository,
};
use serde::Deserialize;
use std::sync::Arc;

/// Query parameters for authenticated download
#[derive(Debug, Deserialize)]
pub struct DownloadQuery {
    timeout_ms: Option<u64>,
}

/// Inline content types allowed for Content-Disposition: inline
const INLINE_CONTENT_TYPES: &[&str] = &[
    "text/css", "text/plain", "text/csv",
    "application/json", "application/ld+json",
    "image/jpeg", "image/gif", "image/png", "image/apng", 
    "image/webp", "image/avif",
    "video/mp4", "video/webm", "video/ogg", "video/quicktime",
    "audio/mp4", "audio/webm", "audio/aac", "audio/mpeg", 
    "audio/ogg", "audio/wave", "audio/wav", "audio/x-wav", 
    "audio/x-pn-wav", "audio/flac", "audio/x-flac",
];

/// Determine Content-Disposition header value
fn content_disposition(content_type: &str, filename: Option<&str>) -> String {
    let disposition = if INLINE_CONTENT_TYPES.contains(&content_type) {
        "inline"
    } else {
        "attachment"
    };
    
    if let Some(name) = filename {
        format!("{}; filename=\"{}\"", disposition, name)
    } else {
        disposition.to_string()
    }
}

/// GET /_matrix/client/v1/media/download/{serverName}/{mediaId}
pub async fn get(
    State(state): State<AppState>,
    Path((server_name, media_id)): Path<(String, String)>,
    Query(query): Query<DownloadQuery>,
    user: AuthenticatedUser,
) -> Result<Response<Body>, StatusCode> {
    // Apply timeout (default 20s, max 120s)
    let timeout_ms = query.timeout_ms.unwrap_or(20000).min(120000);
    let timeout_duration = std::time::Duration::from_millis(timeout_ms);

    // Create MediaService instance
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));

    let media_service = MediaService::new(media_repo, room_repo, membership_repo)
        .with_federation_client(
            state.federation_media_client.clone(),
            state.homeserver_name.clone(),
        );

    // Download with timeout
    let download_result = tokio::time::timeout(
        timeout_duration,
        media_service.download_media(&media_id, &server_name, &user.user_id)
    )
    .await
    .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
    .map_err(|_| StatusCode::NOT_FOUND)?;

    // Build response body
    let body = Body::from(download_result.content);

    // Calculate Content-Disposition
    let disposition = content_disposition(
        &download_result.content_type,
        download_result.filename.as_deref()
    );

    // Build response with required headers
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, download_result.content_type)
        .header(header::CONTENT_LENGTH, download_result.content_length.to_string())
        .header(header::CONTENT_DISPOSITION, disposition)
        .header(
            "Content-Security-Policy",
            "sandbox; default-src 'none'; script-src 'none'; plugin-types application/pdf; style-src 'unsafe-inline'; object-src 'self';"
        )
        .header("Cross-Origin-Resource-Policy", "cross-origin")
        .body(body)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
```

**Key Changes**:
1. Import `AuthenticatedUser` and `Query` extractors
2. Add `DownloadQuery` struct for `timeout_ms` parameter
3. Add `INLINE_CONTENT_TYPES` constant
4. Add `content_disposition()` helper function
5. Use `tokio::time::timeout` for timeout handling
6. Call `MediaService::download_media()` with `user.user_id`
7. Return `Response<Body>` with binary content (NOT JSON)
8. Include all required security headers

#### 2. `packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/by_file_name.rs`

**Current**: 13-line stub returning JSON  
**Action**: Replace with filename-override handler

**Implementation Pattern**:
```rust
use crate::auth::authenticated_user::AuthenticatedUser;
use crate::AppState;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::Response,
};
use matryx_surrealdb::repository::{
    media::MediaRepository, media_service::MediaService,
    membership::MembershipRepository, room::RoomRepository,
};
use serde::Deserialize;
use std::sync::Arc;

// Re-use types from parent module
use super::by_media_id::{DownloadQuery, INLINE_CONTENT_TYPES, content_disposition};

/// GET /_matrix/client/v1/media/download/{serverName}/{mediaId}/{fileName}
pub async fn get(
    State(state): State<AppState>,
    Path((server_name, media_id, file_name)): Path<(String, String, String)>,
    Query(query): Query<DownloadQuery>,
    user: AuthenticatedUser,
) -> Result<Response<Body>, StatusCode> {
    // Apply timeout (default 20s, max 120s)
    let timeout_ms = query.timeout_ms.unwrap_or(20000).min(120000);
    let timeout_duration = std::time::Duration::from_millis(timeout_ms);

    // Create MediaService instance
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));

    let media_service = MediaService::new(media_repo, room_repo, membership_repo)
        .with_federation_client(
            state.federation_media_client.clone(),
            state.homeserver_name.clone(),
        );

    // Download with timeout
    let download_result = tokio::time::timeout(
        timeout_duration,
        media_service.download_media(&media_id, &server_name, &user.user_id)
    )
    .await
    .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
    .map_err(|_| StatusCode::NOT_FOUND)?;

    // Build response body
    let body = Body::from(download_result.content);

    // IMPORTANT: Use file_name from path parameter (overrides stored name)
    let disposition = content_disposition(
        &download_result.content_type,
        Some(&file_name)
    );

    // Build response with required headers
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, download_result.content_type)
        .header(header::CONTENT_LENGTH, download_result.content_length.to_string())
        .header(header::CONTENT_DISPOSITION, disposition)
        .header(
            "Content-Security-Policy",
            "sandbox; default-src 'none'; script-src 'none'; plugin-types application/pdf; style-src 'unsafe-inline'; object-src 'self';"
        )
        .header("Cross-Origin-Resource-Policy", "cross-origin")
        .body(body)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
```

**Key Changes**:
1. Extract `file_name` from path parameters
2. **Force** `file_name` in Content-Disposition (do NOT use stored filename)
3. Otherwise identical to base download handler

#### 3. `packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/mod.rs`

**Current**: Exports sub-modules  
**Action**: Export both handlers and shared types

**Implementation**:
```rust
pub mod by_file_name;

// Re-export for use in by_file_name handler
pub use by_media_id::{DownloadQuery, INLINE_CONTENT_TYPES, content_disposition};

// Main download handler (without filename override)
mod by_media_id {
    // ... implementation from section 1 above
}

pub use by_media_id::get;
```

### MediaService Integration

The handlers use [MediaService](../packages/surrealdb/src/repository/media_service.rs#L212-L248) which provides:

```rust
pub async fn download_media(
    &self,
    media_id: &str,
    server_name: &str,
    requesting_user: &str,
) -> Result<MediaDownloadResult, MediaError>

pub struct MediaDownloadResult {
    pub content: Vec<u8>,
    pub content_type: String,
    pub content_length: u64,
    pub filename: Option<String>,
}
```

**Features**:
- ✅ Access validation via room membership
- ✅ Quarantine checking
- ✅ Federation support (automatic remote media download)
- ✅ Timeout handling (caller's responsibility via tokio::time::timeout)

### Authentication Flow

Routes in [main.rs](../packages/server/src/main.rs#L477-L478) are under `create_client_routes()` which applies `require_auth_middleware`. The `AuthenticatedUser` extractor:

1. Extracts `Authorization: Bearer <token>` header
2. Validates JWT token via `SessionService`
3. Checks user exists and is active in database
4. Provides `user.user_id` for MediaService access control

### Error Responses

Map MediaError to HTTP status codes:

| MediaError | HTTP Status | Error Code |
|------------|-------------|------------|
| NotFound | 404 | M_NOT_FOUND |
| NotYetUploaded | 504 | M_NOT_YET_UPLOADED |
| TooLarge | 502 | M_TOO_LARGE |
| AccessDenied | 403 | M_FORBIDDEN |
| Timeout (tokio) | 504 | M_NOT_YET_UPLOADED |

## Definition of Done

### Functional Requirements
1. ✅ Both endpoints return binary content (not JSON) with proper Content-Type
2. ✅ Content-Disposition header uses correct `inline`/`attachment` logic based on MIME type
3. ✅ Content-Disposition includes filename when available (path param overrides stored name)
4. ✅ Security headers (CSP, CORP) included on all responses
5. ✅ timeout_ms query parameter works (default 20s, max 120s)
6. ✅ Authentication required (401 if missing/invalid token)
7. ✅ Access control enforced (403 if user lacks room membership for media)
8. ✅ Remote media downloads via federation (if serverName ≠ homeserver_name)

### Code Quality
1. ✅ No compilation errors in modified files
2. ✅ Follows existing code patterns from [v1/download.rs](../packages/server/src/_matrix/media/v1/download.rs)
3. ✅ Uses MediaService for all media operations (no direct DB access)
4. ✅ Proper error handling with appropriate HTTP status codes

### Verification Commands

```bash
# Compile check
cargo build -p matryx_server

# Endpoint availability check
curl -I -H "Authorization: Bearer <valid_token>" \
  http://localhost:8008/_matrix/client/v1/media/download/example.com/abc123

# Security headers check
curl -i -H "Authorization: Bearer <valid_token>" \
  http://localhost:8008/_matrix/client/v1/media/download/example.com/abc123 | grep -E "(Content-Security-Policy|Cross-Origin-Resource-Policy|Content-Disposition)"

# Filename override check
curl -I -H "Authorization: Bearer <valid_token>" \
  http://localhost:8008/_matrix/client/v1/media/download/example.com/abc123/myfile.pdf | grep "Content-Disposition.*myfile.pdf"

# Unauthenticated request should fail
curl -i http://localhost:8008/_matrix/client/v1/media/download/example.com/abc123
# Expected: 401 Unauthorized
```

## References

### Matrix Specification
- [authed-content-repo.yaml](../tmp/matrix-spec/data/api/client-server/authed-content-repo.yaml) - OpenAPI spec for v1 authenticated endpoints
- [content_repo.md](../tmp/matrix-spec/content/client-server-api/modules/content_repo.md) - Content repository specification including inline content security

### Existing Implementation
- [packages/server/src/_matrix/media/v1/download.rs](../packages/server/src/_matrix/media/v1/download.rs) - Working authenticated download reference
- [packages/server/src/_matrix/media/v3/download/by_server_name/by_media_id/mod.rs](../packages/server/src/_matrix/media/v3/download/by_server_name/by_media_id/mod.rs) - v3 download with multipart (different response format)
- [packages/surrealdb/src/repository/media_service.rs](../packages/surrealdb/src/repository/media_service.rs) - MediaService implementation

### Infrastructure
- [packages/server/src/auth/authenticated_user.rs](../packages/server/src/auth/authenticated_user.rs) - AuthenticatedUser extractor
- [packages/server/src/main.rs](../packages/server/src/main.rs#L477-L478) - Route registration

## Notes

### Key Differences from v3 Endpoints

| Aspect | v3 (/media/v3/download) | v1 (/client/v1/media/download) |
|--------|-------------------------|--------------------------------|
| Authentication | Optional | **Required** |
| Response Format | multipart/mixed | **Binary content** |
| Content-Disposition | Optional | **Required (v1.12+)** |
| Security Headers | Recommended | **Required** |
| Spec Status | Deprecated | Current standard |

### Design Decisions

1. **Code Reuse**: Share `DownloadQuery`, `INLINE_CONTENT_TYPES`, and `content_disposition()` between both handlers to avoid duplication
2. **Timeout Strategy**: Use `tokio::time::timeout` wrapper (same as v3 implementation) rather than passing timeout to MediaService
3. **Error Mapping**: Use simple `StatusCode` returns instead of MatrixError for consistency with existing v1 handlers
4. **Header Values**: Use exact string literals from spec (no variables) for security headers to prevent misconfiguration

### Implementation Notes

- Routes are already registered in main.rs, no router changes needed
- Authentication is handled by middleware + `AuthenticatedUser` extractor
- MediaService handles both local and federated media transparently
- Content-Type should be preserved from original upload (MediaService handles this)
- Binary response uses `Body::from(Vec<u8>)` (Axum pattern)
