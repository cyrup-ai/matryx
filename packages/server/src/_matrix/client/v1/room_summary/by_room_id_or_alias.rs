use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v1/room_summary/{roomIdOrAlias}
pub async fn get(Path(_room_id_or_alias): Path<String>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "room_id": "!example:example.com",
        "canonical_alias": "#example:example.com",
        "guest_can_join": true,
        "join_rule": "public",
        "name": "Example Room",
        "num_joined_members": 42,
        "room_type": null,
        "topic": "An example room",
        "world_readable": true,
        "avatar_url": "mxc://example.com/avatar"
    })))
}
