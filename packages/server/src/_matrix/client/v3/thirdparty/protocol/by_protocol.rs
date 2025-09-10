use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/thirdparty/protocol/{protocol}
pub async fn get(Path(_protocol): Path<String>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "user_fields": [],
        "location_fields": [],
        "icon": null,
        "field_types": {},
        "instances": []
    })))
}
