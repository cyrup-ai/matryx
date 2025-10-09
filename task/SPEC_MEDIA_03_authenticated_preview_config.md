# SPEC_MEDIA_03: Authenticated Preview URL and Config Endpoints - SECURITY FIXES REQUIRED

## Status
**⚠️  CRITICAL SECURITY ISSUES - NOT PRODUCTION READY**

Core functionality is implemented but has **critical SSRF vulnerabilities** that must be resolved before production use.

## Critical Security Issues

### 1. SSRF Vulnerability Protection (REQUIRED)

**Location:** `packages/server/src/_matrix/client/v1/media/preview_url.rs` (lines 47-73)

**Problem:** No validation to prevent Server-Side Request Forgery attacks. The endpoint can be exploited to:
- Scan internal networks (localhost, 127.0.0.1, 192.168.x.x, 10.x.x.x, 172.16-31.x.x)
- Access cloud metadata services (169.254.169.254, fd00:ec2::254)
- Access internal services via DNS resolution
- Follow redirects from public URLs to internal resources

**Required Implementation:**

```rust
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Validate URL is not targeting internal networks
async fn validate_url_safety(url: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let parsed = url::Url::parse(url)?;
    
    // Only allow http/https
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err("Invalid scheme".into());
    }
    
    // Get host
    let host = parsed.host_str().ok_or("No host")?;
    
    // Resolve hostname to IP addresses
    let addrs: Vec<IpAddr> = tokio::net::lookup_host(format!("{}:80", host))
        .await?
        .map(|addr| addr.ip())
        .collect();
    
    // Check each resolved IP
    for addr in addrs {
        if is_private_ip(&addr) {
            return Err("Access to private IPs is forbidden".into());
        }
    }
    
    Ok(())
}

fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            ipv4.is_private() ||
            ipv4.is_loopback() ||
            ipv4.is_link_local() ||
            ipv4.octets()[0] == 169 && ipv4.octets()[1] == 254 || // AWS metadata
            ipv4.octets()[0] == 0 // 0.0.0.0/8
        },
        IpAddr::V6(ipv6) => {
            ipv6.is_loopback() ||
            ipv6.is_unicast_link_local() ||
            ipv6.is_unique_local() ||
            ipv6.is_unspecified()
        }
    }
}
```

**Apply validation BEFORE fetching:**
```rust
// In fetch_url_preview function, add after line 60:
validate_url_safety(url).await?;
```

### 2. Redirect Control (REQUIRED)

**Problem:** reqwest follows up to 10 redirects by default with no SSRF validation on redirect targets.

**Required Fix:**

```rust
// In fetch_url_preview, replace reqwest::Client::builder() (line 66-69) with:
let client = reqwest::Client::builder()
    .timeout(std::time::Duration::from_secs(10))
    .user_agent(format!("Matrix Homeserver {}", homeserver_name))
    .redirect(reqwest::redirect::Policy::custom(|attempt| {
        // Validate redirect URL is not private
        if let Some(url) = attempt.url().as_str() {
            // Synchronous check - requires refactoring to async or use sync DNS
            // For now, limit redirects to same host
            if attempt.previous().len() >= 3 {
                return attempt.error("Too many redirects");
            }
            if attempt.url().host_str() != attempt.previous()[0].host_str() {
                return attempt.error("Cross-domain redirects forbidden");
            }
        }
        attempt.follow()
    }))
    .build()?;
```

**Better approach:** Disable redirects entirely and handle manually with validation:
```rust
let client = reqwest::Client::builder()
    .timeout(std::time::Duration::from_secs(10))
    .user_agent(format!("Matrix Homeserver {}", homeserver_name))
    .redirect(reqwest::redirect::Policy::none())
    .build()?;

let mut current_url = url.to_string();
let mut redirect_count = 0;

loop {
    validate_url_safety(&current_url).await?;
    
    let response = client.get(&current_url).send().await?;
    
    if response.status().is_redirection() {
        if redirect_count >= 3 {
            return Err("Too many redirects".into());
        }
        
        if let Some(location) = response.headers().get("location") {
            current_url = location.to_str()?.to_string();
            redirect_count += 1;
            continue;
        }
        return Err("Redirect without location".into());
    }
    
    // Process response...
    break;
}
```

## Code Quality Improvements (Recommended)

### 3. Case-Insensitive HTML Parsing

**Location:** `preview_url.rs` lines 93-125

**Issue:** Regex patterns are case-sensitive and won't match `<META>` or `<TITLE>` tags.

**Fix:** Add `(?i)` flag to regex patterns:
```rust
// Line 101: Add (?i) for case-insensitive
regex::Regex::new(r#"(?i)<meta[^>]+property="og:title"[^>]+content="([^"]*)"[^>]*>"#)

// Apply to all regex patterns (title, description, image, fallbacks)
```

### 4. Extract og:image:type Field

**Location:** `preview_url.rs` line 160-167

**Issue:** The spec shows `og:image:type` in response but implementation doesn't extract it.

**Fix:** Update PreviewResponse struct and include image type:
```rust
#[derive(Serialize)]
pub struct PreviewResponse {
    #[serde(rename = "og:title", skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(rename = "og:description", skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "og:image", skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(rename = "og:image:type", skip_serializing_if = "Option::is_none")]
    pub image_type: Option<String>,
    #[serde(rename = "matrix:image:size", skip_serializing_if = "Option::is_none")]
    pub image_size: Option<u64>,
}

// In fetch_url_preview, when storing image (line 160-167):
let img_content_type_final = img_content_type.clone();
// ... upload ...
// Then add to response:
image_type = Some(img_content_type_final);
```

### 5. Validate Image Content-Type

**Location:** `preview_url.rs` line 148-150

**Issue:** Trusts Content-Type header without validation.

**Fix:** Verify content type is actually an image:
```rust
// After line 150, add validation:
if !img_content_type.starts_with("image/") {
    // Skip non-image content
    continue; // or skip to next logic
}
```

## Optional Enhancements (Nice to Have)

### 6. Extract Image Dimensions

The Matrix spec includes optional fields `og:image:width` and `og:image:height`. These could be extracted using the `image` crate (already in Cargo.toml):

```rust
use image::GenericImageView;

// After downloading image (line 158):
if let Ok(img) = image::load_from_memory(&img_data) {
    let (width, height) = img.dimensions();
    // Add to response
}
```

### 7. Preview Caching

Currently every request re-fetches the URL. Consider implementing a TTL-based cache to reduce external requests and improve performance.

## Files Requiring Changes

1. **`packages/server/src/_matrix/client/v1/media/preview_url.rs`**
   - Add SSRF validation function (lines 60-90)
   - Update HTTP client with redirect control (lines 66-73)
   - Add case-insensitive regex flags (lines 93-125)
   - Add image type extraction (lines 148-167)
   - Add content-type validation (line 150)

2. **`packages/server/src/_matrix/media/v3/preview_url.rs`**
   - Apply same fixes as v1 implementation (duplicate code)

## Verification Commands

After fixes, test SSRF protection:

```bash
# Should be rejected:
curl -H "Authorization: Bearer $TOKEN" \
  "http://localhost:8008/_matrix/client/v1/media/preview_url?url=http://localhost:8008/admin"

curl -H "Authorization: Bearer $TOKEN" \
  "http://localhost:8008/_matrix/client/v1/media/preview_url?url=http://169.254.169.254/latest/meta-data"

curl -H "Authorization: Bearer $TOKEN" \
  "http://localhost:8008/_matrix/client/v1/media/preview_url?url=http://192.168.1.1"

# Should work:
curl -H "Authorization: Bearer $TOKEN" \
  "http://localhost:8008/_matrix/client/v1/media/preview_url?url=https://matrix.org"
```

## Definition of Done

- ✅ Routes registered and authentication working (COMPLETE)
- ✅ Config endpoint returns upload limits (COMPLETE)
- ✅ Preview URL parses OpenGraph metadata (COMPLETE - but needs improvements)
- ❌ **SSRF protection implemented and tested (CRITICAL - REQUIRED)**
- ❌ **Redirect validation prevents internal access (CRITICAL - REQUIRED)**
- ⚠️  Case-insensitive HTML parsing (RECOMMENDED)
- ⚠️  Image content-type validation (RECOMMENDED)
- ⚠️  og:image:type field extraction (RECOMMENDED)

**The implementation cannot be marked complete until SSRF protection is implemented.**
