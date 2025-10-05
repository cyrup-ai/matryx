use crate::utils::response_helpers::matrix_response;
use axum::response::IntoResponse;
use serde_json::json;

pub async fn get() -> impl IntoResponse {
    matrix_response(json!({
        "versions": ["r0.6.1", "v1.1", "v1.2", "v1.3", "v1.4", "v1.5", "v1.6", "v1.7", "v1.8", "v1.9", "v1.10", "v1.11"],
        "unstable_features": {}
    }))
}
