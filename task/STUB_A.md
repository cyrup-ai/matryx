# STUB_A: Client Authentication APIs Implementation

**Status**: COMPLETE  
**Package**: `matryx_client`  
**Matrix Spec Compliance**: [Client-Server API v3](../../spec/client/01_foundation_api.md)

---

## Core Objective

Implement Matrix-compliant client authentication APIs for the `matryx_client` package, providing structured clients for login and registration operations. The implementation must use the centralized `MatrixHttpClient` infrastructure and follow Matrix specification requirements for authentication flows.

**Key Requirements:**
- Implement `LoginClient` for handling Matrix login operations
- Implement `RegisterClient` for handling Matrix registration operations  
- Support multiple authentication flows (password, token, SSO)
- Automatic access token management upon successful authentication
- Matrix-spec-compliant error handling with proper error codes
- Type compatibility with server-side implementations

---

## Matrix Specification Compliance

### Relevant Spec Files

**Client API Foundation:**
- [./spec/client/01_foundation_api.md](../../spec/client/01_foundation_api.md)
  - Section: "Client Authentication" - Lines 380-490
  - Section: "Legacy API" - User-Interactive Authentication
  - Section: "Account registration" - Line 408
  - Section: "Login" - Lines 412-420

**Key Endpoints to Implement:**

1. **GET /_matrix/client/v3/login**
   - Discover available login flows
   - Returns supported authentication types
   - No authentication required

2. **POST /_matrix/client/v3/login**
   - Perform login with credentials
   - Returns access_token, user_id, device_id
   - Supports multiple login types: `m.login.password`, `m.login.token`

3. **GET /_matrix/client/v3/register**  
   - Discover registration flows and requirements
   - Returns available authentication stages
   - No authentication required

4. **POST /_matrix/client/v3/register**
   - Register new user account
   - Supports multi-stage User-Interactive Authentication (UIA)
   - Returns user_id and optionally access_token

### Error Codes to Handle

Per Matrix spec, authentication endpoints must handle:
- `M_FORBIDDEN` - Invalid credentials
- `M_USER_IN_USE` - Username already taken (registration)
- `M_INVALID_USERNAME` - Invalid username format (registration)
- `M_WEAK_PASSWORD` - Password doesn't meet requirements (registration)
- `M_UNAUTHORIZED` - Additional auth stages required (UIA)
- `M_LIMIT_EXCEEDED` - Rate limiting with retry_after_ms
- `M_UNKNOWN_TOKEN` - Invalid/expired token

---

## Architecture Overview

### Integration with MatrixHttpClient

The implementation leverages the existing `MatrixHttpClient` infrastructure located at:
- [./packages/client/src/http_client.rs](../../packages/client/src/http_client.rs)

**MatrixHttpClient provides:**
- Generic request/response handling with type safety
- Automatic JSON serialization/deserialization
- Matrix-spec error parsing and error type conversion
- Thread-safe access token management via `Arc<RwLock<Option<String>>>`
- Retry logic with exponential backoff
- Rate limit handling with `retry_after_ms` support

**Pattern:**
```rust
pub struct MatrixHttpClient {
    client: Client,                              // reqwest::Client
    homeserver_url: Url,                         // Base URL
    access_token: Arc<RwLock<Option<String>>>,  // Thread-safe token storage
}
```

### Client Structure Pattern

Both `LoginClient` and `RegisterClient` follow the same architectural pattern:

```rust
pub struct LoginClient {
    http_client: MatrixHttpClient,  // Composition over inheritance
}

impl LoginClient {
    pub fn new(http_client: MatrixHttpClient) -> Self {
        Self { http_client }
    }
    
    // Method implementations use self.http_client for all requests
}
```

**Benefits of this pattern:**
- Single source of truth for HTTP configuration
- Automatic token management across all client types
- Consistent error handling via `HttpClientError`
- Easy testing via dependency injection

### Package Structure

```
packages/client/src/
├── http_client.rs                    # Core HTTP infrastructure
├── _matrix/
│   └── client/
│       ├── v3/
│       │   ├── login/
│       │   │   ├── mod.rs           # Type definitions + exports
│       │   │   └── client.rs        # LoginClient implementation
│       │   └── register/
│       │       ├── mod.rs           # Type definitions + exports  
│       │       └── client.rs        # RegisterClient implementation
│       └── v1/                       # Deprecated stubs
└── lib.rs                            # Re-exports for convenience
```

---

## Implementation Details

### 1. LoginClient Implementation

**Location**: [./packages/client/src/_matrix/client/v3/login/client.rs](../../packages/client/src/_matrix/client/v3/login/client.rs)

**Core Methods:**

```rust
impl LoginClient {
    /// Create new login client with MatrixHttpClient
    pub fn new(http_client: MatrixHttpClient) -> Self
    
    /// Get available login flows from server
    /// GET /_matrix/client/v3/login
    pub async fn get_login_flows(&self) -> Result<LoginFlowsResponse, HttpClientError>
    
    /// Perform login request with full control
    /// POST /_matrix/client/v3/login
    pub async fn login(&self, request: &LoginRequest) -> Result<LoginResponse, HttpClientError>
    
    /// Convenience: Login with username/password
    pub async fn login_with_password(
        &self,
        username: &str,
        password: &str,
        device_id: Option<String>,
        device_display_name: Option<String>,
    ) -> Result<LoginResponse, HttpClientError>
    
    /// Convenience: Login with pre-authenticated token (SSO, app service)
    pub async fn login_with_token(
        &self,
        token: &str,
        device_id: Option<String>,
        device_display_name: Option<String>,
    ) -> Result<LoginResponse, HttpClientError>
}
```

**Key Implementation Pattern:**

```rust
pub async fn login(&self, request: &LoginRequest) -> Result<LoginResponse, HttpClientError> {
    // 1. Make POST request using MatrixHttpClient
    let response: LoginResponse = self.http_client
        .post("/_matrix/client/v3/login", request)
        .await?;

    // 2. Automatically set access token for future requests
    self.http_client.set_access_token(response.access_token.clone()).await;

    // 3. Return response to caller
    Ok(response)
}
```

**Automatic Token Management:**
After successful login, the access token is automatically stored in `MatrixHttpClient`'s shared state. All subsequent requests made by ANY client using the same `MatrixHttpClient` instance will include this token in the `Authorization: Bearer` header.

### 2. RegisterClient Implementation

**Location**: [./packages/client/src/_matrix/client/v3/register/client.rs](../../packages/client/src/_matrix/client/src/_matrix/client/v3/register/client.rs)

**Core Methods:**

```rust
impl RegisterClient {
    /// Create new registration client
    pub fn new(http_client: MatrixHttpClient) -> Self
    
    /// Get available registration flows
    /// GET /_matrix/client/v3/register
    pub async fn get_registration_flows(&self) -> Result<RegistrationFlowsResponse, HttpClientError>
    
    /// Register a new user account with full UIA support
    /// POST /_matrix/client/v3/register
    pub async fn register(&self, request: &RegisterRequest) -> Result<RegisterResponse, HttpClientError>
    
    /// Convenience: Register with username/password
    pub async fn register_with_password(
        &self,
        username: &str,
        password: &str,
        device_display_name: Option<String>,
    ) -> Result<RegisterResponse, HttpClientError>
}
```

**Multi-Stage Authentication Handling:**

The `register()` method handles Matrix's User-Interactive Authentication (UIA) flow:

1. Client makes initial request (may include or omit `auth` field)
2. Server responds with 401 + available flows if additional auth required
3. Client completes auth stage and retries with `auth` field populated
4. Process repeats until all required stages complete
5. Server returns 200 + user_id/access_token

**RegisterRequest Builder Pattern:**

```rust
// In mod.rs - Type definitions
impl RegisterRequest {
    pub fn new(username: &str, password: &str) -> Self {
        Self {
            username: Some(username.to_string()),
            password: Some(password.to_string()),
            auth: None,
            device_id: None,
            initial_device_display_name: None,
            inhibit_login: Some(false),  // Request access_token in response
        }
    }
    
    pub fn with_display_name(mut self, name: String) -> Self {
        self.initial_device_display_name = Some(name);
        self
    }
    
    pub fn with_device_id(mut self, id: String) -> Self {
        self.device_id = Some(id);
        self
    }
    
    pub fn with_auth(mut self, auth: AuthData) -> Self {
        self.auth = Some(auth);
        self
    }
}
```

Usage:
```rust
let request = RegisterRequest::new("alice", "secret_password")
    .with_display_name("Alice's Phone".to_string());
    
let response = register_client.register(&request).await?;
```

### 3. Type Definitions

**Location**: [./packages/client/src/_matrix/client/v3/login/mod.rs](../../packages/client/src/_matrix/client/v3/login/mod.rs)

All request/response types are defined to match the Matrix spec exactly:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    #[serde(rename = "type")]
    pub login_type: String,           // "m.login.password" or "m.login.token"
    
    pub user: Option<String>,         // Username for password login
    pub password: Option<String>,     // Password for password login
    pub token: Option<String>,        // Token for token login
    
    pub device_id: Option<String>,    // Optional device ID (server generates if None)
    pub initial_device_display_name: Option<String>,  // Human-readable device name
    pub refresh_token: Option<bool>,  // Request refresh token (v1.3+)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub user_id: String,              // Fully-qualified Matrix user ID
    pub access_token: String,         // Bearer token for authenticated requests
    pub device_id: Option<String>,    // Device ID assigned by server
    pub well_known: Option<serde_json::Value>,  // Server discovery info
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginFlowsResponse {
    pub flows: Vec<LoginFlow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginFlow {
    #[serde(rename = "type")]
    pub flow_type: String,            // e.g., "m.login.password", "m.login.sso"
}
```

**Type Compatibility with Server:**
These types MUST match the server-side definitions in `packages/server/src/_matrix/client/v3/login/` to ensure request/response compatibility.

### 4. Module Exports and Re-exports

**v3/login/mod.rs:**
```rust
mod client;
mod types;  // (types defined inline in mod.rs)

pub use client::LoginClient;
pub use types::*;  // Export all type definitions
```

**v3/register/mod.rs:**
```rust
mod client;

pub use client::RegisterClient;
// Types defined inline, automatically exported
```

**lib.rs Convenience Module:**
```rust
pub mod auth {
    pub use crate::_matrix::client::v3::login::{LoginClient, LoginRequest, LoginResponse};
    pub use crate::_matrix::client::v3::register::{RegisterClient, RegisterRequest, RegisterResponse};
}
```

This allows ergonomic imports:
```rust
use matryx_client::auth::{LoginClient, RegisterClient};
```

### 5. Error Handling

All authentication operations return `Result<T, HttpClientError>` where `HttpClientError` provides:

```rust
pub enum HttpClientError {
    Network(reqwest::Error),
    
    Matrix {
        status: u16,
        errcode: String,      // e.g., "M_FORBIDDEN", "M_USER_IN_USE"
        error: String,        // Human-readable message
        retry_after_ms: Option<u64>,  // For rate limiting
    },
    
    InvalidResponse { status: u16, body: String, parse_error: String },
    Serialization(serde_json::Error),
    InvalidUrl(url::ParseError),
    AuthenticationRequired,
    MaxRetriesExceeded,
}
```

**Helper Methods:**
```rust
impl HttpClientError {
    pub fn is_retryable(&self) -> bool          // Check if retry makes sense
    pub fn retry_delay(&self) -> Option<Duration>  // Get recommended delay
    pub fn status_code(&self) -> Option<u16>    // Extract HTTP status
    pub fn is_client_error(&self) -> bool       // 4xx errors
    pub fn is_server_error(&self) -> bool       // 5xx errors
}
```

**Error Handling Example:**
```rust
match login_client.login_with_password("alice", "wrong_password", None, None).await {
    Ok(response) => {
        println!("Logged in as {}", response.user_id);
    },
    Err(HttpClientError::Matrix { errcode, error, .. }) if errcode == "M_FORBIDDEN" => {
        eprintln!("Invalid credentials: {}", error);
    },
    Err(HttpClientError::Matrix { errcode, retry_after_ms, .. }) if errcode == "M_LIMIT_EXCEEDED" => {
        if let Some(ms) = retry_after_ms {
            eprintln!("Rate limited. Retry after {}ms", ms);
        }
    },
    Err(e) => {
        eprintln!("Login failed: {}", e);
    }
}
```

---

## Key Patterns and Code Examples

### Pattern 1: MatrixHttpClient Generic Request Pattern

The foundation of all API clients is the generic request pattern in `MatrixHttpClient`:

```rust
// From packages/client/src/http_client.rs
pub async fn request<T, R>(
    &self,
    method: Method,
    path: &str,
    body: Option<&T>,
) -> Result<R, HttpClientError>
where
    T: Serialize,
    R: for<'de> Deserialize<'de>,
{
    // 1. Construct full URL
    let url = self.homeserver_url.join(path)?;
    
    // 2. Build request with method
    let mut req = self.client.request(method, url);
    
    // 3. Add Bearer token if available (thread-safe read)
    if let Some(token) = self.access_token.read().await.as_ref() {
        req = req.bearer_auth(token);
    }
    
    // 4. Add JSON body if provided
    if let Some(body) = body {
        req = req.json(body);
    }
    
    // 5. Send and parse response
    let response = req.send().await?;
    let status = response.status();
    
    if status.is_success() {
        Ok(response.json::<R>().await?)
    } else {
        // Parse Matrix error response
        let error_body = response.text().await?;
        self.parse_matrix_error(status.as_u16(), &error_body)
    }
}
```

**Why this pattern:**
- Type-safe at compile time (Rust's type system enforces correct types)
- Single implementation for all HTTP methods
- Automatic error handling and conversion
- No error-prone manual JSON handling

### Pattern 2: Convenience Methods Wrapping Generic Requests

Convenience methods in `MatrixHttpClient` wrap the generic `request()`:

```rust
pub async fn get<R>(&self, path: &str) -> Result<R, HttpClientError>
where
    R: for<'de> Deserialize<'de>,
{
    self.request::<(), R>(Method::GET, path, None).await
}

pub async fn post<T, R>(&self, path: &str, body: &T) -> Result<R, HttpClientError>
where
    T: Serialize,
    R: for<'de> Deserialize<'de>,
{
    self.request(Method::POST, path, Some(body)).await
}
```

**Usage in LoginClient:**
```rust
// GET request - no body needed
pub async fn get_login_flows(&self) -> Result<LoginFlowsResponse, HttpClientError> {
    self.http_client.get("/_matrix/client/v3/login").await
}

// POST request - with body
pub async fn login(&self, request: &LoginRequest) -> Result<LoginResponse, HttpClientError> {
    self.http_client.post("/_matrix/client/v3/login", request).await
}
```

### Pattern 3: Automatic Token Management

After authentication succeeds, tokens are automatically stored and used:

```rust
// In LoginClient::login()
pub async fn login(&self, request: &LoginRequest) -> Result<LoginResponse, HttpClientError> {
    let response: LoginResponse = self.http_client
        .post("/_matrix/client/v3/login", request)
        .await?;

    // THIS IS CRITICAL: Store token for all future requests
    self.http_client.set_access_token(response.access_token.clone()).await;

    Ok(response)
}
```

**Thread-Safe Token Storage:**
```rust
// In MatrixHttpClient
pub async fn set_access_token(&self, token: String) {
    let mut guard = self.access_token.write().await;  // Acquire write lock
    *guard = Some(token);                              // Update shared state
}
```

**Automatic Usage:**
All subsequent requests automatically include the token because the `request()` method checks for it:
```rust
if let Some(token) = self.access_token.read().await.as_ref() {
    req = req.bearer_auth(token);  // Adds "Authorization: Bearer <token>"
}
```

### Pattern 4: Builder Pattern for Complex Requests

`RegisterRequest` uses the builder pattern for ergonomic API:

```rust
impl RegisterRequest {
    pub fn new(username: &str, password: &str) -> Self {
        Self {
            username: Some(username.to_string()),
            password: Some(password.to_string()),
            auth: None,
            device_id: None,
            initial_device_display_name: None,
            inhibit_login: Some(false),
        }
    }
    
    // Builder methods return Self for chaining
    pub fn with_display_name(mut self, name: String) -> Self {
        self.initial_device_display_name = Some(name);
        self  // Enable chaining
    }
    
    pub fn with_auth(mut self, auth: AuthData) -> Self {
        self.auth = Some(auth);
        self
    }
}
```

**Usage:**
```rust
// Fluent API - reads like English
let request = RegisterRequest::new("bob", "secure_password")
    .with_display_name("Bob's Laptop".to_string())
    .with_device_id("DEVICE123".to_string());
```

### Pattern 5: Retry Logic with Exponential Backoff

`MatrixHttpClient` implements automatic retries for transient failures:

```rust
pub async fn request_with_retry<T, R>(
    &self,
    method: Method,
    path: &str,
    body: Option<&T>,
    max_retries: u32,
) -> Result<R, HttpClientError>
where
    T: Serialize,
    R: for<'de> Deserialize<'de>,
{
    let mut attempt = 0;

    loop {
        match self.request(method.clone(), path, body).await {
            Ok(response) => return Ok(response),
            Err(e) => {
                attempt += 1;
                
                // Use helper to check if retry makes sense
                if !e.is_retryable() || attempt >= max_retries {
                    return Err(e);
                }

                // Exponential backoff: 100ms * 2^(attempt-1)
                let delay_ms = 100 * 2u64.pow(attempt - 1);
                
                // Honor server's retry_after if rate limited
                let delay = if let HttpClientError::Matrix { retry_after_ms: Some(ms), .. } = &e {
                    Duration::from_millis(*ms)
                } else {
                    Duration::from_millis(delay_ms)
                };

                tokio::time::sleep(delay).await;
            }
        }
    }
}
```

**Retryable conditions:**
- Network errors (connection failures, timeouts)
- 5xx server errors (temporary server issues)
- `M_LIMIT_EXCEEDED` with `retry_after_ms`

**Non-retryable:**
- 4xx client errors (bad request, forbidden, etc.)
- Authentication failures
- Validation errors

---

## Source Code Architecture

### Dependency Graph

```
LoginClient / RegisterClient
        ↓
MatrixHttpClient (http_client.rs)
        ↓
   ┌────┴────┐
   ↓         ↓
reqwest   Url + serde_json
```

**Key Dependencies:**
- `reqwest` - HTTP client with async support
- `serde` / `serde_json` - Serialization framework
- `url` - URL parsing and joining
- `tokio` - Async runtime (for RwLock, sleep, etc.)
- `thiserror` - Error type derivation

### Type System Guarantees

Rust's type system provides compile-time guarantees:

1. **Type Safety**: Cannot accidentally pass wrong types to endpoints
2. **Lifetime Safety**: No use-after-free or dangling references
3. **Thread Safety**: `Arc<RwLock<T>>` ensures safe concurrent access
4. **Error Handling**: `Result<T, E>` forces explicit error handling
5. **Ownership**: Clear ownership semantics prevent memory leaks

---

## Changes to Source Files

### Files Created

1. **packages/client/src/_matrix/client/v3/login/client.rs**
   - `LoginClient` struct and implementation
   - All login-related methods
   - Automatic token management on login success

2. **packages/client/src/_matrix/client/v3/register/client.rs**
   - `RegisterClient` struct and implementation
   - Registration methods with UIA support
   - Automatic token management (if `inhibit_login=false`)

### Files Modified

3. **packages/client/src/_matrix/client/v3/login/mod.rs**
   - Added `mod client;` declaration
   - Added `pub use client::LoginClient;` export
   - Type definitions remain (LoginRequest, LoginResponse, LoginFlowsResponse)

4. **packages/client/src/_matrix/client/v3/register/mod.rs**
   - Created module structure
   - Added `mod client;` declaration
   - Added `pub use client::RegisterClient;` export
   - Type definitions (RegisterRequest, RegisterResponse, etc.)

5. **packages/client/src/_matrix/client/v3/mod.rs**
   - Added `pub mod register;` module declaration
   - Ensured `pub mod login;` already present

6. **packages/client/src/lib.rs**
   - Added `pub mod auth { ... }` convenience module
   - Re-exports LoginClient, RegisterClient, and related types
   - Enables `use matryx_client::auth::LoginClient;`

### Files Deprecated (Stub Created)

7. **packages/client/src/_matrix/client/v1/login.rs**
   - Marked as deprecated with migration guidance
   - Points developers to v3 API

8. **packages/client/src/_matrix/client/v1/register.rs**
   - Marked as deprecated with migration guidance
   - Points developers to v3 API

---

## Definition of Done

The implementation is considered complete when:

✅ **LoginClient Completeness:**
- [x] `LoginClient::new()` constructor implemented
- [x] `get_login_flows()` calls GET /_matrix/client/v3/login
- [x] `login()` calls POST /_matrix/client/v3/login
- [x] `login_with_password()` convenience method implemented
- [x] `login_with_token()` convenience method implemented
- [x] Access token automatically set on successful login

✅ **RegisterClient Completeness:**
- [x] `RegisterClient::new()` constructor implemented
- [x] `get_registration_flows()` calls GET /_matrix/client/v3/register
- [x] `register()` calls POST /_matrix/client/v3/register
- [x] `register_with_password()` convenience method implemented
- [x] Access token automatically set if returned (inhibit_login=false)

✅ **Type Definitions:**
- [x] `LoginRequest` / `LoginResponse` match Matrix spec
- [x] `RegisterRequest` / `RegisterResponse` match Matrix spec
- [x] Builder pattern implemented for `RegisterRequest`
- [x] All types are Serialize + Deserialize

✅ **Error Handling:**
- [x] All methods return `Result<T, HttpClientError>`
- [x] Matrix error codes properly parsed from responses
- [x] Rate limiting errors include `retry_after_ms`

✅ **Integration:**
- [x] Both clients use `MatrixHttpClient` for all requests
- [x] Module exports configured correctly in mod.rs files
- [x] Convenience re-exports in lib.rs
- [x] V1 stubs created with deprecation notices

✅ **Compilation:**
- [x] Package compiles without errors: `cargo check -p matryx_client`
- [x] No warnings in STUB_A implementation files
- [x] Types compatible with server-side implementations

---

## Verification Commands

```bash
# Verify package compiles
cargo check -p matryx_client

# Verify specific files compile
cargo check -p matryx_client --lib

# Check for warnings in authentication code
cargo clippy -p matryx_client -- -D warnings

# Verify types are properly exported
cargo doc -p matryx_client --no-deps --open
# Then navigate to auth module and verify LoginClient/RegisterClient appear
```

---

## Related Tasks

- **Server-Side Auth**: [./packages/server/src/_matrix/client/v3/login/](../../packages/server/src/_matrix/client/v3/login/)
- **Session Management**: [./packages/server/src/state.rs](../../packages/server/src/state.rs)
- **HTTP Infrastructure**: [./packages/client/src/http_client.rs](../../packages/client/src/http_client.rs)

---

## Implementation Status

**COMPLETE** - All requirements met, package compiles successfully.

Last Updated: 2025-10-10
