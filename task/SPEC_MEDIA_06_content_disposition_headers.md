# SPEC_MEDIA_06: Add Required Content-Disposition Headers

## Status
Missing - Spec Compliance Issue

## Description
Matrix v1.12 requires `Content-Disposition` headers on all download and thumbnail responses. Current implementation may be missing these headers or not calculating them correctly.

## Current State
Some endpoints have Content-Disposition headers, but need to verify they follow v1.12 requirements:
- Must be present (required in v1.12+)
- Must be either "inline" or "attachment"
- Decision based on Content-Type safety rules
- Must include filename when available

## Spec Requirements

### Content-Disposition Rules

**For Download Endpoints**:
```
Content-Disposition: inline; filename="filename.jpg"
Content-Disposition: attachment; filename="filename.pdf"
```

**Decision Logic**:
1. If Content-Type is in safe list → use `inline`
2. Otherwise → use `attachment`
3. Include filename if available

**Safe Content Types for `inline`** (from spec):
- `text/css`
- `text/plain`
- `text/csv`
- `application/json`
- `application/ld+json`
- `image/jpeg`
- `image/gif`
- `image/png`
- `image/apng`
- `image/webp`
- `image/avif`
- `video/mp4`
- `video/webm`
- `video/ogg`
- `video/quicktime`
- `audio/mp4`
- `audio/webm`
- `audio/aac`
- `audio/mpeg`
- `audio/ogg`
- `audio/wave`
- `audio/wav`
- `audio/x-wav`
- `audio/x-pn-wav`
- `audio/flac`
- `audio/x-flac`

All other types MUST use `attachment` to prevent XSS attacks.

### For Thumbnail Endpoints:
```
Content-Disposition: inline; filename="thumbnail.png"
```

Always `inline` (thumbnails are always safe image types).

## Implementation Tasks

### 1. Create Helper Function

```rust
// packages/server/src/utils/content_disposition.rs

const SAFE_INLINE_CONTENT_TYPES: &[&str] = &[
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

pub fn calculate_content_disposition(
    content_type: &str,
    filename: Option<&str>,
) -> String {
    let disposition = if is_safe_for_inline(content_type) {
        "inline"
    } else {
        "attachment"
    };

    if let Some(name) = filename {
        // Escape filename for header
        let escaped = name.replace('\"', "\\\"");
        format!("{}; filename=\"{}\"", disposition, escaped)
    } else {
        disposition.to_string()
    }
}

fn is_safe_for_inline(content_type: &str) -> bool {
    // Get base content type (before semicolon/parameters)
    let base_type = content_type.split(';').next().unwrap_or("").trim();
    SAFE_INLINE_CONTENT_TYPES.contains(&base_type)
}
```

### 2. Update All Download Endpoints

**Files to update**:
- `packages/server/src/_matrix/media/v1/download.rs`
- `packages/server/src/_matrix/media/v3/download/by_server_name/by_media_id/by_file_name.rs`
- `packages/server/src/_matrix/client/v1/media/download/...` (when implemented)

```rust
use crate::utils::content_disposition::calculate_content_disposition;

// In response builder:
let content_disposition = calculate_content_disposition(
    &download_result.content_type,
    download_result.filename.as_deref()
);

Response::builder()
    .header(header::CONTENT_TYPE, download_result.content_type)
    .header(header::CONTENT_DISPOSITION, content_disposition)
    // ... other headers
```

### 3. Update All Thumbnail Endpoints

**Files to update**:
- `packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs`
- `packages/server/src/_matrix/client/v1/media/thumbnail/...` (when implemented)

```rust
// Thumbnails always use inline
Response::builder()
    .header(header::CONTENT_TYPE, thumbnail_result.content_type)
    .header(header::CONTENT_DISPOSITION, "inline; filename=\"thumbnail.png\"")
    // ... other headers
```

### 4. Verify Filename Handling

Ensure filename is:
- Stored during upload
- Retrieved during download
- Properly escaped in Content-Disposition header
- Preserved from remote servers when federating

### 5. Security Considerations

**XSS Prevention**:
- Never use `inline` for unknown/untrusted content types
- Always use safe list from spec
- Log warnings when untrusted content types are uploaded

**Filename Escaping**:
- Escape quotes in filename
- Sanitize path separators (/, \)
- Limit filename length

## Testing

```bash
# Test safe content type (should be inline)
curl -I -H "Authorization: Bearer <token>" \
  http://localhost:8008/_matrix/client/v1/media/download/example.com/image123.png
# Expect: Content-Disposition: inline; filename="..."

# Test unsafe content type (should be attachment)
curl -I -H "Authorization: Bearer <token>" \
  http://localhost:8008/_matrix/client/v1/media/download/example.com/file123.html
# Expect: Content-Disposition: attachment; filename="..."

# Test thumbnail (always inline)
curl -I -H "Authorization: Bearer <token>" \
  "http://localhost:8008/_matrix/client/v1/media/thumbnail/example.com/abc?width=64&height=64"
# Expect: Content-Disposition: inline; filename="thumbnail.png"

# Test filename with special characters
# Upload file with name: test"file.txt
# Download and verify proper escaping: filename="test\"file.txt"
```

## Migration Note

**Backward Compatibility**:
- Clients SHOULD NOT rely on `inline` vs `attachment`
- Servers MAY choose to always use `attachment` for safety
- Header became required in v1.12, so older clients may not expect it

## References
- Spec: `/tmp/matrix-spec/content/client-server-api/modules/content_repo.md` lines 165-211
- Spec: `/tmp/matrix-spec/data/api/client-server/authed-content-repo.yaml`
- Current: Various download endpoint files
