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
