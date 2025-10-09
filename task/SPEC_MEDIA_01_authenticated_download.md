# SPEC_MEDIA_01: Implement Authenticated Download Endpoints

## Status
**CRITICAL - 0% Implementation Complete** - Stub handlers must be completely replaced

## Current Critical Issues

### Implementation Gap: Complete Replacement Required

Both endpoint handlers are returning **JSON stubs** instead of implementing the Matrix specification requirements. Current code:

**File**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/mod.rs`
```rust
// WRONG: Returns JSON instead of binary content
pub async fn get(
    Path((_server_name, _media_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "content_type": "image/jpeg",
        "content_disposition": "attachment; filename=example.jpg"
    })))
}
```

**File**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/by_file_name.rs`
```rust
// WRONG: Returns JSON instead of binary content
pub async fn get(
    Path((_server_name, _media_id, _file_name)): Path<(String, String, String)>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "content_type": "image/jpeg",
        "content_disposition": "attachment; filename=example.jpg"
    })))
}
```

## Required Implementation

### 1. Endpoint: `GET /_matrix/client/v1/media/download/{serverName}/{mediaId}`

**File**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/mod.rs`

**Requirements**:
- ✅ Use `AuthenticatedUser` extractor to get user context
- ✅ Implement `timeout_ms` query parameter (default: 20s, max: 120s)
- ✅ Call `MediaService::download_media(&media_id, &server_name, &user.user_id)`
- ✅ Return `Response<Body>` with **binary content** (NOT JSON)
- ✅ Set required headers:
  - `Content-Type`: From MediaService result
  - `Content-Length`: From MediaService result
  - `Content-Disposition`: Use inline/attachment logic based on MIME type
  - `Content-Security-Policy`: `"sandbox; default-src 'none'; script-src 'none'; plugin-types application/pdf; style-src 'unsafe-inline'; object-src 'self';"`
  - `Cross-Origin-Resource-Policy`: `"cross-origin"`
- ✅ Implement MIME type checking for Content-Disposition (inline vs attachment)

**Inline MIME Types** (35 allowed):
```rust
const INLINE_CONTENT_TYPES: &[&str] = &[
    "text/css", "text/plain", "text/csv",
    "application/json", "application/ld+json",
    "image/jpeg", "image/gif", "image/png", "image/apng", "image/webp", "image/avif",
    "video/mp4", "video/webm", "video/ogg", "video/quicktime",
    "audio/mp4", "audio/webm", "audio/aac", "audio/mpeg", "audio/ogg",
    "audio/wave", "audio/wav", "audio/x-wav", "audio/x-pn-wav",
    "audio/flac", "audio/x-flac",
];
```

**Content-Disposition Logic**:
- If MIME type in `INLINE_CONTENT_TYPES`: `"inline; filename=\"...\""` or `"inline"` if no filename
- Otherwise: `"attachment; filename=\"...\""` or `"attachment"` if no filename

**Reference Implementation**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v1/download.rs` (lines 1-65) shows MediaService integration pattern but **lacks security headers and MIME checking** - don't copy those gaps.

### 2. Endpoint: `GET /_matrix/client/v1/media/download/{serverName}/{mediaId}/{fileName}`

**File**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/by_file_name.rs`

**Requirements**:
- Same as endpoint #1, EXCEPT:
- ✅ **MUST** use `{fileName}` path parameter in Content-Disposition header (overrides stored filename)
- ✅ Extract `file_name` from `Path<(String, String, String)>`
- ✅ Force filename in Content-Disposition: `content_disposition(&content_type, Some(&file_name))`

### 3. Critical Code Patterns Required

#### MediaService Integration Pattern:
```rust
use matryx_surrealdb::repository::{
    media::MediaRepository, media_service::MediaService,
    membership::MembershipRepository, room::RoomRepository,
};
use std::sync::Arc;

// Create MediaService with repositories
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
```

#### Binary Response Pattern:
```rust
use axum::{body::Body, http::header, response::Response};

let body = Body::from(download_result.content);

Response::builder()
    .status(StatusCode::OK)
    .header(header::CONTENT_TYPE, download_result.content_type)
    .header(header::CONTENT_LENGTH, download_result.content_length.to_string())
    .header(header::CONTENT_DISPOSITION, disposition)
    .header("Content-Security-Policy", "sandbox; default-src 'none'; script-src 'none'; plugin-types application/pdf; style-src 'unsafe-inline'; object-src 'self';")
    .header("Cross-Origin-Resource-Policy", "cross-origin")
    .body(body)
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
```

#### Helper Function for Content-Disposition:
```rust
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
```

## Verification Required

After implementation, verify:

1. **Binary Content**: Response body contains actual file bytes, NOT JSON
2. **Security Headers**: All responses include CSP and CORP headers
3. **Content-Disposition**: 
   - JPEG/PNG/MP4 files get `"inline; filename=..."`
   - ZIP/PDF files get `"attachment; filename=..."`
   - Filename parameter override works in second endpoint
4. **Authentication**: Unauthenticated requests return 401
5. **Access Control**: User without room membership gets 403
6. **Timeout**: Query parameter works (test with `?timeout_ms=5000`)
7. **Federation**: Remote media downloads work when server_name ≠ homeserver

## Build Verification

```bash
cargo build -p matryx_server
```

Must compile without errors after implementation.

## Matrix Specification References

- **API Spec**: `/Volumes/samsung_t9/maxtryx/tmp/matrix-spec/data/api/client-server/authed-content-repo.yaml` (lines 18-147)
- **Content Security**: `/Volumes/samsung_t9/maxtryx/tmp/matrix-spec/content/client-server-api/modules/content_repo.md` (lines 15-21, 168-211)
- **MediaService**: `/Volumes/samsung_t9/maxtryx/packages/surrealdb/src/repository/media_service.rs` (lines 212-248)

## Notes

- Routes are already registered in `/Volumes/samsung_t9/maxtryx/packages/server/src/main.rs` (lines 382-383)
- Authentication middleware is already applied via `require_auth_middleware` (line 349)
- Empty file `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/by_media_id.rs` should likely be removed (code belongs in mod.rs)
