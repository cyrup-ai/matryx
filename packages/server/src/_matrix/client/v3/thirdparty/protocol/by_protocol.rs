use axum::{Json, extract::{Path, State}, http::{HeaderMap, StatusCode}};
use serde::Serialize;
use std::collections::HashMap;
use tracing::{error, info};

use crate::auth::{MatrixAuthError, extract_matrix_auth};
use crate::state::AppState;
use matryx_surrealdb::repository::third_party_service::ThirdPartyService;

#[derive(Serialize)]
pub struct FieldType {
    pub regexp: String,
    pub placeholder: String,
}

#[derive(Serialize)]
pub struct ProtocolInstance {
    pub desc: String,
    pub icon: Option<String>,
    pub fields: HashMap<String, String>,
    pub network_id: String,
}

#[derive(Serialize)]
pub struct ProtocolResponse {
    pub user_fields: Vec<FieldType>,
    pub location_fields: Vec<FieldType>,
    pub icon: Option<String>,
    pub field_types: HashMap<String, FieldType>,
    pub instances: Vec<ProtocolInstance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge_status: Option<String>,
}

/// GET /_matrix/client/v3/thirdparty/protocol/{protocol}
pub async fn get(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ProtocolResponse>, StatusCode> {
    // TASK17 SUBTASK 7: Authenticate user
    let auth_result = extract_matrix_auth(&headers, &state.session_service).await;
    let matrix_auth = match auth_result {
        Ok(auth) => auth,
        Err(MatrixAuthError::MissingToken) => return Err(StatusCode::UNAUTHORIZED),
        Err(MatrixAuthError::MissingAuthorization) => return Err(StatusCode::UNAUTHORIZED),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let user_id = match matrix_auth {
        crate::auth::MatrixAuth::User(user_auth) => user_auth.user_id,
        _ => return Err(StatusCode::UNAUTHORIZED),
    };

    info!("Third-party protocol info request for '{}' from user {}", protocol, user_id);

    // Use ThirdPartyService and BridgeRepository for protocol information
    let third_party_service = ThirdPartyService::new(state.db.clone());
    
    let protocol_config = match third_party_service.third_party_repo().get_protocol_by_id(&protocol).await {
        Ok(Some(config)) => config,
        Ok(None) => {
            error!("Protocol '{}' not found", protocol);
            return Err(StatusCode::NOT_FOUND);
        },
        Err(e) => {
            error!("Failed to get protocol '{}': {}", protocol, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Get bridge status for this protocol
    let bridges = match third_party_service.bridge_repo().get_bridges_for_protocol(&protocol).await {
        Ok(bridges) => bridges,
        Err(e) => {
            error!("Failed to get bridges for protocol '{}': {}", protocol, e);
            Vec::new() // Continue without bridge info
        }
    };

    // Build field_types map
    let mut field_types: HashMap<String, FieldType> = HashMap::new();
    for field in &protocol_config.user_fields {
        field_types.insert(format!("user.{}", field.placeholder), FieldType {
            regexp: field.regexp.clone(),
            placeholder: field.placeholder.clone(),
        });
    }
    for field in &protocol_config.location_fields {
        field_types.insert(format!("location.{}", field.placeholder), FieldType {
            regexp: field.regexp.clone(),
            placeholder: field.placeholder.clone(),
        });
    }

    // Convert protocol instances
    let instances: Vec<ProtocolInstance> = protocol_config.instances
        .into_iter()
        .map(|instance| ProtocolInstance {
            desc: instance.desc,
            icon: instance.icon,
            fields: instance.fields,
            network_id: instance.network_id,
        })
        .collect();

    // Determine bridge status based on available bridges
    let bridge_status = if bridges.is_empty() {
        Some("no_bridges_available".to_string())
    } else {
        Some(format!("{}_bridges_active", bridges.len()))
    };

    let response = ProtocolResponse {
        user_fields: protocol_config.user_fields.into_iter().map(|f| FieldType {
            regexp: f.regexp,
            placeholder: f.placeholder,
        }).collect(),
        location_fields: protocol_config.location_fields.into_iter().map(|f| FieldType {
            regexp: f.regexp,
            placeholder: f.placeholder,
        }).collect(),
        icon: protocol_config.avatar_url,
        field_types,
        instances,
        bridge_status,
    };

    Ok(Json(response))
}
