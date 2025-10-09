# SPEC_MEDIA_07: Add Deprecation Warnings and Migration Path

## Status
Enhancement - Spec Compliance

## Description
Matrix v1.11 deprecated unauthenticated media endpoints in favor of authenticated ones. The server should help clients migrate by providing clear deprecation warnings.

## Spec Context

### Deprecation Timeline (from spec)
- **v1.11 (current)**: New authenticated endpoints introduced, old ones deprecated
- **v1.12 (expected July-Sep 2024)**: Servers SHOULD "freeze" unauthenticated endpoints
  - Media uploaded before freeze: accessible via old endpoints
  - Media uploaded after freeze: ONLY accessible via new endpoints

### Deprecated Endpoints
All at `/_matrix/media/v3/*`:
- `GET /download/{serverName}/{mediaId}`
- `GET /download/{serverName}/{mediaId}/{fileName}`
- `GET /thumbnail/{serverName}/{mediaId}`
- `GET /preview_url`
- `GET /config`

### New Endpoints
All at `/_matrix/client/v1/media/*`:
- `GET /download/{serverName}/{mediaId}` (authenticated)
- `GET /download/{serverName}/{mediaId}/{fileName}` (authenticated)
- `GET /thumbnail/{serverName}/{mediaId}` (authenticated)
- `GET /preview_url` (authenticated)
- `GET /config` (authenticated)

### Upload Endpoints
**Not yet migrated** (expected in future spec version):
- `POST /_matrix/media/v3/upload`
- `PUT /_matrix/media/v3/upload/{serverName}/{mediaId}`
- `POST /_matrix/media/v1/create`

## Implementation Tasks

### 1. Add Deprecation Headers

Add to all deprecated v3 endpoints:

```rust
// In each deprecated endpoint handler
Response::builder()
    .header("Deprecation", "true")
    .header("Sunset", "Wed, 01 Sep 2024 00:00:00 GMT")  // Update to actual v1.12 date
    .header("Link", r#"<https://spec.matrix.org/v1.11/client-server-api/#content-repository>; rel="deprecation""#)
    .header("X-Matrix-Deprecated-Endpoint", "Use /_matrix/client/v1/media/* instead")
    // ... other headers
```

### 2. Add Warning Logs

```rust
// Log when deprecated endpoints are used
tracing::warn!(
    endpoint = "GET /_matrix/media/v3/download/{}/{}",
    user = user_id,
    "Deprecated endpoint used - client should migrate to /_matrix/client/v1/media/download"
);
```

### 3. Track Freeze Date Configuration

```rust
// In ServerConfig
pub struct MediaConfig {
    pub freeze_date: Option<DateTime<Utc>>,
    pub freeze_enabled: bool,
}

impl MediaConfig {
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

### 4. Implement Freeze Logic

```rust
// In deprecated download endpoints
pub async fn get(
    State(state): State<AppState>,
    Path((server_name, media_id)): Path<(String, String)>,
) -> Result<Response<Body>, StatusCode> {
    let media_service = MediaService::new(/* ... */);
    
    // Check if media is frozen
    let media_info = media_service
        .get_media_info(&media_id, &server_name)
        .await?;
    
    if state.config.media.is_frozen(media_info.uploaded_at) {
        // Media uploaded after freeze - return 404 to force migration
        tracing::info!(
            media_id = media_id,
            "Blocking access to post-freeze media on deprecated endpoint"
        );
        
        return Err(StatusCode::NOT_FOUND);
    }
    
    // Allow access to pre-freeze media
    // ... normal download logic
}
```

### 5. Exempt IdP Icons (Spec Requirement)

```rust
// IdP icons for SSO should NOT be frozen
if media_info.is_idp_icon {
    // Always allow, even if after freeze date
    tracing::debug!("Allowing IdP icon access despite freeze");
    // ... proceed with download
}
```

### 6. Add Metrics

Track deprecated endpoint usage:

```rust
// Metrics to track
- deprecated_media_endpoint_calls (counter, by endpoint)
- frozen_media_blocks (counter)
- pre_freeze_media_access (counter)
```

### 7. Migration Documentation

Create `docs/media_migration.md`:

```markdown
# Media Endpoint Migration Guide

## For Clients

The media endpoints have moved from `/_matrix/media/v3/*` to `/_matrix/client/v1/media/*`.

### Before (deprecated):
GET /_matrix/media/v3/download/{serverName}/{mediaId}

### After (authenticated):
GET /_matrix/client/v1/media/download/{serverName}/{mediaId}
Authorization: Bearer <access_token>

### Key Changes:
- All download/thumbnail endpoints now require authentication
- Upload endpoints will migrate in a future version
- Old endpoints will be frozen after [DATE]

## For Administrators

### Configuration

```yaml
media:
  freeze_enabled: true
  freeze_date: "2024-09-01T00:00:00Z"
```

### Monitoring

Check logs for deprecated endpoint usage:
```bash
grep "Deprecated endpoint used" homeserver.log
```
```

## Testing

### 1. Test Deprecation Headers
```bash
curl -I http://localhost:8008/_matrix/media/v3/download/example.com/abc123

# Should include:
# Deprecation: true
# Sunset: Wed, 01 Sep 2024 00:00:00 GMT
# X-Matrix-Deprecated-Endpoint: Use /_matrix/client/v1/media/* instead
```

### 2. Test Freeze Logic
```bash
# Upload media before freeze
# Should be accessible on old endpoints

# Upload media after freeze
# Should be 404 on old endpoints
# Should work on new endpoints
```

### 3. Test IdP Icon Exemption
```bash
# IdP icons should always work, even after freeze
curl http://localhost:8008/_matrix/media/v3/download/example.com/idp-icon-123
```

## Configuration Example

```toml
# config.toml
[media]
freeze_enabled = false  # Set to true to enable freeze
freeze_date = "2024-09-01T00:00:00Z"  # When to freeze unauthenticated access

# For testing
freeze_enabled = true
freeze_date = "2024-01-01T00:00:00Z"  # Past date to test freeze behavior
```

## References
- Spec: `/tmp/matrix-spec/content/client-server-api/modules/content_repo.md` lines 50-76
- Deprecation best practices: https://tools.ietf.org/id/draft-dalal-deprecation-header-01.html
