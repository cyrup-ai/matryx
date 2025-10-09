# Media API Implementation Gap Analysis - Summary

## Overview
This analysis compared the current media implementation in `/packages/server/src/_matrix/media/` against the Matrix Client-Server API specification for Content Repository (v1.11+).

## Analysis Date
2025-10-08

## Key Findings

### ✅ What's Implemented
1. **v1 Endpoints (Partial)**:
   - ✅ `POST /_matrix/media/v1/create` (stub - needs work)
   - ✅ `POST /_matrix/media/v1/upload` (functional)
   - ✅ `GET /_matrix/media/v1/download/{serverName}/{mediaId}` (functional)

2. **v3 Endpoints (Deprecated but Working)**:
   - ✅ `POST /_matrix/media/v3/upload` (functional)
   - ✅ `PUT /_matrix/media/v3/upload/{serverName}/{mediaId}` (functional)
   - ✅ `GET /_matrix/media/v3/download/{serverName}/{mediaId}/{fileName}` (functional)
   - ✅ `GET /_matrix/media/v3/config` (functional)
   - ✅ `GET /_matrix/media/v3/preview_url` (functional)
   - ⚠️ `GET /_matrix/media/v3/thumbnail/{serverName}/{mediaId}` (returns JSON, should return binary)

### ❌ What's Missing

**Critical - Required by v1.11 Spec**:
1. ❌ `GET /_matrix/client/v1/media/download/{serverName}/{mediaId}` (authenticated)
2. ❌ `GET /_matrix/client/v1/media/download/{serverName}/{mediaId}/{fileName}` (authenticated)
3. ❌ `GET /_matrix/client/v1/media/thumbnail/{serverName}/{mediaId}` (authenticated)
4. ❌ `GET /_matrix/client/v1/media/preview_url` (authenticated)
5. ❌ `GET /_matrix/client/v1/media/config` (authenticated)

**Implementation Issues**:
6. ⚠️ Thumbnail endpoint returns JSON instead of binary image data
7. ⚠️ Missing Content-Disposition headers (required in v1.12+)
8. ⚠️ No deprecation warnings on v3 endpoints
9. ⚠️ No freeze logic for v1.12 compliance
10. ⚠️ v1/create is a non-functional stub

## Task Files Created

### High Priority (Spec Compliance)
1. **SPEC_MEDIA_01_authenticated_download.md**
   - Implement v1 authenticated download endpoints
   - Critical for v1.11+ compliance

2. **SPEC_MEDIA_02_authenticated_thumbnail.md**
   - Implement v1 authenticated thumbnail endpoint
   - Critical for v1.11+ compliance

3. **SPEC_MEDIA_03_authenticated_preview_config.md**
   - Implement v1 authenticated preview_url and config
   - Critical for v1.11+ compliance

### Medium Priority (Bug Fixes)
4. **SPEC_MEDIA_04_fix_v3_thumbnail.md**
   - Fix v3 thumbnail to return binary data instead of JSON
   - Current implementation violates spec

5. **SPEC_MEDIA_05_v1_create_upload.md**
   - Complete v1/create endpoint (currently a stub)
   - Verify PUT upload endpoint compliance

### Low Priority (Headers & Migration)
6. **SPEC_MEDIA_06_content_disposition_headers.md**
   - Add required Content-Disposition headers
   - Required for v1.12 compliance

7. **SPEC_MEDIA_07_deprecation_warnings.md**
   - Add deprecation headers to v3 endpoints
   - Implement freeze logic for v1.12

## Compliance Status

### Matrix v1.11
- ❌ **Non-Compliant**: Missing all authenticated v1 endpoints
- ⚠️ **Partial**: Has deprecated v3 endpoints (temporary acceptable)

### Matrix v1.12 (Expected)
- ❌ **Non-Compliant**: No freeze logic implemented
- ❌ **Non-Compliant**: Missing Content-Disposition headers
- ❌ **Non-Compliant**: Missing authenticated endpoints

## Recommended Implementation Order

1. **SPEC_MEDIA_04** - Fix thumbnail bug (quick win, fixes broken feature)
2. **SPEC_MEDIA_01** - Authenticated downloads (core functionality)
3. **SPEC_MEDIA_02** - Authenticated thumbnails (core functionality)
4. **SPEC_MEDIA_03** - Authenticated preview/config (core functionality)
5. **SPEC_MEDIA_06** - Content-Disposition headers (v1.12 requirement)
6. **SPEC_MEDIA_05** - Complete v1/create (enhancement)
7. **SPEC_MEDIA_07** - Deprecation warnings (migration support)

## Specification References

### Primary Spec Files
- `/tmp/matrix-spec/content/client-server-api/modules/content_repo.md`
- `/tmp/matrix-spec/data/api/client-server/content-repo.yaml` (deprecated v3)
- `/tmp/matrix-spec/data/api/client-server/authed-content-repo.yaml` (new v1)
- `/spec/server/19-content-repository.md` (federation)

### Current Implementation
- `/packages/server/src/_matrix/media/v1/*` (partial)
- `/packages/server/src/_matrix/media/v3/*` (deprecated but functional)

## Testing Notes

After implementation, verify:
1. All v1 authenticated endpoints work with access tokens
2. All v3 endpoints still work (backward compatibility)
3. Thumbnails return binary image data
4. Content-Disposition headers are correct
5. Freeze logic respects configuration
6. IdP icons exempted from freeze

## Next Steps

1. Review and prioritize task files
2. Assign tasks to development sprints
3. Implement authenticated v1 endpoints first
4. Add comprehensive tests for media endpoints
5. Update API documentation
6. Plan v1.12 freeze rollout

## Impact Assessment

**Breaking Changes**: None (adding missing features)
**Backward Compatibility**: Maintained (v3 endpoints remain)
**Client Impact**: Clients can migrate at their own pace
**Server Impact**: Need to implement authenticated endpoints before v1.12 freeze
