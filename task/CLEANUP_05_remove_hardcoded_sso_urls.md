# CLEANUP_05: SSO Redirect Security & Bug Fixes

## STATUS: 6/10 - Core implementation complete, but CRITICAL security vulnerability and bug remain

---

## CODEBASE RESEARCH & ANALYSIS

### Architecture Overview

**AppState Structure** ([`packages/server/src/state.rs:42`](../packages/server/src/state.rs))
```rust
pub struct AppState {
    pub db: Surreal<Any>,
    pub session_service: Arc<MatrixSessionService<Any>>,
    pub homeserver_name: String,  // ✓ Available for redirect validation
    // ... other fields
}
```

**SsoProvider Entity** ([`packages/surrealdb/src/repository/auth.rs:546-552`](../packages/surrealdb/src/repository/auth.rs))
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsoProvider {
    pub id: String,
    pub name: String,
    pub icon_url: Option<String>,
    pub brand: Option<String>,
    pub redirect_url: String,  // SSO provider's auth endpoint
}
```

**Error Handling** ([`packages/server/src/error/matrix_errors.rs:37`](../packages/server/src/error/matrix_errors.rs))
```rust
#[error("Invalid parameter value")]
MatrixError::InvalidParam,  // Returns M_INVALID_PARAM with 400 status
```

### Dependencies Verification

**Already Available** ([`packages/server/Cargo.toml:69`](../packages/server/Cargo.toml))
```toml
url = "2.5"  # ✓ Already in dependencies - no addition needed
urlencoding = "2.1.3"  # ✓ Already in dependencies
```

### Existing Codebase Patterns

**URL Validation Pattern** (from OAuth2 service at [`packages/server/src/auth/oauth2.rs:144-151`](../packages/server/src/auth/oauth2.rs))
```rust
// Validate redirect_uri
if !client.redirect_uris.contains(&params.redirect_uri) {
    return Err(ErrorResponse {
        error: "invalid_request".to_string(),
        error_description: Some("Invalid redirect_uri".to_string()),
        // ...
    });
}
```

**URL Parsing Pattern** (from preview_url at [`packages/server/src/_matrix/client/v1/media/preview_url.rs:188-201`](../packages/server/src/_matrix/client/v1/media/preview_url.rs))
```rust
use url::Url;

fn resolve_url(base_url: &str, relative_url: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    if relative_url.starts_with("http://") || relative_url.starts_with("https://") {
        Ok(relative_url.to_string())
    } else if let Ok(base) = Url::parse(base_url) {
        if let Ok(joined) = base.join(relative_url) {
            Ok(joined.to_string())
        } else {
            Ok(relative_url.to_string())
        }
    } else {
        Ok(relative_url.to_string())
    }
}
```

---

## SECURITY CONTEXT

### Why This Matters for Matrix Homeservers

**Open Redirect Vulnerabilities** are particularly dangerous in Matrix SSO implementations:

1. **Phishing Attacks**: Attackers can craft malicious URLs like:
   ```
   https://yourserver.matrix.org/_matrix/client/v3/login/sso/redirect?redirectUrl=https://evil.com
   ```
   This redirects users to `https://sso-provider.com/auth?redirectUrl=https://evil.com`, making the SSO provider send authenticated users to the attacker's site.

2. **OAuth Token Theft**: If the SSO provider includes tokens in the redirect URL, attackers can steal authentication credentials.

3. **Trust Exploitation**: Users trust URLs from their homeserver, making them more likely to follow malicious redirects.

**Matrix Specification Requirement**: The Matrix spec doesn't explicitly mandate redirect validation, but it's a security best practice aligned with OWASP guidelines and OAuth 2.0 security recommendations.

---

## COMPLETED ✅

- ✅ Removed hardcoded example.com URLs
- ✅ Added database queries via AuthRepository
- ✅ Injected AppState for database access  
- ✅ Return HTTP 302 redirects using `Redirect::temporary()`
- ✅ Handle `redirectUrl` query parameter
- ✅ Return 404 if no providers configured
- ✅ Return 404 if specific provider not found
- ✅ Added logging statements
- ✅ `SsoProvider` struct has `redirect_url` field
- ✅ Routes properly registered in main.rs

---

## CRITICAL ISSUES REMAINING ❌

### ISSUE 1: SECURITY VULNERABILITY - Open Redirect (HIGH PRIORITY)

**Location**: 
- [`packages/server/src/_matrix/client/v3/login/sso/redirect/mod.rs:48-51`](../packages/server/src/_matrix/client/v3/login/sso/redirect/mod.rs)
- [`packages/server/src/_matrix/client/v3/login/sso/redirect/by_idp_id.rs:46-49`](../packages/server/src/_matrix/client/v3/login/sso/redirect/by_idp_id.rs)

**Problem**: 
The `redirectUrl` parameter is NOT validated before being passed to the SSO provider. This creates an **OPEN REDIRECT VULNERABILITY** where attackers can craft malicious URLs.

**Current vulnerable code**:
```rust
if let Some(client_redirect) = params.redirect_url {
    sso_url.push_str("?redirectUrl=");
    sso_url.push_str(&urlencoding::encode(&client_redirect));
}
```

**Required Fix**:
Add validation to ensure `redirectUrl` is safe. The redirectUrl should be:
- A relative path starting with `/`, OR
- An absolute URL pointing to the same homeserver domain, OR  
- Reject invalid URLs with `MatrixError::InvalidParam`

**Implementation Pattern**:
```rust
use url::Url;

fn validate_redirect_url(redirect_url: &str, homeserver_domain: &str) -> Result<(), MatrixError> {
    // Allow relative URLs (most common case)
    if redirect_url.starts_with('/') {
        return Ok(());
    }
    
    // Parse absolute URLs and validate domain
    if let Ok(parsed) = Url::parse(redirect_url) {
        if let Some(host) = parsed.host_str() {
            // Allow exact match or subdomain
            if host == homeserver_domain || host.ends_with(&format!(".{}", homeserver_domain)) {
                return Ok(());
            }
        }
    }
    
    // Reject invalid or dangerous redirects
    tracing::warn!("Rejected invalid SSO redirectUrl: {}", redirect_url);
    Err(MatrixError::InvalidParam)
}

// Then in handler:
if let Some(client_redirect) = &params.redirect_url {
    validate_redirect_url(client_redirect, &state.homeserver_name)?;
    // ... rest of code
}
```

### ISSUE 2: BUG - Broken Query Parameter Construction

**Location**: Same files as Issue 1

**Problem**:
If `provider.redirect_url` already contains query parameters (e.g., `"https://sso.example.com/auth?client_id=abc"`), the code appends `"?redirectUrl=..."` which creates an **INVALID URL** with two `?` characters:
```
https://sso.example.com/auth?client_id=abc?redirectUrl=...
                                         ^ SECOND ? IS INVALID
```

**Current broken code**:
```rust
let mut sso_url = provider.redirect_url.clone();
if let Some(client_redirect) = params.redirect_url {
    sso_url.push_str("?redirectUrl=");  // ❌ Always uses "?"
    sso_url.push_str(&urlencoding::encode(&client_redirect));
}
```

**Required Fix**:
Use the `url` crate to properly construct URLs with query parameters:

```rust
use url::Url;

// Parse provider URL
let mut sso_url = Url::parse(&provider.redirect_url)
    .map_err(|e| {
        tracing::error!("Invalid provider redirect_url: {}", e);
        MatrixError::Unknown
    })?;

// Add redirectUrl as query parameter (handles existing params correctly)
if let Some(client_redirect) = params.redirect_url {
    validate_redirect_url(&client_redirect, &state.homeserver_name)?;
    sso_url.query_pairs_mut()
        .append_pair("redirectUrl", &client_redirect);
}

// Convert back to string for redirect
Ok(Redirect::temporary(sso_url.as_str()))
```

---

## IMPLEMENTATION GUIDE

### Step 1: Add validation helper function

Add this helper function to **both** files (or extract to a shared module):

```rust
use url::Url;

/// Validates that a redirect URL is safe to use
/// 
/// Accepts:
/// - Relative URLs starting with `/`
/// - Absolute URLs pointing to the homeserver domain
/// 
/// Rejects all other URLs to prevent open redirect vulnerabilities
fn validate_redirect_url(redirect_url: &str, homeserver_domain: &str) -> Result<(), MatrixError> {
    // Allow relative URLs (most common case for Matrix clients)
    if redirect_url.starts_with('/') {
        return Ok(());
    }
    
    // Parse and validate absolute URLs
    if let Ok(parsed) = Url::parse(redirect_url) {
        if let Some(host) = parsed.host_str() {
            // Allow exact domain match or subdomain
            if host == homeserver_domain || host.ends_with(&format!(".{}", homeserver_domain)) {
                return Ok(());
            }
        }
    }
    
    // Reject potentially malicious redirects
    tracing::warn!(
        "Rejected SSO redirectUrl '{}' - must be relative or match homeserver domain '{}'",
        redirect_url,
        homeserver_domain
    );
    Err(MatrixError::InvalidParam)
}
```

### Step 2: Update import statements

In both `mod.rs` and `by_idp_id.rs`, ensure these imports exist:

```rust
use url::Url;  // Add this import
use crate::error::matrix_errors::MatrixError;  // Already present
```

### Step 3: Replace URL construction logic

**In `mod.rs` (lines 44-55)**, replace:
```rust
// OLD CODE - REMOVE THIS
let mut sso_url = provider.redirect_url.clone();

if let Some(client_redirect) = params.redirect_url {
    sso_url.push_str("?redirectUrl=");
    sso_url.push_str(&urlencoding::encode(&client_redirect));
}

tracing::info!("Redirecting to SSO provider: {}", provider.id);

Ok(Redirect::temporary(&sso_url))
```

With:
```rust
// NEW CODE - SECURE AND CORRECT
let mut sso_url = Url::parse(&provider.redirect_url)
    .map_err(|e| {
        tracing::error!("Invalid SSO provider redirect_url for '{}': {}", provider.id, e);
        MatrixError::Unknown
    })?;

if let Some(client_redirect) = params.redirect_url {
    validate_redirect_url(&client_redirect, &state.homeserver_name)?;
    sso_url.query_pairs_mut()
        .append_pair("redirectUrl", &client_redirect);
}

tracing::info!("Redirecting to SSO provider: {}", provider.id);

Ok(Redirect::temporary(sso_url.as_str()))
```

**In `by_idp_id.rs` (lines 43-53)**, make the same replacement:
```rust
// OLD CODE - REMOVE THIS
let mut sso_url = provider.redirect_url.clone();

if let Some(client_redirect) = params.redirect_url {
    sso_url.push_str("?redirectUrl=");
    sso_url.push_str(&urlencoding::encode(&client_redirect));
}

tracing::info!("Redirecting to SSO provider: {}", provider.id);

Ok(Redirect::temporary(&sso_url))
```

With:
```rust
// NEW CODE - SECURE AND CORRECT
let mut sso_url = Url::parse(&provider.redirect_url)
    .map_err(|e| {
        tracing::error!("Invalid SSO provider redirect_url for '{}': {}", provider.id, e);
        MatrixError::Unknown
    })?;

if let Some(client_redirect) = params.redirect_url {
    validate_redirect_url(&client_redirect, &state.homeserver_name)?;
    sso_url.query_pairs_mut()
        .append_pair("redirectUrl", &client_redirect);
}

tracing::info!("Redirecting to SSO provider: {}", provider.id);

Ok(Redirect::temporary(sso_url.as_str()))
```

---

## CODE REFERENCES

### Files to Modify
- [`packages/server/src/_matrix/client/v3/login/sso/redirect/mod.rs`](../packages/server/src/_matrix/client/v3/login/sso/redirect/mod.rs) - Main SSO redirect endpoint
- [`packages/server/src/_matrix/client/v3/login/sso/redirect/by_idp_id.rs`](../packages/server/src/_matrix/client/v3/login/sso/redirect/by_idp_id.rs) - Provider-specific redirect endpoint

### Related Code References
- [`packages/server/src/state.rs:42`](../packages/server/src/state.rs) - AppState with homeserver_name field
- [`packages/server/src/error/matrix_errors.rs:37`](../packages/server/src/error/matrix_errors.rs) - MatrixError::InvalidParam definition
- [`packages/surrealdb/src/repository/auth.rs:424-432`](../packages/surrealdb/src/repository/auth.rs) - get_sso_providers method
- [`packages/surrealdb/src/repository/auth.rs:546-552`](../packages/surrealdb/src/repository/auth.rs) - SsoProvider struct
- [`packages/server/src/auth/oauth2.rs:144-151`](../packages/server/src/auth/oauth2.rs) - Example of redirect URI validation
- [`packages/server/src/_matrix/client/v1/media/preview_url.rs:188-201`](../packages/server/src/_matrix/client/v1/media/preview_url.rs) - Example of url::Url usage

---

## DEPENDENCIES

**No additions needed** - all required dependencies already present in [`packages/server/Cargo.toml`](../packages/server/Cargo.toml):
```toml
url = "2.5"  # ✓ Already present at line 69
urlencoding = "2.1.3"  # ✓ Already present at line 47
```

---

## FILES TO MODIFY

### 1. `packages/server/src/_matrix/client/v3/login/sso/redirect/mod.rs`

**Changes Required**:
- Line ~5: Add `use url::Url;` import
- Lines ~15-30: Add `validate_redirect_url()` helper function
- Lines ~44-55: Replace URL construction logic with secure version

### 2. `packages/server/src/_matrix/client/v3/login/sso/redirect/by_idp_id.rs`

**Changes Required**:
- Line ~5: Add `use url::Url;` import
- Lines ~15-30: Add `validate_redirect_url()` helper function (or import from mod.rs)
- Lines ~43-53: Replace URL construction logic with secure version

---

## DEFINITION OF DONE

- [ ] `redirectUrl` parameter is validated before use (security fix)
- [ ] Invalid redirectUrl values return `MatrixError::InvalidParam` with 400 status
- [ ] URL query parameters are constructed correctly using `url` crate (bug fix)
- [ ] Both redirect endpoints (main and by_idp_id) have both fixes applied
- [ ] Code compiles without errors
- [ ] Code follows existing codebase patterns and style
- [ ] Manual verification: Test with relative URLs like `/callback`
- [ ] Manual verification: Test with absolute URLs matching homeserver domain
- [ ] Manual verification: Malicious redirectUrl values (e.g., `https://evil.com`) are rejected
- [ ] Manual verification: Provider URLs with existing query params work correctly

---

## PRIORITY: HIGH (Security vulnerability)

This task blocks production deployment due to the open redirect security vulnerability. The bug also causes incorrect URLs to be generated when SSO providers have pre-configured query parameters.

**Impact**: 
- **Security**: Prevents phishing attacks and credential theft
- **Functionality**: Ensures SSO redirects work correctly with all provider configurations
- **Compliance**: Aligns with OWASP security guidelines and OAuth 2.0 best practices
