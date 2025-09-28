use axum::{Json, extract::{Path, State}, http::StatusCode};
use serde_json::Value;
use tracing::{error, info};

use crate::state::AppState;
use matryx_surrealdb::repository::third_party_service::ThirdPartyService;

/// GET /_matrix/client/v3/thirdparty/location/{alias}
/// 
/// Retrieve Matrix room aliases from third-party networks.
/// This endpoint allows clients to query for Matrix room aliases that correspond
/// to locations or channels in third-party networks (like IRC channels, Discord servers, etc.)
pub async fn get(
    State(state): State<AppState>,
    Path(alias): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    info!("Third-party location lookup for alias: {}", alias);

    // Create third-party service
    let third_party_service = ThirdPartyService::new(state.db.clone());

    // Query all protocols to find locations matching the alias
    let protocols = match third_party_service.query_third_party_protocols().await {
        Ok(protocols) => protocols,
        Err(e) => {
            error!("Failed to query third-party protocols: {:?}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let mut locations = Vec::new();

    // Search across all protocols for locations matching the alias
    for (protocol_id, _protocol_config) in protocols {
        // Create search fields for alias lookup
        let mut search_fields = std::collections::HashMap::new();
        search_fields.insert("alias".to_string(), alias.clone());

        match third_party_service.lookup_location(&protocol_id, &search_fields).await {
            Ok(protocol_locations) => {
                for location in protocol_locations {
                    locations.push(serde_json::json!({
                        "alias": location.alias,
                        "protocol": location.protocol,
                        "fields": location.fields
                    }));
                }
            }
            Err(e) => {
                // Log error but continue searching other protocols
                error!("Failed to lookup locations in protocol {}: {:?}", protocol_id, e);
            }
        }
    }

    info!("Found {} third-party locations for alias: {}", locations.len(), alias);
    Ok(Json(serde_json::Value::Array(locations)))
}
