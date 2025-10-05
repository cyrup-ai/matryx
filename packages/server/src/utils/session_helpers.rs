use tower_cookies::Cookie;

/// Create a secure session cookie with Matrix-compliant security settings
///
/// This function creates HTTP-only, secure cookies with appropriate SameSite and
/// expiration settings for Matrix server session management.
pub fn create_secure_session_cookie(name: &str, value: &str) -> Cookie<'static> {
    Cookie::build((name.to_owned(), value.to_owned()))
        .http_only(true) // Prevent XSS
        .secure(true) // HTTPS only
        .same_site(tower_cookies::cookie::SameSite::Lax) // CSRF protection
        .max_age(tower_cookies::cookie::time::Duration::hours(24)) // 24 hour expiry
        .path("/") // Available site-wide
        .build()
}
