# Media API Implementation - Critical Issues (QA Rating: 4/10)

## QA Review Date
2025-10-09

## Overall Rating: 4/10

### Rating Justification
While core v3 functionality (upload, download, config, preview_url) is implemented and working, there are **critical production-blocking issues**:
1. v3 thumbnail endpoint returns JSON instead of binary image data (SPEC VIOLATION)
2. Authenticated v1 client endpoints are registered but return JSON stubs (NON-FUNCTIONAL)
3. Architectural confusion with duplicate implementations at different URL paths
4. Original spec document was inaccurate, claiming endpoints were "missing" when they exist as stubs

## CRITICAL ISSUES REQUIRING IMMEDIATE ATTENTION

### 1. v3 Thumbnail Returns JSON Instead of Binary Image Data
**File**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs`

**Current Implementation** (WRONG):
```rust
Ok(Json(json!({
    "content_type": thumbnail_result.content_type,
    "width": thumbnail_result.width,
    "height": thumbnail_result.height
})))
```

**Required Implementation**:
- Return binary image data in response body
- Set Content-Type header to image type
- Set Content-Length header
- Return actual thumbnail bytes, not JSON metadata

**Impact**: SPEC VIOLATION - Clients cannot display thumbnails

---

### 2. Authenticated v1 Client Endpoints Are Non-Functional Stubs
**Affected Files**:
- `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/mod.rs`
- `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/by_file_name.rs`
- `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v1/media/thumbnail/by_server_name/by_media_id.rs`

**Current Implementation** (WRONG):
All three endpoints return JSON stubs:
```rust
Ok(Json(json!({
    "content_type": "image/jpeg",
    "content_disposition": "attachment; filename=example.jpg"
})))
```

**Required Implementation**:
- Use the EXISTING functional code from `/_matrix/media/v1/download.rs`
- Return binary data with proper headers
- Add authentication via `AuthenticatedUser` extractor
- Remove duplicate stub implementations

**Impact**: Matrix v1.11+ NON-COMPLIANT - Authenticated media endpoints don't work

---

### 3. Architectural Duplication Issue
**Problem**: Functional v1 implementations exist at TWO different paths:

**Functional (but deprecated path)**:
- `/_matrix/media/v1/download/{serverName}/{mediaId}` → Returns binary (CORRECT)
  - File: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v1/download.rs`

**Stubs (spec-compliant path)**:
- `/_matrix/client/v1/media/download/{serverName}/{mediaId}` → Returns JSON (WRONG)
  - File: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v1/media/download/*/mod.rs`

**Required Fix**:
1. Move functional implementation from `/_matrix/media/v1/download.rs` to stub locations
2. Delete or deprecate old `/_matrix/media/v1/*` endpoints
3. Ensure authentication middleware applies to client endpoints

---

## WHAT'S ACTUALLY WORKING (✅)

### v3 Endpoints (Deprecated but Functional)
1. ✅ `POST /_matrix/media/v3/upload` - Full multipart upload with validation
2. ✅ `PUT /_matrix/media/v3/upload/{serverName}/{mediaId}` - Metadata updates
3. ✅ `GET /_matrix/media/v3/download/{serverName}/{mediaId}` - Multipart/mixed response
4. ✅ `GET /_matrix/media/v3/download/{serverName}/{mediaId}/{fileName}` - Binary download
5. ✅ `GET /_matrix/media/v3/config` - Dynamic config based on storage usage
6. ✅ `GET /_matrix/media/v3/preview_url` - Full OpenGraph parsing with image caching
7. ❌ `GET /_matrix/media/v3/thumbnail/{serverName}/{mediaId}` - **BROKEN: Returns JSON**

### v1 Endpoints (Deprecated Path but Functional)
1. ✅ `POST /_matrix/media/v1/upload` - Full implementation
2. ✅ `GET /_matrix/media/v1/download/{serverName}/{mediaId}` - Binary download
3. ✅ `GET /_matrix/media/v1/download/{serverName}/{mediaId}/{fileName}` - Binary download
4. ⚠️ `POST /_matrix/media/v1/create` - Minimal stub (returns fake content_uri)

### v1 Authenticated Client Endpoints (Spec-Compliant Path)
1. ✅ `GET /_matrix/client/v1/media/config` - Functional
2. ✅ `GET /_matrix/client/v1/media/preview_url` - Fully implemented with OpenGraph
3. ❌ `GET /_matrix/client/v1/media/download/{serverName}/{mediaId}` - **STUB: Returns JSON**
4. ❌ `GET /_matrix/client/v1/media/download/{serverName}/{mediaId}/{fileName}` - **STUB: Returns JSON**
5. ❌ `GET /_matrix/client/v1/media/thumbnail/{serverName}/{mediaId}` - **STUB: Returns JSON**

---

## IMPLEMENTATION TASKS

### HIGH PRIORITY (Production Blockers)

#### TASK 1: Fix v3 Thumbnail to Return Binary Data
**File**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs`

**Changes Required**:
1. Change return type from `Result<Json<Value>, StatusCode>` to `Result<Response<Body>, StatusCode>`
2. Return `thumbnail_result.data` as binary body
3. Set `Content-Type` header to `thumbnail_result.content_type`
4. Set `Content-Length` header
5. Remove JSON serialization

**Reference Implementation**: See `/_matrix/media/v3/download/by_server_name/by_media_id/by_file_name.rs` lines 35-54

---

#### TASK 2: Implement v1 Client Download Endpoints
**Files**:
- `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/mod.rs`
- `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/by_file_name.rs`

**Changes Required**:
1. Copy implementation from `/_matrix/media/v1/download.rs` (lines 15-64)
2. Add `AuthenticatedUser` extractor parameter
3. Update to return binary Response<Body> instead of JSON
4. Ensure Content-Disposition headers are set

---

#### TASK 3: Implement v1 Client Thumbnail Endpoint
**File**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v1/media/thumbnail/by_server_name/by_media_id.rs`

**Changes Required**:
1. Generate thumbnail using MediaService
2. Return binary image data (not JSON)
3. Set appropriate Content-Type header
4. Add Content-Disposition header
5. Use `AuthenticatedUser` for access control

**Reference**: Follow pattern from download implementation

---

### MEDIUM PRIORITY (Spec Compliance)

#### TASK 4: Complete v1/create Endpoint
**File**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v1/create.rs`

**Current**: Returns fake `content_uri: "mxc://{homeserver}/example"`

**Required**:
1. Generate unique media_id
2. Reserve media_id in database
3. Return real content_uri
4. Implement subsequent PUT upload to reserved ID

---

#### TASK 5: Add Content-Disposition Headers (v1.12 Requirement)
**Affected Files**: All download and thumbnail endpoints

**Changes Required**:
1. Add `Content-Disposition: inline; filename="..."` header to all media responses
2. Use filename from database metadata if available
3. Generate filename from media_id as fallback
4. Ensure proper quote escaping in filename

---

### LOW PRIORITY (Future Enhancement)

#### TASK 6: Deprecation Warnings for v3 Endpoints
Add deprecation headers to all v3 endpoints:
- `Deprecation: true`
- Link to v1 authenticated endpoints in response headers

#### TASK 7: Implement Freeze Logic (v1.12)
- Add configuration for media freeze policy
- Exempt IdP icons from freeze
- Return appropriate error codes for frozen media

---

## COMPLIANCE STATUS

### Matrix Client-Server API v1.11
❌ **NON-COMPLIANT**
- Authenticated v1 download endpoints are stubs
- Authenticated v1 thumbnail endpoint is stub
- v3 thumbnail returns incorrect format

### Matrix Client-Server API v1.12 (Future)
❌ **NON-COMPLIANT**
- Missing Content-Disposition headers
- No freeze logic implemented
- No deprecation warnings on v3 endpoints

---

## FILE LOCATIONS REFERENCE

### Working Implementations (Can be used as reference)
- **v3 Download**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v3/download/by_server_name/by_media_id/by_file_name.rs`
- **v3 Upload**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v3/upload/mod.rs`
- **v3 Config**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v3/config.rs`
- **v3 Preview**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v3/preview_url.rs`
- **v1 Download**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v1/download.rs`
- **v1 Upload**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v1/upload.rs`

### Broken/Stub Implementations (Need fixing)
- **v3 Thumbnail**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v3/thumbnail/by_server_name/by_media_id.rs`
- **v1 Client Download**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v1/media/download/by_server_name/by_media_id/mod.rs`
- **v1 Client Thumbnail**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v1/media/thumbnail/by_server_name/by_media_id.rs`
- **v1 Create**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/media/v1/create.rs`

### Router Configuration
- **Main Router**: `/Volumes/samsung_t9/maxtryx/packages/server/src/main.rs` (lines 384-463)
  - Client routes: lines 384-463
  - Media routes: lines 631-655

---

## TESTING CHECKLIST (After Fixes)

- [ ] v3 thumbnail returns binary PNG/JPEG data
- [ ] v3 thumbnail includes correct Content-Type header
- [ ] v1 client download endpoints return binary data
- [ ] v1 client download endpoints require authentication
- [ ] v1 client thumbnail returns binary image
- [ ] v1 client thumbnail requires authentication
- [ ] All endpoints set Content-Disposition headers
- [ ] v1/create generates unique media IDs
- [ ] Federation media download works
- [ ] Large file uploads succeed
- [ ] Thumbnail generation preserves aspect ratio

---

## NEXT STEPS

1. **Immediate**: Fix v3 thumbnail (TASK 1) - Production blocker
2. **High Priority**: Implement v1 client download (TASK 2) - Spec compliance
3. **High Priority**: Implement v1 client thumbnail (TASK 3) - Spec compliance
4. **Medium Priority**: Complete v1/create (TASK 4)
5. **Medium Priority**: Add Content-Disposition headers (TASK 5)
6. **Future**: Deprecation warnings and freeze logic (TASKS 6-7)

/Volumes/samsung_t9/maxtryx/task/SPEC_MEDIA_00_SUMMARY.md
