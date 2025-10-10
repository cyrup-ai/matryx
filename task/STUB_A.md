# STUB_A: Implement Client Authentication APIs (Login/Register)

**Status**: Ready for Implementation
**Priority**: CRITICAL
**Estimated Effort**: 1 week
**Package**: packages/client

---

## OBJECTIVE

Replace stub implementations with functional HTTP client code for Matrix authentication endpoints (login, register) to enable basic client authentication.

---

## PROBLEM DESCRIPTION

Client authentication endpoints are currently stubs with no implementation:

**Files Affected**:
- `packages/client/src/_matrix/client/v1/login.rs`
- `packages/client/src/_matrix/client/v1/register.rs`

**Current State**:
```rust
//! Client stub for _matrix/client/v1/login.rs
//! This is a placeholder stub for the client implementation.
//! The actual HTTP client functionality should be implemented here
//! using reqwest to make outbound HTTP requests.

pub mod client_stub {
    pub fn placeholder() {
        // Client implementation would go here
    }
}
```

**Impact**:
- Clients cannot authenticate to Matrix homeservers
- Login functionality completely non-functional
- User registration impossible
- All authenticated operations blocked

---

## RESEARCH NOTES

**Matrix Specification**:
- Login endpoint: `POST /_matrix/client/v3/login`
- Register endpoint: `POST /_matrix/client/v3/register`
- Both require JSON request/response bodies
- Authentication types: password, token, SSO

**Required Dependencies** (already in Cargo.toml):
- reqwest: HTTP client
- serde/serde_json: JSON serialization
- url: URL handling

**HTTP Client Pattern**:
```rust
pub struct LoginClient {
    http_client: Arc<HttpClient>,
    base_url: Url,
}
```

---

## SUBTASK 1: Implement Login Request/Response Types

**Objective**: Define strongly-typed request and response structures for login.

**Location**: `packages/client/src/_matrix/client/v1/login.rs`

**Implementation**:

Replace stub with:
```rust
//! Matrix client login endpoint implementation
//!
//! Implements POST /_matrix/client/v3/login per Matrix specification.

use crate::http_client::{HttpClient, HttpClientError};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use url::Url;

/// Login request body
#[derive(Debug, Clone, Serialize)]
pub struct LoginRequest {
    /// Authentication type (e.g., "m.login.password")
    #[serde(rename = "type")]
    pub login_type: String,

    /// User identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifier: Option<UserIdentifier>,

    /// Password for password-based login
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,

    /// Token for token-based login
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,

    /// Device ID (optional, server generates if not provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,

    /// Initial device display name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_device_display_name: Option<String>,
}

/// User identifier for login
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum UserIdentifier {
    #[serde(rename = "m.id.user")]
    User {
        user: String,
    },
    #[serde(rename = "m.id.thirdparty")]
    ThirdParty {
        medium: String,
        address: String,
    },
    #[serde(rename = "m.id.phone")]
    Phone {
        country: String,
        phone: String,
    },
}

/// Login response body
#[derive(Debug, Clone, Deserialize)]
pub struct LoginResponse {
    /// User ID that logged in
    pub user_id: String,

    /// Access token for authentication
    pub access_token: String,

    /// Device ID for this session
    pub device_id: String,

    /// Homeserver name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub home_server: Option<String>,

    /// Well-known server discovery info
    #[serde(skip_serializing_if = "Option::is_none")]
    pub well_known: Option<serde_json::Value>,
}

impl LoginRequest {
    /// Create password-based login request
    pub fn password(user: String, password: String) -> Self {
        Self {
            login_type: "m.login.password".to_string(),
            identifier: Some(UserIdentifier::User { user }),
            password: Some(password),
            token: None,
            device_id: None,
            initial_device_display_name: None,
        }
    }

    /// Create token-based login request
    pub fn token(token: String) -> Self {
        Self {
            login_type: "m.login.token".to_string(),
            identifier: None,
            password: None,
            token: Some(token),
            device_id: None,
            initial_device_display_name: None,
        }
    }

    /// Set device ID for this login
    pub fn with_device_id(mut self, device_id: String) -> Self {
        self.device_id = Some(device_id);
        self
    }

    /// Set initial device display name
    pub fn with_display_name(mut self, name: String) -> Self {
        self.initial_device_display_name = Some(name);
        self
    }
}
```

**Files to Modify**:
- `packages/client/src/_matrix/client/v1/login.rs`

**Definition of Done**:
- LoginRequest and LoginResponse types defined
- All Matrix-required fields present
- Builder methods for common login types
- Proper serde annotations

---

## SUBTASK 2: Implement LoginClient with HTTP Calls

**Objective**: Create functional login client that makes actual HTTP requests.

**Location**: `packages/client/src/_matrix/client/v1/login.rs`

**Implementation**:

Add client implementation:
```rust
/// Client for Matrix login operations
pub struct LoginClient {
    http_client: Arc<HttpClient>,
    base_url: Url,
}

impl LoginClient {
    /// Create new login client
    pub fn new(http_client: Arc<HttpClient>, base_url: Url) -> Self {
        Self {
            http_client,
            base_url,
        }
    }

    /// Perform login request
    ///
    /// # Arguments
    /// * `request` - Login request with credentials
    ///
    /// # Returns
    /// Login response with access token and user ID
    ///
    /// # Errors
    /// - Network errors
    /// - Invalid credentials (M_FORBIDDEN)
    /// - Rate limiting (M_LIMIT_EXCEEDED)
    pub async fn login(&self, request: LoginRequest) -> Result<LoginResponse, HttpClientError> {
        let url = self.base_url
            .join("/_matrix/client/v3/login")
            .map_err(|e| HttpClientError::InvalidUrl(e.to_string()))?;

        tracing::debug!("Sending login request to {}", url);

        let response = self.http_client
            .post(url)
            .json(&request)
            .send()
            .await?;

        let status = response.status();

        if status.is_success() {
            let login_response: LoginResponse = response
                .json()
                .await
                .map_err(|e| HttpClientError::ParseError(e.to_string()))?;

            tracing::info!("Login successful for user {}", login_response.user_id);

            Ok(login_response)
        } else {
            let body = response.text().await.unwrap_or_default();
            tracing::warn!("Login failed with status {}: {}", status, body);

            // Parse Matrix error or return generic error
            Err(HttpClientError::from_response_body(status.as_u16(), &body))
        }
    }

    /// Get available login types from server
    ///
    /// Queries GET /_matrix/client/v3/login to discover supported
    /// authentication methods.
    pub async fn get_login_flows(&self) -> Result<Vec<String>, HttpClientError> {
        let url = self.base_url
            .join("/_matrix/client/v3/login")
            .map_err(|e| HttpClientError::InvalidUrl(e.to_string()))?;

        let response = self.http_client
            .get(url)
            .send()
            .await?;

        #[derive(Deserialize)]
        struct FlowsResponse {
            flows: Vec<Flow>,
        }

        #[derive(Deserialize)]
        struct Flow {
            #[serde(rename = "type")]
            flow_type: String,
        }

        let flows_response: FlowsResponse = response
            .json()
            .await
            .map_err(|e| HttpClientError::ParseError(e.to_string()))?;

        Ok(flows_response.flows.into_iter().map(|f| f.flow_type).collect())
    }
}
```

**Files to Modify**:
- `packages/client/src/_matrix/client/v1/login.rs`

**Definition of Done**:
- LoginClient struct implemented
- login() method makes real HTTP POST requests
- get_login_flows() method queries available auth types
- Proper error handling and logging
- No unwrap() or expect() calls

---

## SUBTASK 3: Implement Register Request/Response Types

**Objective**: Define types for user registration endpoint.

**Location**: `packages/client/src/_matrix/client/v1/register.rs`

**Implementation**:

Replace stub with:
```rust
//! Matrix client registration endpoint implementation
//!
//! Implements POST /_matrix/client/v3/register per Matrix specification.

use crate::http_client::{HttpClient, HttpClientError};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use url::Url;

/// Registration request body
#[derive(Debug, Clone, Serialize)]
pub struct RegisterRequest {
    /// Authentication data (e.g., dummy, password, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<serde_json::Value>,

    /// Desired username (without @user:server part)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,

    /// Password for the account
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,

    /// Device ID (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,

    /// Initial device display name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_device_display_name: Option<String>,

    /// Inhibit login (get user_id but no access_token)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inhibit_login: Option<bool>,
}

/// Registration response body
#[derive(Debug, Clone, Deserialize)]
pub struct RegisterResponse {
    /// The created user ID
    pub user_id: String,

    /// Access token (unless inhibit_login was true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,

    /// Device ID for this session
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,

    /// Homeserver name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub home_server: Option<String>,
}

/// Registration flows response (from GET)
#[derive(Debug, Clone, Deserialize)]
pub struct RegistrationFlowsResponse {
    /// Available registration flows
    pub flows: Vec<RegistrationFlow>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegistrationFlow {
    /// List of stages in this flow
    pub stages: Vec<String>,
}

impl RegisterRequest {
    /// Create basic registration request with username and password
    pub fn new(username: String, password: String) -> Self {
        Self {
            auth: None,
            username: Some(username),
            password: Some(password),
            device_id: None,
            initial_device_display_name: None,
            inhibit_login: None,
        }
    }

    /// Add authentication data (for multi-stage registration)
    pub fn with_auth(mut self, auth: serde_json::Value) -> Self {
        self.auth = Some(auth);
        self
    }

    /// Set device ID
    pub fn with_device_id(mut self, device_id: String) -> Self {
        self.device_id = Some(device_id);
        self
    }

    /// Set initial device display name
    pub fn with_display_name(mut self, name: String) -> Self {
        self.initial_device_display_name = Some(name);
        self
    }

    /// Don't automatically log in after registration
    pub fn inhibit_login(mut self) -> Self {
        self.inhibit_login = Some(true);
        self
    }
}
```

**Files to Modify**:
- `packages/client/src/_matrix/client/v1/register.rs`

**Definition of Done**:
- RegisterRequest and RegisterResponse types defined
- RegistrationFlowsResponse for querying flows
- Builder methods for common scenarios
- Proper serde annotations

---

## SUBTASK 4: Implement RegisterClient with HTTP Calls

**Objective**: Create functional registration client.

**Location**: `packages/client/src/_matrix/client/v1/register.rs`

**Implementation**:

Add client:
```rust
/// Client for Matrix registration operations
pub struct RegisterClient {
    http_client: Arc<HttpClient>,
    base_url: Url,
}

impl RegisterClient {
    /// Create new registration client
    pub fn new(http_client: Arc<HttpClient>, base_url: Url) -> Self {
        Self {
            http_client,
            base_url,
        }
    }

    /// Get available registration flows
    ///
    /// Queries what authentication stages are required for registration
    /// on this homeserver.
    pub async fn get_registration_flows(&self) -> Result<RegistrationFlowsResponse, HttpClientError> {
        let url = self.base_url
            .join("/_matrix/client/v3/register")
            .map_err(|e| HttpClientError::InvalidUrl(e.to_string()))?;

        let response = self.http_client
            .get(url)
            .send()
            .await?;

        let flows: RegistrationFlowsResponse = response
            .json()
            .await
            .map_err(|e| HttpClientError::ParseError(e.to_string()))?;

        Ok(flows)
    }

    /// Register a new user account
    ///
    /// # Arguments
    /// * `request` - Registration request with username and auth data
    ///
    /// # Returns
    /// Registration response with user_id and optional access_token
    ///
    /// # Errors
    /// - Username already taken (M_USER_IN_USE)
    /// - Invalid username (M_INVALID_USERNAME)
    /// - Weak password (M_WEAK_PASSWORD)
    /// - Missing auth stage (401 with flows)
    pub async fn register(&self, request: RegisterRequest) -> Result<RegisterResponse, HttpClientError> {
        let url = self.base_url
            .join("/_matrix/client/v3/register")
            .map_err(|e| HttpClientError::InvalidUrl(e.to_string()))?;

        tracing::debug!("Sending registration request to {}", url);

        let response = self.http_client
            .post(url)
            .json(&request)
            .send()
            .await?;

        let status = response.status();

        if status.is_success() {
            let register_response: RegisterResponse = response
                .json()
                .await
                .map_err(|e| HttpClientError::ParseError(e.to_string()))?;

            tracing::info!("Registration successful for user {}", register_response.user_id);

            Ok(register_response)
        } else {
            let body = response.text().await.unwrap_or_default();
            tracing::warn!("Registration failed with status {}: {}", status, body);

            Err(HttpClientError::from_response_body(status.as_u16(), &body))
        }
    }
}
```

**Files to Modify**:
- `packages/client/src/_matrix/client/v1/register.rs`

**Definition of Done**:
- RegisterClient implemented
- register() method makes real HTTP requests
- get_registration_flows() queries available flows
- Proper error handling for common registration errors
- Logging for debugging

---

## SUBTASK 5: Update Module Exports and Integration

**Objective**: Ensure new clients are exported and usable.

**Location**: `packages/client/src/_matrix/client/v1/mod.rs`

**Implementation**:

Update module file:
```rust
//! Matrix Client-Server API v1 endpoints

pub mod login;
pub mod register;

pub use login::{LoginClient, LoginRequest, LoginResponse, UserIdentifier};
pub use register::{RegisterClient, RegisterRequest, RegisterResponse, RegistrationFlowsResponse};
```

**Files to Modify**:
- `packages/client/src/_matrix/client/v1/mod.rs`
- `packages/client/src/_matrix/client/mod.rs` (if needed)
- `packages/client/src/lib.rs` (if needed)

**Definition of Done**:
- All types properly exported
- Clients accessible from root client module
- No broken module references

---

## CONSTRAINTS

⚠️ **NO TESTS**: Do not write unit tests, integration tests, or test fixtures. Test team handles all testing.

⚠️ **NO BENCHMARKS**: Do not write benchmark code. Performance team handles benchmarking.

⚠️ **FOCUS ON FUNCTIONALITY**: Only modify production code in ./src directories.

---

## DEPENDENCIES

**Matrix Specification**:
- Clone: https://github.com/matrix-org/matrix-spec
- Section: Client-Server API - Authentication
- Endpoints: POST /login, GET /login, POST /register, GET /register

**Rust Crates** (already in Cargo.toml):
- reqwest (HTTP client)
- serde/serde_json (serialization)
- url (URL handling)
- tokio (async runtime)

**Existing Code**:
- HttpClient wrapper (assumed to exist in packages/client/src/http_client.rs)
- HttpClientError types

---

## DEFINITION OF DONE

- [ ] login.rs stub completely replaced with functional implementation
- [ ] register.rs stub completely replaced with functional implementation
- [ ] All request/response types defined
- [ ] HTTP POST/GET requests implemented
- [ ] Error handling for Matrix error codes
- [ ] Logging for all operations
- [ ] Module exports updated
- [ ] No compilation errors
- [ ] No test code written
- [ ] No benchmark code written

---

## FILES TO MODIFY

1. `packages/client/src/_matrix/client/v1/login.rs` (replace entire file)
2. `packages/client/src/_matrix/client/v1/register.rs` (replace entire file)
3. `packages/client/src/_matrix/client/v1/mod.rs` (update exports)

---

## NOTES

- This enables basic authentication - the foundation for all client operations
- Login is required before any authenticated API calls
- Multi-stage auth (CAPTCHA, email verification) handled via auth field
- Device ID should be persisted across sessions
- Access token must be stored securely (not logged)
- These are the most critical client APIs to implement first
