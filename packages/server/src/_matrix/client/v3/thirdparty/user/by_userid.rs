use axum::{
    Json,
    http::StatusCode,
};
use serde_json::Value;
use tracing::{error, info};

use crate::state::AppState;
use matryx_surrealdb::repository::third_party_service::ThirdPartyService;

/// Core logic for looking up third-party users by userid
/// Used by GET /_matrix/client/v3/thirdparty/user?userid={userid}
pub async fn get_by_userid(
    state: AppState,
    userid: String,
) -> Result<Json<Value>, StatusCode> {
    info!("Third-party user lookup for user ID: {}", userid);

    // Create third-party service
    let third_party_service = ThirdPartyService::new(state.db.clone());

    // Query all protocols to find users matching the user ID
    let protocols = match third_party_service.query_third_party_protocols().await {
        Ok(protocols) => protocols,
        Err(e) => {
            error!("Failed to query third-party protocols: {:?}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        },
    };

    let mut users = Vec::new();

    // Search across all protocols for users matching the user ID
    for (protocol_id, _protocol_config) in protocols {
        // Create search fields for user lookup
        let mut search_fields = std::collections::HashMap::new();
        search_fields.insert("userid".to_string(), userid.clone());

        match third_party_service.lookup_user(&protocol_id, &search_fields).await {
            Ok(protocol_users) => {
                for user in protocol_users {
                    users.push(serde_json::json!({
                        "userid": user.userid,
                        "protocol": user.protocol,
                        "fields": user.fields
                    }));
                }
            },
            Err(e) => {
                // Log error but continue searching other protocols
                error!("Failed to lookup users in protocol {}: {:?}", protocol_id, e);
            },
        }
    }

    info!("Found {} third-party users for user ID: {}", users.len(), userid);
    Ok(Json(serde_json::Value::Array(users)))
}
