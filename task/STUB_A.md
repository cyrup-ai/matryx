# STUB_A: Implement Client Authentication APIs (Login/Register)

**Status**: Ready for Implementation  
**Priority**: CRITICAL  
**Estimated Effort**: 3-4 days (revised after research)  
**Package**: packages/client

---

## OBJECTIVE

Implement functional HTTP client code for Matrix authentication endpoints (login, register) using the existing MatrixHttpClient infrastructure. Focus on v3 endpoints (current Matrix spec) while maintaining v1 stubs for backward compatibility.

---

## RESEARCH FINDINGS

### Existing Infrastructure Discovered

#### 1. MatrixHttpClient (ALREADY EXISTS)
**Location**: [`packages/client/src/http_client.rs`](../packages/client/src/http_client.rs)

**Capabilities**:
- ✅ Generic request/response handling with type safety
- ✅ Matrix-spec-compliant error parsing (`MatrixErrorResponse`)
- ✅ Retry logic with exponential backoff
- ✅ Thread-safe authentication token management via `Arc<RwLock<Option<String>>>`
- ✅ Convenience methods: `get()`, `post()`, `put()`, `delete()`
- ✅ Method: `request_with_retry()` for resilient network calls

**Key Methods**:
```rust
pub async fn request<T, R>(&self, method: Method, path: &str, body: Option<&T>) -> Result<R, HttpClientError>
pub async fn set_access_token(&self, token: String)
pub async fn get_access_token(&self) -> Result<String, HttpClientError>
pub async fn clear_access_token(&self)
```

**Error Types**:
```rust
pub enum HttpClientError {
    Network(reqwest::Error),
    Matrix { status: u16, errcode: String, error: String, retry_after_ms: Option<u64> },
    Serialization(serde_json::Error),
    InvalidUrl(url::ParseError),
    AuthenticationRequired,
    MaxRetriesExceeded,
}
```

#### 2. V3 Login Implementation (ALREADY EXISTS)
**Location**: [`packages/client/src/_matrix/client/v3/login/mod.rs`](../packages/client/src/_matrix/client/v3/login/mod.rs)

**Current State**: Fully implemented with types and HTTP calls, BUT uses raw `reqwest::Client` instead of `MatrixHttpClient`

**Existing Types**:
```rust
pub struct LoginRequest {
    pub login_type: String,
    pub user: Option<String>,
    pub password: Option<String>,
    pub device_id: Option<String>,
    pub initial_device_display_name: Option<String>,
    pub token: Option<String>,
    pub refresh_token: Option<String>,
}

pub struct LoginResponse {
    pub user_id: String,
    pub access_token: String,
    pub device_id: String,
    pub refresh_token: Option<String>,
    pub expires_in_ms: Option<u64>,
    pub well_known: Option<Value>,
}
```

**Existing Functions**:
- `get_login_flows(client, homeserver_url)` - GET /_matrix/client/v3/login
- `login(client, homeserver_url, request)` - POST /_matrix/client/v3/login
- `login_with_password(...)` - Convenience for password auth
- `login_with_token(...)` - Convenience for token auth

#### 3. Server-Side Register Types (REFERENCE)
**Location**: [`packages/server/src/_matrix/client/v3/register/handlers.rs`](../packages/server/src/_matrix/client/v3/register/handlers.rs)

**Server Request Type**:
```rust
pub struct RegistrationRequest {
    pub username: Option<String>,
    pub password: Option<String>,
    pub device_id: Option<String>,
    pub initial_device_display_name: Option<String>,
    pub inhibit_login: bool,  // default: false
    pub refresh_token: bool,  // default: false
    pub auth: Option<Value>,  // User-Interactive Authentication
}
```

**Server Response Type**:
```rust
pub struct RegistrationResponse {
    pub user_id: String,
    pub access_token: Option<String>,  // None if inhibit_login=true
    pub device_id: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_in_ms: Option<i64>,
    pub well_known: Option<Value>,
}
```

#### 4. High-Level MatrixClient (EXISTS)
**Location**: [`packages/client/src/lib.rs`](../packages/client/src/lib.rs)

Has built-in login functionality at lines 126-138:
```rust
pub async fn login(&mut self, username: &str, password: &str, device_id: Option<String>) -> Result<()>
```

Uses internal `login_password()` method that constructs requests manually.

### V1 Endpoints Status

**Current State**: Both are minimal stubs with placeholder functions

**Files**:
- [`packages/client/src/_matrix/client/v1/login.rs`](../packages/client/src/_matrix/client/v1/login.rs) - 13 lines, stub only
- [`packages/client/src/_matrix/client/v1/register.rs`](../packages/client/src/_matrix/client/v1/register.rs) - 13 lines, stub only

**Matrix Specification Context**: 
- v1 endpoints are deprecated in favor of v3
- Modern Matrix implementations use v3 for client-server API
- v1 can remain as stubs or simple wrappers to v3

---

## PROBLEM DESCRIPTION (REVISED)

### Primary Issues

1. **v3/login uses raw reqwest::Client** instead of MatrixHttpClient
   - Missing retry logic
   - Missing standardized error handling
   - Duplicates HTTP client functionality

2. **No v3/register client implementation**
   - Server has types defined but no client equivalent
   - Prevents user registration from client library

3. **v1 stubs are placeholders**
   - Should either implement as v3 wrappers OR clearly document as deprecated

### Impact

- Client authentication works (via lib.rs MatrixClient) but inconsistently
- No access to advanced MatrixHttpClient features (retry, proper error handling)
- Cannot register new users from client library
- Codebase has duplication between lib.rs and v3/login

---

## REVISED IMPLEMENTATION STRATEGY

### Core Principle: Use Existing Code

**DO NOT DUPLICATE** the following that already exist:
- ✅ MatrixHttpClient - use it directly
- ✅ LoginRequest/LoginResponse types in v3 - reuse them
- ✅ HttpClientError - use for error handling
- ✅ Server's RegistrationRequest/RegistrationResponse - mirror them

---

## SUBTASK 1: Create LoginClient Using MatrixHttpClient

**Objective**: Create a proper client wrapper for v3 login that uses MatrixHttpClient instead of raw reqwest.

**Location**: Create NEW file `packages/client/src/_matrix/client/v3/login/client.rs`

**Why New File**: Keep existing `mod.rs` functions for backward compatibility, add new client-based API alongside.

**Implementation**:

```rust
//! Matrix login client using MatrixHttpClient
//!
//! This module provides a structured client for Matrix login operations
//! using the centralized MatrixHttpClient infrastructure.

use crate::http_client::{MatrixHttpClient, HttpClientError};
use super::{LoginRequest, LoginResponse, LoginFlowsResponse, LoginFlow, UserIdentifier};
use serde::{Deserialize, Serialize};

/// Client for Matrix login operations using MatrixHttpClient
pub struct LoginClient {
    http_client: MatrixHttpClient,
}

impl LoginClient {
    /// Create new login client with MatrixHttpClient
    pub fn new(http_client: MatrixHttpClient) -> Self {
        Self { http_client }
    }

    /// Get available login flows from server
    ///
    /// Performs GET /_matrix/client/v3/login to discover supported
    /// authentication methods.
    ///
    /// # Returns
    /// - `Ok(LoginFlowsResponse)` with available login types
    /// - `Err(HttpClientError)` on network/server errors
    pub async fn get_login_flows(&self) -> Result<LoginFlowsResponse, HttpClientError> {
        self.http_client
            .get("/_matrix/client/v3/login")
            .await
    }

    /// Perform login request
    ///
    /// Sends POST /_matrix/client/v3/login with credentials.
    ///
    /// # Arguments
    /// * `request` - Login request with credentials and device info
    ///
    /// # Returns
    /// - `Ok(LoginResponse)` with access_token and user_id on success
    /// - `Err(HttpClientError::Matrix)` with M_FORBIDDEN on invalid credentials
    /// - `Err(HttpClientError::Matrix)` with M_LIMIT_EXCEEDED on rate limiting
    /// - `Err(HttpClientError::Network)` on connection failures
    pub async fn login(&self, request: &LoginRequest) -> Result<LoginResponse, HttpClientError> {
        let response: LoginResponse = self.http_client
            .post("/_matrix/client/v3/login", request)
            .await?;

        // Set the access token for future authenticated requests
        self.http_client.set_access_token(response.access_token.clone()).await;

        Ok(response)
    }

    /// Login with username and password (convenience method)
    ///
    /// Creates a password-type LoginRequest and performs login.
    ///
    /// # Arguments
    /// * `username` - User identifier (localpart without @server)
    /// * `password` - User's password
    /// * `device_id` - Optional device ID (server generates if None)
    /// * `device_display_name` - Optional human-readable device name
    pub async fn login_with_password(
        &self,
        username: &str,
        password: &str,
        device_id: Option<String>,
        device_display_name: Option<String>,
    ) -> Result<LoginResponse, HttpClientError> {
        let request = LoginRequest {
            login_type: "m.login.password".to_string(),
            user: Some(username.to_string()),
            password: Some(password.to_string()),
            device_id,
            initial_device_display_name: device_display_name,
            token: None,
            refresh_token: None,
        };

        self.login(&request).await
    }

    /// Login with token (convenience method)
    ///
    /// Used for SSO, application service, or pre-authenticated tokens.
    ///
    /// # Arguments
    /// * `token` - Pre-authenticated token from SSO or app service
    /// * `device_id` - Optional device ID
    /// * `device_display_name` - Optional human-readable device name
    pub async fn login_with_token(
        &self,
        token: &str,
        device_id: Option<String>,
        device_display_name: Option<String>,
    ) -> Result<LoginResponse, HttpClientError> {
        let request = LoginRequest {
            login_type: "m.login.token".to_string(),
            user: None,
            password: None,
            device_id,
            initial_device_display_name: device_display_name,
            token: Some(token.to_string()),
            refresh_token: None,
        };

        self.login(&request).await
    }
}
```

**Files to Modify**:
1. **CREATE**: `packages/client/src/_matrix/client/v3/login/client.rs`
2. **UPDATE**: `packages/client/src/_matrix/client/v3/login/mod.rs` - add `pub mod client;` and re-export

**Definition of Done**:
- LoginClient struct uses MatrixHttpClient
- All login methods implemented with proper error types
- Automatically sets access token on successful login
- Re-exported from login module

---

## SUBTASK 2: Create RegisterClient with MatrixHttpClient

**Objective**: Implement v3 register client mirroring server-side types.

**Location**: Create NEW directory and files:
- `packages/client/src/_matrix/client/v3/register/`
- `packages/client/src/_matrix/client/v3/register/mod.rs`
- `packages/client/src/_matrix/client/v3/register/client.rs`

**Implementation**:

### File: `packages/client/src/_matrix/client/v3/register/mod.rs`

```rust
//! Matrix client registration API
//!
//! Implements POST /_matrix/client/v3/register per Matrix specification.
//!
//! Reference: ../../../server/src/_matrix/client/v3/register/handlers.rs

pub mod client;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use client::RegisterClient;

/// Registration request body matching server-side RegistrationRequest
#[derive(Debug, Clone, Serialize)]
pub struct RegisterRequest {
    /// Desired username (localpart only, without @user:server)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,

    /// Password for the account
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,

    /// Device ID (optional, server generates if not provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,

    /// Initial device display name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_device_display_name: Option<String>,

    /// If true, don't automatically log in (no access_token returned)
    #[serde(default)]
    pub inhibit_login: bool,

    /// Whether client supports refresh tokens
    #[serde(default)]
    pub refresh_token: bool,

    /// User-Interactive Authentication data (for CAPTCHA, email verification, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<Value>,
}

/// Registration response body matching server-side RegistrationResponse
#[derive(Debug, Clone, Deserialize)]
pub struct RegisterResponse {
    /// The fully-qualified Matrix user ID (MXID) created
    pub user_id: String,

    /// Access token (None if inhibit_login was true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,

    /// Device ID for this session
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,

    /// Refresh token (if requested and supported)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,

    /// Access token lifetime in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in_ms: Option<i64>,

    /// Well-known client configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub well_known: Option<Value>,
}

/// Available registration flows (from GET /_matrix/client/v3/register)
#[derive(Debug, Clone, Deserialize)]
pub struct RegistrationFlowsResponse {
    pub flows: Vec<RegistrationFlow>,
}

/// Single registration flow describing required auth stages
#[derive(Debug, Clone, Deserialize)]
pub struct RegistrationFlow {
    /// Ordered list of authentication stages
    pub stages: Vec<String>,
}

impl RegisterRequest {
    /// Create basic registration request with username and password
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: Some(username.into()),
            password: Some(password.into()),
            device_id: None,
            initial_device_display_name: None,
            inhibit_login: false,
            refresh_token: false,
            auth: None,
        }
    }

    /// Add User-Interactive Authentication data
    pub fn with_auth(mut self, auth: Value) -> Self {
        self.auth = Some(auth);
        self
    }

    /// Set device ID
    pub fn with_device_id(mut self, device_id: impl Into<String>) -> Self {
        self.device_id = Some(device_id.into());
        self
    }

    /// Set initial device display name
    pub fn with_display_name(mut self, name: impl Into<String>) -> Self {
        self.initial_device_display_name = Some(name.into());
        self
    }

    /// Don't automatically log in after registration
    pub fn inhibit_login(mut self) -> Self {
        self.inhibit_login = true;
        self
    }

    /// Request refresh token support
    pub fn with_refresh_token(mut self) -> Self {
        self.refresh_token = true;
        self
    }
}
```

### File: `packages/client/src/_matrix/client/v3/register/client.rs`

```rust
//! Registration client implementation using MatrixHttpClient

use crate::http_client::{MatrixHttpClient, HttpClientError};
use super::{RegisterRequest, RegisterResponse, RegistrationFlowsResponse};

/// Client for Matrix registration operations
pub struct RegisterClient {
    http_client: MatrixHttpClient,
}

impl RegisterClient {
    /// Create new registration client
    pub fn new(http_client: MatrixHttpClient) -> Self {
        Self { http_client }
    }

    /// Get available registration flows
    ///
    /// Queries GET /_matrix/client/v3/register to discover what
    /// authentication stages are required on this homeserver.
    ///
    /// # Returns
    /// - `Ok(RegistrationFlowsResponse)` with available flows
    /// - `Err(HttpClientError)` on network/server errors
    pub async fn get_registration_flows(&self) -> Result<RegistrationFlowsResponse, HttpClientError> {
        self.http_client
            .get("/_matrix/client/v3/register")
            .await
    }

    /// Register a new user account
    ///
    /// Sends POST /_matrix/client/v3/register with account details.
    ///
    /// # Arguments
    /// * `request` - Registration request with username, password, and auth data
    ///
    /// # Returns
    /// - `Ok(RegisterResponse)` with user_id and optional access_token
    /// - `Err(HttpClientError::Matrix)` with specific error codes:
    ///   - M_USER_IN_USE (400) - Username already taken
    ///   - M_INVALID_USERNAME (400) - Username format invalid
    ///   - M_WEAK_PASSWORD (400) - Password doesn't meet requirements
    ///   - M_UNAUTHORIZED (401) - Additional auth stages required
    ///
    /// # Multi-Stage Authentication
    ///
    /// If server returns 401, the response will contain `flows` indicating
    /// required auth stages (CAPTCHA, email verification, etc.). Client must
    /// complete these stages and retry with `auth` field populated.
    pub async fn register(&self, request: &RegisterRequest) -> Result<RegisterResponse, HttpClientError> {
        let response: RegisterResponse = self.http_client
            .post("/_matrix/client/v3/register", request)
            .await?;

        // If access_token was returned (inhibit_login=false), set it for future requests
        if let Some(ref token) = response.access_token {
            self.http_client.set_access_token(token.clone()).await;
        }

        Ok(response)
    }

    /// Register with username and password (convenience method)
    ///
    /// Creates a basic registration request and submits it.
    ///
    /// # Arguments
    /// * `username` - Desired username (localpart only)
    /// * `password` - Account password
    /// * `device_display_name` - Optional human-readable device name
    ///
    /// # Note
    /// This may fail with M_UNAUTHORIZED if server requires additional
    /// auth stages (CAPTCHA, email verification). Use `register()` with
    /// full `RegisterRequest` including `auth` field for multi-stage flows.
    pub async fn register_with_password(
        &self,
        username: &str,
        password: &str,
        device_display_name: Option<String>,
    ) -> Result<RegisterResponse, HttpClientError> {
        let mut request = RegisterRequest::new(username, password);
        
        if let Some(name) = device_display_name {
            request = request.with_display_name(name);
        }

        self.register(&request).await
    }
}
```

**Files to Create**:
1. **CREATE**: `packages/client/src/_matrix/client/v3/register/` directory
2. **CREATE**: `packages/client/src/_matrix/client/v3/register/mod.rs`
3. **CREATE**: `packages/client/src/_matrix/client/v3/register/client.rs`

**Files to Modify**:
1. **UPDATE**: `packages/client/src/_matrix/client/v3/mod.rs` - add `pub mod register;`

**Definition of Done**:
- RegisterRequest/RegisterResponse types match server exactly
- RegisterClient uses MatrixHttpClient
- get_registration_flows() queries available auth methods
- register() handles multi-stage auth responses
- Automatically sets access_token on successful registration
- Builder pattern for RegisterRequest

---

## SUBTASK 3: Update Module Exports

**Objective**: Make new clients accessible from public API.

**Files to Modify**:

### 1. `packages/client/src/_matrix/client/v3/login/mod.rs`

Add at the top:
```rust
pub mod client;
pub use client::LoginClient;
```

Keep existing functions for backward compatibility.

### 2. `packages/client/src/_matrix/client/v3/mod.rs`

Add:
```rust
pub mod register;
```

### 3. `packages/client/src/_matrix/client/mod.rs`

Already has:
```rust
pub mod v3;
```

Verify v3 is public.

### 4. `packages/client/src/lib.rs`

Add convenience re-exports at end of file (optional but recommended):
```rust
// Re-export authentication clients
pub mod auth {
    pub use crate::_matrix::client::v3::login::{LoginClient, LoginRequest, LoginResponse};
    pub use crate::_matrix::client::v3::register::{RegisterClient, RegisterRequest, RegisterResponse};
}
```

**Definition of Done**:
- LoginClient accessible via `matryx_client::auth::LoginClient`
- RegisterClient accessible via `matryx_client::auth::RegisterClient`
- All request/response types exported
- No broken module references
- Compilation succeeds

---

## SUBTASK 4: Handle V1 Stubs

**Objective**: Document v1 as deprecated, optionally implement as thin wrappers.

**Option A: Leave as stubs with documentation** (RECOMMENDED)

Update both files with:

#### `packages/client/src/_matrix/client/v1/login.rs`
```rust
//! Matrix Client-Server API v1 Login (DEPRECATED)
//!
//! **NOTE**: v1 endpoints are deprecated per Matrix specification.
//! Use v3 endpoints instead: `crate::_matrix::client::v3::login`
//!
//! Modern Matrix implementations should use:
//! - `crate::_matrix::client::v3::login::LoginClient`
//! - POST /_matrix/client/v3/login
//!
//! This module exists for backward compatibility only.

#[deprecated(since = "0.1.0", note = "Use v3::login::LoginClient instead")]
pub mod deprecated_stub {
    /// Placeholder for deprecated v1 login
    /// 
    /// Use `crate::_matrix::client::v3::login::LoginClient` instead.
    pub fn placeholder() {
        // Implementation would call v3 login, but v1 is deprecated
    }
}
```

#### `packages/client/src/_matrix/client/v1/register.rs`
```rust
//! Matrix Client-Server API v1 Register (DEPRECATED)
//!
//! **NOTE**: v1 endpoints are deprecated per Matrix specification.
//! Use v3 endpoints instead: `crate::_matrix::client::v3::register`
//!
//! Modern Matrix implementations should use:
//! - `crate::_matrix::client::v3::register::RegisterClient`
//! - POST /_matrix/client/v3/register
//!
//! This module exists for backward compatibility only.

#[deprecated(since = "0.1.0", note = "Use v3::register::RegisterClient instead")]
pub mod deprecated_stub {
    /// Placeholder for deprecated v1 register
    /// 
    /// Use `crate::_matrix::client::v3::register::RegisterClient` instead.
    pub fn placeholder() {
        // Implementation would call v3 register, but v1 is deprecated
    }
}
```

**Option B: Implement as v3 wrappers** (if backward compatibility needed)

Only do this if there are actual consumers of v1 API. Otherwise keep as stubs.

**Files to Modify**:
1. `packages/client/src/_matrix/client/v1/login.rs`
2. `packages/client/src/_matrix/client/v1/register.rs`

**Definition of Done**:
- V1 files clearly marked as deprecated
- Documentation points to v3 alternatives
- No functionality loss (v1 was already stubs)

---

## DEPENDENCIES & REFERENCES

### Existing Crates (Already in Cargo.toml)
From [`packages/client/Cargo.toml`](../packages/client/Cargo.toml):
- ✅ `reqwest = "0.12"` - HTTP client (already used by MatrixHttpClient)
- ✅ `serde = "1.0.228"` - Serialization
- ✅ `serde_json = "1.0.145"` - JSON handling
- ✅ `url = "2.5"` - URL parsing
- ✅ `tokio = "1.47.1"` - Async runtime
- ✅ `thiserror = "2.0.17"` - Error handling
- ✅ `anyhow = "1.0.100"` - Error context

**No new dependencies required.**

### Matrix Specification Reference
**Location**: [`tmp/matrix-spec-official/`](../tmp/matrix-spec-official/)

**Relevant Sections**:
- Client-Server API Authentication
- POST /_matrix/client/v3/login
- GET /_matrix/client/v3/login
- POST /_matrix/client/v3/register
- GET /_matrix/client/v3/register
- User-Interactive Authentication (UIA) flows

### Code References

| Component | File Path | Purpose |
|-----------|-----------|---------|
| MatrixHttpClient | [`packages/client/src/http_client.rs`](../packages/client/src/http_client.rs) | HTTP client infrastructure |
| Existing v3 Login | [`packages/client/src/_matrix/client/v3/login/mod.rs`](../packages/client/src/_matrix/client/v3/login/mod.rs) | Types and implementations to refactor |
| Server Register Types | [`packages/server/src/_matrix/client/v3/register/handlers.rs`](../packages/server/src/_matrix/client/v3/register/handlers.rs) | Reference for client types |
| High-level Client | [`packages/client/src/lib.rs`](../packages/client/src/lib.rs) | Integration point |

---

## IMPLEMENTATION SEQUENCE

### Phase 1: Login Client (Day 1)
1. Create `packages/client/src/_matrix/client/v3/login/client.rs`
2. Implement LoginClient using MatrixHttpClient
3. Update login/mod.rs exports
4. Test compilation

### Phase 2: Register Client (Day 2)
1. Create `packages/client/src/_matrix/client/v3/register/` directory
2. Create register/mod.rs with types
3. Create register/client.rs with RegisterClient
4. Update v3/mod.rs exports
5. Test compilation

### Phase 3: Module Integration (Day 2-3)
1. Update `packages/client/src/lib.rs` with convenience exports
2. Update v1 stubs with deprecation notices
3. Test all module paths resolve correctly
4. Verify no compilation errors

### Phase 4: Verification (Day 3)
1. Ensure MatrixHttpClient is used throughout
2. Verify error handling matches HttpClientError patterns
3. Check token management (set_access_token on successful auth)
4. Confirm v3 is primary, v1 is marked deprecated

---

## DEFINITION OF DONE

- [ ] LoginClient created using MatrixHttpClient
- [ ] LoginClient implements get_login_flows(), login(), login_with_password(), login_with_token()
- [ ] RegisterClient created using MatrixHttpClient
- [ ] RegisterClient implements get_registration_flows(), register(), register_with_password()
- [ ] RegisterRequest/RegisterResponse match server-side types exactly
- [ ] Module exports updated (v3/mod.rs, lib.rs)
- [ ] V1 stubs documented as deprecated with clear migration path
- [ ] No compilation errors
- [ ] All types properly Serialize/Deserialize
- [ ] Access tokens automatically set on successful login/register
- [ ] Error handling uses HttpClientError throughout

---

## WHAT NOT TO DO

- ❌ **NO NEW DEPENDENCIES** - Use existing reqwest, serde, etc.
- ❌ **NO DUPLICATION** - Reuse MatrixHttpClient, don't create new HTTP client
- ❌ **NO RAW REQWEST** - Always use MatrixHttpClient wrapper
- ❌ **NO BREAKING CHANGES** - Keep existing v3/login/mod.rs functions for compatibility
- ❌ **NO V1 IMPLEMENTATION** - Mark as deprecated, don't implement new v1 code

---

## CONSTRAINTS REMINDER

⚠️ **NO TESTS**: Do not write unit tests, integration tests, or test fixtures.

⚠️ **NO BENCHMARKS**: Do not write benchmark code.

⚠️ **NO DOCUMENTATION FILES**: Do not create README.md, DESIGN.md, or other markdown docs.

⚠️ **FOCUS ON SOURCE CODE**: Only modify .rs files in packages/client/src/

---

## KEY INSIGHTS FROM RESEARCH

### What Already Works
1. MatrixHttpClient provides production-ready HTTP infrastructure
2. v3/login has all necessary types defined
3. Server has validated RegisterRequest/RegisterResponse types
4. High-level MatrixClient in lib.rs already does login (can be refactored later)

### What Needs Building
1. Structured LoginClient wrapper (new)
2. Complete RegisterClient implementation (new)
3. Module exports and convenience re-exports
4. Clear deprecation notices for v1

### Why This Approach
- **Leverage existing code**: Don't rebuild what works (MatrixHttpClient)
- **Match server types**: Ensure client/server compatibility
- **Structured clients**: Better than loose functions for API discovery
- **Token management**: Automatic token setting reduces user error
- **Future-proof**: v3 is current spec, v1 is legacy

---

## EXAMPLE USAGE (Post-Implementation)

### Login Example
```rust
use matryx_client::auth::{LoginClient, RegisterClient};
use matryx_client::http_client::MatrixHttpClient;
use url::Url;

let homeserver = Url::parse("https://matrix.example.com")?;
let http_client = MatrixHttpClient::new(homeserver)?;

// Login
let login_client = LoginClient::new(http_client.clone());
let response = login_client
    .login_with_password("alice", "secret123", None, Some("My Device".into()))
    .await?;

println!("Logged in as: {}", response.user_id);
println!("Access token: {}", response.access_token);
// Token is automatically set in http_client
```

### Register Example
```rust
let register_client = RegisterClient::new(http_client.clone());

// Check required flows first
let flows = register_client.get_registration_flows().await?;
println!("Available flows: {:?}", flows);

// Simple registration
let response = register_client
    .register_with_password("bob", "password456", Some("Bob's Phone".into()))
    .await?;

println!("Registered user: {}", response.user_id);
// Token is automatically set if inhibit_login=false
```

---

## NOTES

- This implementation enables **structured authentication** - the foundation for all Matrix client operations
- Login and register are the **most critical client APIs** - everything else requires authentication
- Multi-stage auth (CAPTCHA, email verification) handled via `auth` field in requests
- Device ID should be persisted across sessions by calling application
- **Access tokens must NOT be logged** - ensure no debug logging of sensitive fields
- The v3 implementation is **production-ready** when using MatrixHttpClient's retry and error handling
- Consider adding **refresh token support** in future iterations (already in response types)

---

**Implementation Time Estimate**: 3-4 days for complete, tested, integrated solution  
**Complexity**: Medium (types exist, infrastructure exists, assembly required)  
**Risk**: Low (well-defined Matrix spec, existing patterns to follow)
