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
