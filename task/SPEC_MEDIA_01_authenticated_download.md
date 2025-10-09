# SPEC_MEDIA_01: Implement Authenticated Download Endpoints

## Status
Missing Implementation

## Description
The Matrix v1.11 specification introduced authenticated media download endpoints at `/_matrix/client/v1/media/*`. These are the new, required endpoints that replace the deprecated unauthenticated endpoints.

## Current State
- No implementation exists in `/packages/server/src/_matrix/media/` for v1 authenticated downloads
- Only deprecated v3 endpoints are implemented
- V1 download endpoints are missing entirely

## Spec Requirements

### Endpoint 1: `GET /_matrix/client/v1/media/download/{serverName}/{mediaId}`
**Path**: `packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id.rs`

**Requirements**:
- Requires authentication (access token)
- Returns binary content with proper headers
- Must include `Content-Type` header (from original upload or reasonably close)
- Must include `Content-Disposition` header (required in v1.12+):
  - `inline` if Content-Type is in allowed list
  - `attachment` otherwise
  - Include filename if available
- Must include security headers:
  - `Content-Security-Policy: sandbox; default-src 'none'; script-src 'none'; plugin-types application/pdf; style-src 'unsafe-inline'; object-src 'self';`
  - `Cross-Origin-Resource-Policy: cross-origin` (v1.4+)
- Support `timeout_ms` query parameter (default 20000ms)
- May return 307/308 redirects for CDN support

### Endpoint 2: `GET /_matrix/client/v1/media/download/{serverName}/{mediaId}/{fileName}`
**Path**: `packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/by_file_name.rs`

**Requirements**:
- Same as endpoint 1
- MUST include the fileName in Content-Disposition header
- fileName overrides any stored filename

## Implementation Tasks

1. Create directory structure:
   - `packages/server/src/_matrix/client/v1/media/`
   - `packages/server/src/_matrix/client/v1/media/download/`
   - `packages/server/src/_matrix/client/v1/media/download/by_server_name/`
   - `packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/`

2. Implement authenticated download handler:
   - Extract and validate access token
   - Fetch media from MediaService
   - Apply proper Content-Type logic from spec
   - Calculate Content-Disposition based on content type
   - Add all required security headers
   - Support timeout_ms parameter
   - Return binary content (not JSON)

3. Implement authenticated download with filename handler:
   - Same as above but force filename in Content-Disposition

4. Register routes in main.rs

## Error Responses Required
- `401` - Unauthorized (invalid/missing token)
- `429` - Rate limited
- `502` - Content too large
- `504` - Not yet uploaded (M_NOT_YET_UPLOADED)

## Verification
```bash
# Test authenticated download
curl -H "Authorization: Bearer <token>" \
  http://localhost:8008/_matrix/client/v1/media/download/example.com/abc123

# Verify headers
curl -I -H "Authorization: Bearer <token>" \
  http://localhost:8008/_matrix/client/v1/media/download/example.com/abc123

# Test with filename override
curl -H "Authorization: Bearer <token>" \
  http://localhost:8008/_matrix/client/v1/media/download/example.com/abc123/myfile.pdf
```

## References
- Spec: `/tmp/matrix-spec/data/api/client-server/authed-content-repo.yaml` lines 18-147
- Spec: `/tmp/matrix-spec/content/client-server-api/modules/content_repo.md` lines 50-76
