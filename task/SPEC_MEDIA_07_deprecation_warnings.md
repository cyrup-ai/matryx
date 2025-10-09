# SPEC_MEDIA_07: Add Deprecation Warnings and Migration Path

## Status
**NOT STARTED - 0/10 - COMPLETE NON-IMPLEMENTATION**

## Critical Issues

**NOTHING has been implemented from this specification.** All 6 core requirements are missing:

1. ❌ Database schema extension (is_idp_icon field)
2. ❌ Server configuration (MediaConfig struct)  
3. ❌ Deprecation headers (0 of 5 endpoints)
4. ❌ Warning logs (0 of 5 endpoints)
5. ❌ Freeze logic (0 of 3 endpoints)
6. ❌ IdP icon admin endpoint

## Required Implementation

### STEP 1: Database Schema - Add is_idp_icon Field

**File**: `/Volumes/samsung_t9/maxtryx/packages/surrealdb/src/repository/media.rs`
**Line**: After line 24 (after `quarantined_at`)

Add this field to the MediaInfo struct:

```rust
/// Whether this media is an IdP icon for SSO (exempt from freeze)
#[serde(default)]
pub is_idp_icon: Option<bool>,
```

**Current Status**: Field does not exist. Only quarantine fields are present.

---

### STEP 2: Server Configuration - Add MediaConfig

**File**: `/Volumes/samsung_t9/maxtryx/packages/server/src/config/server_config.rs`

**Change 2A** - Add MediaConfig struct (before line 155, before ServerConfig struct):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaConfig {
    /// Enable the freeze mechanism for deprecated media endpoints
    pub freeze_enabled: bool,
    /// Timestamp when the freeze takes effect
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

**Change 2B** - Add field to ServerConfig struct (around line 165):

```rust
pub struct ServerConfig {
    pub homeserver_name: String,
    pub federation_port: u16,
    pub media_base_url: String,
    // ... existing fields ...
    pub captcha: CaptchaConfig,
    pub media_config: MediaConfig,  // ← ADD THIS LINE
}
```

**Change 2C** - Initialize in ServerConfig::init() (around line 306, before the closing config):

```rust
let config = ServerConfig {
    homeserver_name: homeserver_name.clone(),
    // ... existing fields ...
    captcha: CaptchaConfig::from_env(),
    media_config: MediaConfig::from_env(),  // ← ADD THIS LINE
};
```

**Current Status**: MediaConfig does not exist. Only MediaConfigResponse exists (different type for API).

---

### STEP 3: Add Deprecation Headers to ALL 5 v3 Endpoints

Each v3 endpoint must return these HTTP headers on EVERY response:

```rust
.header("Deprecation", "true")
.header("Sunset", "Wed, 01 Sep 2024 00:00:00 GMT")
.header("Link", r#"<https://spec.matrix.org/v1.11/client-server-api/#content-repository>; rel="deprecation""#)
.header("X-Matrix-Deprecated-Endpoint", "Use /_matrix/client/v1/media/* instead")
```

**Endpoints requiring changes:**

1. `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v3/download/by_server_name/by_media_id/mod.rs`
   - Currently returns Response, add headers before returning (line ~90)
   
2. `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v3/download/by_server_name/by_media_id/by_file_name.rs`
   - Currently returns Response<Body>, add headers to Response::builder() (line ~38)
   
3. `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs`
   - Currently returns Json<Value> - MUST convert to Response with headers
   
4. `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v3/preview_url.rs`
   - Currently returns Json<PreviewResponse> - MUST convert to Response with headers
   
5. `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v3/config.rs`
   - Currently returns Json<MediaConfigResponse> - MUST convert to Response with headers

**Current Status**: ZERO deprecation headers on any endpoint.

---

### STEP 4: Add Warning Logs to ALL 5 v3 Endpoints

At the START of each handler function, add:

```rust
use tracing::warn;

warn!(
    endpoint = "GET /_matrix/media/v3/[path]",
    "Deprecated endpoint accessed - clients should migrate to /_matrix/client/v1/media/*"
);
```

Replace `[path]` with the actual endpoint path.

**Current Status**: ZERO warning logs on any endpoint.

---

### STEP 5: Add Freeze Logic to 3 Media-Serving Endpoints

**Required for:**
- `v3/download/.../mod.rs` (download by media_id)
- `v3/download/.../by_file_name.rs` (download with filename)
- `v3/thumbnail/.../by_media_id.rs` (thumbnail)

**NOT required for:**
- `v3/preview_url.rs` (no media_id)
- `v3/config.rs` (no media_id)

Add this logic AFTER parameter validation, BEFORE MediaService calls:

```rust
// Check freeze status
if state.config.media_config.freeze_enabled {
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    
    if let Ok(Some(media_info)) = media_repo.get_media_info(&media_id, &server_name).await {
        let is_idp_icon = media_info.is_idp_icon.unwrap_or(false);
        
        if !is_idp_icon && state.config.media_config.is_frozen(media_info.created_at) {
            warn!(
                media_id = media_id,
                uploaded_at = %media_info.created_at,
                "Blocking post-freeze media on deprecated endpoint"
            );
            
            return Err(MatrixError::NotFound);
        }
    }
}
```

**Current Status**: ZERO freeze logic on any endpoint.

---

### STEP 6: Create Admin Endpoint for IdP Icon Management

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

/// POST /_matrix/client/v1/admin/media/mark_idp_icon
/// Admin endpoint to mark/unmark media as IdP icon (exempt from freeze)
pub async fn mark_idp_icon(
    State(state): State<AppState>,
    Json(req): Json<MarkIdpIconRequest>,
) -> Result<Json<MarkIdpIconResponse>, MatrixError> {
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));
    
    let mut media_info = media_repo
        .get_media_info(&req.media_id, &req.server_name)
        .await?
        .ok_or(MatrixError::NotFound)?;
    
    media_info.is_idp_icon = Some(req.is_idp_icon);
    
    media_repo
        .store_media_info(&req.media_id, &req.server_name, &media_info)
        .await?;
    
    Ok(Json(MarkIdpIconResponse { success: true }))
}
```

**Also required**: Hook this into the router in server initialization.

**Current Status**: File does not exist.

---

## Implementation Priority

1. **FIRST**: Steps 1-2 (database + config) - Foundation for everything else
2. **SECOND**: Step 3 (deprecation headers) - Most visible to clients
3. **THIRD**: Step 4 (warning logs) - Server-side visibility
4. **FOURTH**: Step 5 (freeze logic) - Enforcement mechanism
5. **FIFTH**: Step 6 (admin endpoint) - IdP icon management

---

## Environment Variables

Add to documentation:

```bash
# Media endpoint freeze configuration
MEDIA_FREEZE_ENABLED=false                    # Set to true to enable freeze
MEDIA_FREEZE_DATE=2024-09-01T00:00:00Z       # ISO 8601 timestamp
```

---

## Verification Checklist

- [ ] MediaInfo has is_idp_icon field
- [ ] ServerConfig has MediaConfig struct and media_config field
- [ ] MediaConfig loads from environment variables
- [ ] All 5 v3 endpoints return deprecation headers
- [ ] All 5 v3 endpoints log warning when accessed
- [ ] 3 media-serving endpoints check freeze status
- [ ] IdP icons are exempt from freeze
- [ ] Admin endpoint exists for marking IdP icons
- [ ] Admin endpoint is registered in router
