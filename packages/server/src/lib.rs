// Deny unwrap/expect to enforce proper error handling
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
// Allow unwrap/expect in test code
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod _matrix;
pub mod _well_known;
pub mod auth;
pub mod cache;
pub mod config;
pub mod crypto;
pub mod email;
pub mod error;
pub mod event_replacements;
pub mod federation;
pub mod mentions;
pub mod metrics;
pub mod middleware;
pub mod migration;
pub mod monitoring;
pub mod performance;
pub mod reactions;
pub mod response;
pub mod room;
pub mod security;
pub mod server_notices;
pub mod state;
pub mod tasks;
pub mod threading;
pub mod utils;

pub use crate::auth::MatrixSessionService;
pub use crate::config::ServerConfig;
pub use crate::state::AppState;

use tower_cookies::Cookie;

/// Create a secure session cookie with proper security attributes
pub fn create_secure_session_cookie(name: &str, value: &str) -> Cookie<'static> {
    Cookie::build((name.to_owned(), value.to_owned()))
        .http_only(true) // Prevent XSS
        .secure(true) // HTTPS only
        .same_site(tower_cookies::cookie::SameSite::Lax) // CSRF protection
        .max_age(tower_cookies::cookie::time::Duration::hours(24)) // 24 hour expiry
        .path("/") // Available site-wide
        .build()
}
