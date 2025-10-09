# CLEANUP_05: SSO Redirect Security & Bug Fixes

## STATUS: 6/10 - Core implementation complete, but CRITICAL security vulnerability and bug remain

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

## CRITICAL ISSUES REMAINING ❌

### ISSUE 1: SECURITY VULNERABILITY - Open Redirect (HIGH PRIORITY)

**Location**: 
- `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v3/login/sso/redirect/mod.rs:48-51`
- `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v3/login/sso/redirect/by_idp_id.rs:46-49`

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
fn validate_redirect_url(redirect_url: &str, homeserver_domain: &str) -> Result<(), MatrixError> {
    // Allow relative URLs
    if redirect_url.starts_with('/') {
        return Ok(());
    }
    
    // Parse absolute URLs and validate domain
    if let Ok(parsed) = url::Url::parse(redirect_url) {
        if let Some(host) = parsed.host_str() {
            if host == homeserver_domain || host.ends_with(&format!(".{}", homeserver_domain)) {
                return Ok(());
            }
        }
    }
    
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
If `provider.redirect_url` already contains query parameters (e.g., `"https://sso.example.com/auth?client_id=abc"`), the code appends `"?redirectUrl=..."` which creates an **INVALID URL** with two `?` characters: `https://sso.example.com/auth?client_id=abc?redirectUrl=...`

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

## DEPENDENCIES

Add to `packages/server/Cargo.toml` if not already present:
```toml
url = "2.5"  # For proper URL parsing and construction
```

## FILES TO MODIFY

1. **`/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v3/login/sso/redirect/mod.rs`**
   - Add `validate_redirect_url()` helper function
   - Fix URL construction using `url` crate
   - Add validation before using redirectUrl

2. **`/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v3/login/sso/redirect/by_idp_id.rs`**  
   - Import and use same `validate_redirect_url()` helper
   - Fix URL construction using `url` crate
   - Add validation before using redirectUrl

## DEFINITION OF DONE

- [ ] `redirectUrl` parameter is validated before use (security)
- [ ] URL query parameters are constructed correctly using `url` crate (bug fix)
- [ ] Both redirect endpoints (main and by_idp_id) have both fixes applied
- [ ] Invalid redirectUrl returns `MatrixError::InvalidParam` 
- [ ] Code compiles without errors
- [ ] Manual security testing: malicious redirectUrl values are rejected

## PRIORITY: HIGH (Security vulnerability)

This task blocks production deployment due to the open redirect security vulnerability.
