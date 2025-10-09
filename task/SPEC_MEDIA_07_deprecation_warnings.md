# SPEC_MEDIA_07: Add Deprecation Warnings and Migration Path

## Status
Enhancement - Spec Compliance

## Description
Matrix v1.11 deprecated unauthenticated media endpoints in favor of authenticated ones. The server must help clients migrate by providing clear deprecation warnings and implementing a "freeze" mechanism that prevents newly-uploaded media from being accessed via deprecated endpoints.

## Matrix Specification Context

### Deprecation Timeline

Per [Matrix v1.11 Client-Server API - Content Repository](../../packages/server/tmp/matrix-spec/content/client-server-api/modules/content_repo.md):

- **v1.11 (released)**: New authenticated endpoints introduced at `/_matrix/client/v1/media/*`, old unauthenticated endpoints at `/_matrix/media/v3/*` deprecated
- **v1.12 (expected Q3 2024)**: Servers SHOULD "freeze" unauthenticated endpoints:
  - Media uploaded **before** freeze date → accessible via old endpoints
  - Media uploaded **after** freeze date → ONLY accessible via new authenticated endpoints
  - Remote media freeze based on cache population date, not original upload date

### Deprecated Endpoints (v3)

Located in `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v3/`:
- `GET /download/{serverName}/{mediaId}` - [mod.rs](../../packages/server/src/_matrix/media/v3/download/by_server_name/by_media_id/mod.rs)
- `GET /download/{serverName}/{mediaId}/{fileName}` - [by_file_name.rs](../../packages/server/src/_matrix/media/v3/download/by_server_name/by_media_id/by_file_name.rs)
- `GET /thumbnail/{serverName}/{mediaId}` - [by_media_id.rs](../../packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs)
- `GET /preview_url` - [preview_url.rs](../../packages/server/src/_matrix/media/v3/preview_url.rs)
- `GET /config` - [config.rs](../../packages/server/src/_matrix/media/v3/config.rs)

### New Authenticated Endpoints (v1)

Located in `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v1/media/`:
- `GET /download/{serverName}/{mediaId}` - [by_media_id.rs](../../packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/by_media_id.rs)
- `GET /download/{serverName}/{mediaId}/{fileName}` - [by_file_name.rs](../../packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/by_file_name.rs)
- `GET /thumbnail/{serverName}/{mediaId}` - [by_media_id.rs](../../packages/server/src/_matrix/client/v1/media/thumbnail/by_server_name/by_media_id.rs)
- `GET /preview_url` - [preview_url.rs](../../packages/server/src/_matrix/client/v1/media/preview_url.rs)
- `GET /config` - [config.rs](../../packages/server/src/_matrix/client/v1/media/config.rs)

**Note**: Upload endpoints (`/upload`, `/create`) are NOT YET migrated per spec.

### IdP Icon Exemption

Per [m.login.sso flow schema](../../packages/server/tmp/matrix-spec/data/api/client-server/definitions/sso_login_flow.yaml):

> Servers SHOULD ensure media used for IdP icons is excluded from the freeze. See the m.login.sso flow schema for details.

**Rationale**: Identity Provider (IdP) icons must be accessible to unauthenticated users during the SSO login flow, before they have an access token. These icons are referenced in the `identity_providers` array of the `m.login.sso` response with `mxc://` URIs.

## Codebase Architecture

### Media Data Flow

```
HTTP Request → Endpoint Handler → MediaService → MediaRepository → SurrealDB
                     ↓
              AppState (has config)
                     ↓
              MediaConfig (NEW - freeze settings)
```

### Key Files & Structures

#### 1. MediaInfo Struct
**File**: `/Volumes/samsung_t9/maxtryx/packages/surrealdb/src/repository/media.rs`

```rust
pub struct MediaInfo {
    pub media_id: String,
    pub server_name: String,
    pub content_type: String,
    pub content_length: u64,
    pub upload_name: Option<String>,
    pub uploaded_by: String,
    pub created_at: DateTime<Utc>,  // ← Used for freeze date comparison
    pub expires_at: Option<DateTime<Utc>>,
    pub quarantined: Option<bool>,
    // ... quarantine fields
}
```

**Required Addition**:
```rust
/// Whether this media is an IdP icon (exempt from freeze)
#[serde(default)]
pub is_idp_icon: Option<bool>,
```

#### 2. ServerConfig
**File**: `/Volumes/samsung_t9/maxtryx/packages/server/src/config/server_config.rs`

Current structure at line ~155:
```rust
pub struct ServerConfig {
    pub homeserver_name: String,
    pub federation_port: u16,
    pub media_base_url: String,
    // ... other fields
    pub rate_limiting: RateLimitConfig,
    pub captcha: CaptchaConfig,
}
```

**Required Addition** (add before impl ServerConfig):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaConfig {
    /// Enable the freeze mechanism for deprecated media endpoints
    pub freeze_enabled: bool,
    /// Timestamp when the freeze takes effect (media uploaded after this is frozen)
    pub freeze_date: Option<DateTime<Utc>>,
}

impl Default for MediaConfig {
    fn default() -> Self {
        Self {
            freeze_enabled: false,
            freeze_date: None,
        }
    }
}

impl MediaConfig {
    pub fn from_env() -> Self {
        Self {
            freeze_enabled: env::var("MEDIA_FREEZE_ENABLED")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(false),
            freeze_date: env::var("MEDIA_FREEZE_DATE")
                .ok()
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
        }
    }
    
    /// Check if media uploaded at given time should be frozen
    pub fn is_frozen(&self, upload_date: DateTime<Utc>) -> bool {
        if !self.freeze_enabled {
            return false;
        }
        
        if let Some(freeze) = self.freeze_date {
            upload_date >= freeze
        } else {
            false
        }
    }
}
```

Then add to ServerConfig struct:
```rust
pub media_config: MediaConfig,
```

And in ServerConfig::init() at line ~206, add:
```rust
media_config: MediaConfig::from_env(),
```

#### 3. AppState
**File**: `/Volumes/samsung_t9/maxtryx/packages/server/src/state.rs`

AppState already has `config: &'static ServerConfig` (line 35), so media config is accessible via:
```rust
state.config.media_config.is_frozen(upload_date)
```

### MediaService Methods

**File**: `/Volumes/samsung_t9/maxtryx/packages/surrealdb/src/repository/media_service.rs`

Available methods:
- `get_media_info(&self, media_id: &str, server_name: &str) -> Result<Option<MediaInfo>, RepositoryError>` (line ~267)
- `download_media(&self, media_id: &str, server_name: &str, requesting_user: &str) -> Result<MediaDownloadResult, MediaError>` (line ~197)

## Implementation Plan

### Step 1: Extend Database Schema

**File**: `/Volumes/samsung_t9/maxtryx/packages/surrealdb/src/repository/media.rs`

Add `is_idp_icon` field to MediaInfo struct (line ~18):

```rust
pub struct MediaInfo {
    // ... existing fields ...
    #[serde(default)]
    pub quarantined_at: Option<DateTime<Utc>>,
    // NEW FIELD:
    /// Whether this media is an IdP icon for SSO (exempt from freeze)
    #[serde(default)]
    pub is_idp_icon: Option<bool>,
}
```

No migration needed - `#[serde(default)]` makes it backward compatible.

### Step 2: Add MediaConfig to Server Configuration

**File**: `/Volumes/samsung_t9/maxtryx/packages/server/src/config/server_config.rs`

1. Add MediaConfig struct before line 155 (see code above in "Required Addition")
2. Add `media_config: MediaConfig` field to ServerConfig struct (line ~165)
3. Initialize in ServerConfig::init() (line ~206): `media_config: MediaConfig::from_env(),`

### Step 3: Implement Deprecation Headers

All deprecated v3 endpoints need HTTP headers. **Pattern to follow**:

```rust
use axum::http::header;

// In endpoint handler, before returning Response:
let response = Response::builder()
    .status(StatusCode::OK)
    // Add deprecation headers:
    .header("Deprecation", "true")
    .header("Sunset", "Wed, 01 Sep 2024 00:00:00 GMT") // Adjust to actual v1.12 release
    .header("Link", r#"<https://spec.matrix.org/v1.11/client-server-api/#content-repository>; rel="deprecation""#)
    .header("X-Matrix-Deprecated-Endpoint", "Use /_matrix/client/v1/media/* instead")
    // ... existing headers ...
    .body(body)?;
```

**Apply to these files:**

1. `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v3/download/by_server_name/by_media_id/mod.rs` (line ~89-95)
2. `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v3/download/by_server_name/by_media_id/by_file_name.rs` (line ~38-50)
3. `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs` (convert to Response instead of Json)
4. `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v3/preview_url.rs` (convert to Response instead of Json)
5. `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v3/config.rs` (convert to Response instead of Json)

### Step 4: Add Warning Logs

Add to the start of each deprecated endpoint handler:

```rust
use tracing::warn;

warn!(
    endpoint = "GET /_matrix/media/v3/download/{}/{}",
    media_id = media_id,
    server_name = server_name,
    "Deprecated endpoint used - client should migrate to /_matrix/client/v1/media/download"
);
```

### Step 5: Implement Freeze Logic

Only for endpoints that serve media by ID (download, thumbnail). **NOT** for preview_url or config.

**Files to modify:**
- `v3/download/by_server_name/by_media_id/mod.rs`
- `v3/download/by_server_name/by_media_id/by_file_name.rs`
- `v3/thumbnail/by_server_name/by_media_id.rs`

**Pattern** (insert after parameter validation, before MediaService calls):

```rust
// Check freeze status if enabled
if state.config.media_config.freeze_enabled {
    // Get media info to check upload date
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    
    if let Ok(Some(media_info)) = media_repo.get_media_info(&media_id, &server_name).await {
        // Check if this is an IdP icon (exempt from freeze)
        let is_idp_icon = media_info.is_idp_icon.unwrap_or(false);
        
        if !is_idp_icon && state.config.media_config.is_frozen(media_info.created_at) {
            warn!(
                media_id = media_id,
                server_name = server_name,
                uploaded_at = %media_info.created_at,
                "Blocking access to post-freeze media on deprecated endpoint"
            );
            
            return Err(MatrixError::NotFound); // or StatusCode::NOT_FOUND depending on endpoint
        }
    }
}

// ... continue with normal download logic ...
```

### Step 6: Add IdP Icon Support (Admin API)

**NEW FILE**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v1/admin/media_idp_icons.rs`

```rust
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use crate::{AppState, error::MatrixError};
use matryx_surrealdb::repository::media::MediaRepository;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct MarkIdpIconRequest {
    pub media_id: String,
    pub server_name: String,
    pub is_idp_icon: bool,
}

#[derive(Serialize)]
pub struct MarkIdpIconResponse {
    pub success: bool,
}

/// Admin endpoint to mark/unmark media as IdP icon
pub async fn mark_idp_icon(
    State(state): State<AppState>,
    Json(req): Json<MarkIdpIconRequest>,
) -> Result<Json<MarkIdpIconResponse>, MatrixError> {
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    
    // Get existing media info
    let mut media_info = media_repo
        .get_media_info(&req.media_id, &req.server_name)
        .await?
        .ok_or(MatrixError::NotFound)?;
    
    // Update IdP icon status
    media_info.is_idp_icon = Some(req.is_idp_icon);
    
    // Store updated info
    media_repo
        .store_media_info(&req.media_id, &req.server_name, &media_info)
        .await?;
    
    Ok(Json(MarkIdpIconResponse { success: true }))
}
```

**Note**: Hook this into router in `main.rs` under admin routes.

## File-by-File Implementation Details

### File 1: media.rs (Database)
**Path**: `/Volumes/samsung_t9/maxtryx/packages/surrealdb/src/repository/media.rs`
**Line**: 18 (in MediaInfo struct)

**Add**:
```rust
/// Whether this media is an IdP icon for SSO (exempt from freeze)
#[serde(default)]
pub is_idp_icon: Option<bool>,
```

### File 2: server_config.rs (Configuration)
**Path**: `/Volumes/samsung_t9/maxtryx/packages/server/src/config/server_config.rs`

**Location 1** (before line 155): Add MediaConfig struct (see Step 2)
**Location 2** (line ~165 in ServerConfig struct): Add field `pub media_config: MediaConfig,`
**Location 3** (line ~206 in ServerConfig::init()): Add `media_config: MediaConfig::from_env(),`

### File 3: v3/download/.../mod.rs
**Path**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v3/download/by_server_name/by_media_id/mod.rs`

**Changes**:
1. Add warning log at line 30 (after parameter validation)
2. Add freeze check at line 48 (before MediaService call)
3. Add deprecation headers at line 89 (in Response::builder())

Current response building (line 89):
```rust
let response = build_multipart_media_response(multipart_response)
    .map_err(|e| { ... })?;
```

**Change to**:
```rust
let mut response = build_multipart_media_response(multipart_response)
    .map_err(|e| { ... })?;

// Add deprecation headers
response.headers_mut().insert("Deprecation", "true".parse().unwrap());
response.headers_mut().insert("Sunset", "Wed, 01 Sep 2024 00:00:00 GMT".parse().unwrap());
response.headers_mut().insert("Link", 
    r#"<https://spec.matrix.org/v1.11/client-server-api/#content-repository>; rel="deprecation""#
    .parse().unwrap());
response.headers_mut().insert("X-Matrix-Deprecated-Endpoint", 
    "Use /_matrix/client/v1/media/* instead".parse().unwrap());
```

### File 4: v3/download/.../by_file_name.rs
**Path**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v3/download/by_server_name/by_media_id/by_file_name.rs`

**Changes**:
1. Add warning log after line 20
2. Add freeze check after line 24
3. Modify Response::builder() at line 38 to add deprecation headers

### File 5: v3/thumbnail/by_media_id.rs
**Path**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs`

**Current**: Returns `Json<Value>` (line 22)
**Change to**: Return `Response<Body>` with deprecation headers

**Changes**:
1. Add warning log at start of handler
2. Add freeze check before MediaService call
3. Convert Json response to Response with headers

### File 6: v3/preview_url.rs
**Path**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v3/preview_url.rs`

**Changes**:
1. Add warning log at start (line 43)
2. Convert `Json<PreviewResponse>` to `Response` with deprecation headers
3. **NO freeze logic** (no media_id to check)

### File 7: v3/config.rs
**Path**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v3/config.rs`

**Changes**:
1. Add warning log at start (line 18)
2. Convert `Json<MediaConfigResponse>` to `Response` with deprecation headers
3. **NO freeze logic** (no media_id to check)

## Environment Variables

Add to server configuration:

```bash
# Media endpoint freeze configuration
MEDIA_FREEZE_ENABLED=false                    # Set to true to enable freeze
MEDIA_FREEZE_DATE=2024-09-01T00:00:00Z       # ISO 8601 timestamp for freeze activation
```

## Example Usage

### Testing Deprecation Headers

```bash
# Should return deprecation headers
curl -I http://localhost:8008/_matrix/media/v3/download/example.com/abc123

# Expected headers:
# Deprecation: true
# Sunset: Wed, 01 Sep 2024 00:00:00 GMT
# Link: <https://spec.matrix.org/v1.11/client-server-api/#content-repository>; rel="deprecation"
# X-Matrix-Deprecated-Endpoint: Use /_matrix/client/v1/media/* instead
```

### Testing Freeze Logic

```bash
# Enable freeze with past date
export MEDIA_FREEZE_ENABLED=true
export MEDIA_FREEZE_DATE=2024-01-01T00:00:00Z

# Upload new media (will have created_at > freeze_date)
# Try to access via deprecated endpoint → 404
curl http://localhost:8008/_matrix/media/v3/download/example.com/new_media_id
# Should return 404 Not Found

# Access via new authenticated endpoint → works
curl -H "Authorization: Bearer <token>" \
  http://localhost:8008/_matrix/client/v1/media/download/example.com/new_media_id
# Should return media content
```

### Testing IdP Icon Exemption

```bash
# Mark media as IdP icon (admin endpoint)
curl -X POST http://localhost:8008/_matrix/client/v1/admin/media/mark_idp_icon \
  -H "Authorization: Bearer <admin_token>" \
  -H "Content-Type: application/json" \
  -d '{
    "media_id": "idp_icon_123",
    "server_name": "example.com",
    "is_idp_icon": true
  }'

# Access IdP icon via deprecated endpoint (even if after freeze) → works
curl http://localhost:8008/_matrix/media/v3/download/example.com/idp_icon_123
# Should return media content (not blocked by freeze)
```

## Definition of Done

The task is complete when:

1. **Configuration**:
   - MediaConfig struct exists in server_config.rs with freeze_enabled and freeze_date fields
   - MediaConfig is initialized from environment variables
   - MediaConfig is accessible via AppState.config.media_config

2. **Database Schema**:
   - MediaInfo struct has is_idp_icon field
   - Field is properly serialized/deserialized with default value

3. **Deprecation Headers**:
   - All 5 deprecated v3 endpoints return these headers on every response:
     - Deprecation: true
     - Sunset: <date>
     - Link: <spec URL>
     - X-Matrix-Deprecated-Endpoint: <migration message>

4. **Warning Logs**:
   - All 5 deprecated endpoints log warnings when accessed
   - Logs include endpoint path, media_id (if applicable), and migration message

5. **Freeze Logic**:
   - Download and thumbnail endpoints check media upload date against freeze_date
   - Media uploaded after freeze_date returns 404 on deprecated endpoints
   - Media uploaded before freeze_date works on deprecated endpoints
   - IdP icons (is_idp_icon=true) are exempt from freeze regardless of upload date

6. **IdP Icon Management**:
   - Admin can mark/unmark media as IdP icons
   - IdP icons accessible on deprecated endpoints even when frozen

The implementation does NOT require adding:
- Metrics/monitoring infrastructure
- Extensive error handling beyond existing patterns
- Additional endpoints beyond what's specified

## References

- Matrix v1.11 Spec: [Content Repository](../../packages/server/tmp/matrix-spec/content/client-server-api/modules/content_repo.md)
- SSO Login Flow: [sso_login.md](../../packages/server/tmp/matrix-spec/content/client-server-api/modules/sso_login.md)
- IdP Schema: [sso_login_flow.yaml](../../packages/server/tmp/matrix-spec/data/api/client-server/definitions/sso_login_flow.yaml)
- Deprecation Header Standard: [RFC 8594](https://tools.ietf.org/html/rfc8594)
- MediaInfo Struct: [media.rs](../../packages/surrealdb/src/repository/media.rs#L6)
- ServerConfig: [server_config.rs](../../packages/server/src/config/server_config.rs#L155)
- AppState: [state.rs](../../packages/server/src/state.rs#L28)
