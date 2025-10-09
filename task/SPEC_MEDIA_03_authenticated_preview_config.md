# SPEC_MEDIA_03: Implement Authenticated Preview URL and Config Endpoints

## Status
Missing Implementation

## Description
Matrix v1.11 introduced authenticated versions of preview_url and config endpoints at `/_matrix/client/v1/media/*`.

## Current State
- v3/preview_url exists (deprecated, unauthenticated)
- v3/config exists (deprecated, unauthenticated)
- No v1 authenticated versions

## Spec Requirements

### Endpoint 1: `GET /_matrix/client/v1/media/preview_url`
**Path**: `packages/server/src/_matrix/client/v1/media/preview_url.rs`

**Required Query Parameters**:
- `url` (string, required) - URL to preview
- `ts` (integer, optional) - Preferred timestamp for preview

**Response** (JSON):
```json
{
  "og:title": "Page Title",
  "og:description": "Page description",
  "og:image": "mxc://example.com/mediaId",
  "og:image:type": "image/png",
  "og:image:height": 48,
  "og:image:width": 48,
  "matrix:image:size": 102400
}
```

**Security Note**:
- Clients should avoid this for URLs in encrypted rooms
- URLs may leak sensitive information to homeserver

**Implementation**:
- Requires authentication
- Can reuse existing v3 implementation logic
- Fetch URL content with timeout and size limits
- Parse OpenGraph/meta tags
- Download and store preview images as mxc:// URIs
- Return metadata in OpenGraph format

### Endpoint 2: `GET /_matrix/client/v1/media/config`
**Path**: `packages/server/src/_matrix/client/v1/media/config.rs`

**Response** (JSON):
```json
{
  "m.upload.size": 50000000
}
```

**Fields**:
- `m.upload.size` (integer, optional) - Max upload size in bytes
  - If not listed or null, size limit is unknown
  - Clients should use this as a guide
  - Proxies may enforce lower limits

**Implementation**:
- Requires authentication
- Can reuse existing v3 implementation logic
- Return server's upload size limit
- Consider dynamic limits based on user/storage

## Implementation Tasks

1. Create directory structure:
   - `packages/server/src/_matrix/client/v1/media/`

2. Create preview_url.rs:
   - Extract and validate access token
   - Validate URL format
   - Reuse logic from v3/preview_url.rs
   - Add authentication check

3. Create config.rs:
   - Extract and validate access token
   - Reuse logic from v3/config.rs
   - Return upload limits

4. Register routes in main.rs:
   - `GET /_matrix/client/v1/media/preview_url`
   - `GET /_matrix/client/v1/media/config`

## Code Migration

The v3 implementations already exist and can be adapted:

### From v3/preview_url.rs:
- Already has URL fetching logic
- Already parses OpenGraph tags
- Already downloads and stores preview images
- Just needs authentication added

### From v3/config.rs:
- Already returns m.upload.size
- Already has dynamic size calculation
- Just needs authentication added

## Error Responses
- `401` - Unauthorized (invalid/missing token)
- `429` - Rate limited

## Verification
```bash
# Test authenticated preview_url
curl -H "Authorization: Bearer <token>" \
  "http://localhost:8008/_matrix/client/v1/media/preview_url?url=https://matrix.org"

# Test authenticated config
curl -H "Authorization: Bearer <token>" \
  http://localhost:8008/_matrix/client/v1/media/config
```

## References
- Spec: `/tmp/matrix-spec/data/api/client-server/authed-content-repo.yaml` lines 359-490
- Spec: `/tmp/matrix-spec/content/client-server-api/modules/content_repo.md`
- Current: `packages/server/src/_matrix/media/v3/preview_url.rs`
- Current: `packages/server/src/_matrix/media/v3/config.rs`
