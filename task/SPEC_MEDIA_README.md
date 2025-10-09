# Media API Gap Analysis - Task Index

## Quick Reference

This directory contains detailed task files for implementing missing Matrix Media API endpoints and fixing compliance issues.

## Task Files (Priority Order)

### 🔴 Critical - Spec Compliance (v1.11)
| File | Description | Status |
|------|-------------|--------|
| [SPEC_MEDIA_01](SPEC_MEDIA_01_authenticated_download.md) | Implement authenticated download endpoints | ❌ Missing |
| [SPEC_MEDIA_02](SPEC_MEDIA_02_authenticated_thumbnail.md) | Implement authenticated thumbnail endpoint | ❌ Missing |
| [SPEC_MEDIA_03](SPEC_MEDIA_03_authenticated_preview_config.md) | Implement authenticated preview_url & config | ❌ Missing |

### 🟠 High - Bug Fixes
| File | Description | Status |
|------|-------------|--------|
| [SPEC_MEDIA_04](SPEC_MEDIA_04_fix_v3_thumbnail.md) | Fix v3 thumbnail to return binary (not JSON) | ⚠️ Bug |

### 🟡 Medium - Enhancements
| File | Description | Status |
|------|-------------|--------|
| [SPEC_MEDIA_05](SPEC_MEDIA_05_v1_create_upload.md) | Complete v1/create endpoint (currently stub) | ⚠️ Stub |
| [SPEC_MEDIA_06](SPEC_MEDIA_06_content_disposition_headers.md) | Add required Content-Disposition headers | ❌ Missing |

### 🟢 Low - Migration Support
| File | Description | Status |
|------|-------------|--------|
| [SPEC_MEDIA_07](SPEC_MEDIA_07_deprecation_warnings.md) | Add deprecation warnings & freeze logic | ❌ Missing |

## Summary
| File | Description |
|------|-------------|
| [SPEC_MEDIA_00_SUMMARY](SPEC_MEDIA_00_SUMMARY.md) | **START HERE** - Complete gap analysis and overview |

## Quick Stats
- **Total Tasks**: 7
- **Critical (v1.11)**: 3 tasks
- **Bug Fixes**: 1 task
- **Enhancements**: 2 tasks
- **Migration**: 1 task

## Implementation Checklist

- [ ] **Phase 1: Critical Endpoints** (v1.11 compliance)
  - [ ] SPEC_MEDIA_01: Authenticated downloads
  - [ ] SPEC_MEDIA_02: Authenticated thumbnails
  - [ ] SPEC_MEDIA_03: Authenticated preview/config

- [ ] **Phase 2: Bug Fixes**
  - [ ] SPEC_MEDIA_04: Fix thumbnail to return binary

- [ ] **Phase 3: Headers & Standards** (v1.12 compliance)
  - [ ] SPEC_MEDIA_06: Content-Disposition headers

- [ ] **Phase 4: Enhancements**
  - [ ] SPEC_MEDIA_05: Complete v1/create endpoint

- [ ] **Phase 5: Migration Support**
  - [ ] SPEC_MEDIA_07: Deprecation warnings & freeze

## Current vs Spec

### ✅ Implemented
- v1/upload (functional)
- v1/download (functional)
- v3/upload (deprecated but working)
- v3/download (deprecated but working)
- v3/config (deprecated but working)
- v3/preview_url (deprecated but working)

### ❌ Missing
- All `/_matrix/client/v1/media/*` authenticated endpoints
- Content-Disposition headers (v1.12 requirement)
- Deprecation warnings
- Freeze logic for v1.12

### ⚠️ Issues
- v3/thumbnail returns JSON instead of binary
- v1/create is a non-functional stub

## Specification Version
- **Target**: Matrix v1.11 (current)
- **Future**: Matrix v1.12 (freeze unauthenticated endpoints)

## References
- Matrix Spec: `/tmp/matrix-spec/content/client-server-api/modules/content_repo.md`
- Current Code: `/packages/server/src/_matrix/media/`
- Federation Spec: `/spec/server/19-content-repository.md`
