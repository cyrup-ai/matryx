use axum::http::{HeaderName, Method, header};
use tower_http::cors::{Any, CorsLayer};

/// Create CORS layer compliant with Matrix Client-Server API specification
///
/// Matrix specification requires:
/// - Access-Control-Allow-Origin: *
/// - Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS
/// - Access-Control-Allow-Headers: X-Requested-With, Content-Type, Authorization
pub fn create_cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            HeaderName::from_static("x-requested-with"),
        ])
}
