use axum::extract::ConnectInfo;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use tracing::{error, info};

use crate::auth::{MatrixAuthError, extract_matrix_auth};
use crate::state::AppState;
use matryx_surrealdb::repository::third_party_service::ThirdPartyService;

#[derive(Serialize, Deserialize)]
pub struct Protocol {
    pub user_fields: Vec<FieldType>,
    pub location_fields: Vec<FieldType>,
    pub icon: String,
    pub field_types: HashMap<String, FieldType>,
    pub instances: Vec<ProtocolInstance>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FieldType {
    pub regexp: String,
    pub placeholder: String,
}

#[derive(Serialize, Deserialize)]
pub struct ProtocolInstance {
    pub desc: String,
    pub icon: Option<String>,
    pub fields: HashMap<String, String>,
    pub network_id: String,
}

/// GET /_matrix/client/v3/thirdparty/protocols
pub async fn get(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Result<Json<HashMap<String, Protocol>>, StatusCode> {
    // Authenticate user
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

    info!("Third-party protocols request from user {} at {}", user_id, addr);

    // TASK17 SUBTASK 4: Use ThirdPartyService instead of direct queries
    let third_party_service = ThirdPartyService::new(state.db.clone());
    
    let protocols_map = match third_party_service.query_third_party_protocols().await {
        Ok(protocols) => protocols,
        Err(e) => {
            error!("Failed to query third-party protocols: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Convert to response format
    let mut response = HashMap::new();

    for (protocol_id, protocol_config) in protocols_map {
        // Convert repository types to server types
        let user_fields: Vec<FieldType> = protocol_config.user_fields
            .into_iter()
            .map(|f| FieldType {
                regexp: f.regexp,
                placeholder: f.placeholder,
            })
            .collect();

        let location_fields: Vec<FieldType> = protocol_config.location_fields
            .into_iter()
            .map(|f| FieldType {
                regexp: f.regexp,
                placeholder: f.placeholder,
            })
            .collect();

        let instances: Vec<ProtocolInstance> = protocol_config.instances
            .into_iter()
            .map(|i| ProtocolInstance {
                desc: i.desc,
                icon: i.icon,
                fields: i.fields,
                network_id: i.network_id,
            })
            .collect();

        // Build field_types map
        let mut field_types: HashMap<String, FieldType> = HashMap::new();
        for field in &user_fields {
            field_types.insert(format!("user.{}", field.placeholder), field.clone());
        }
        for field in &location_fields {
            field_types.insert(format!("location.{}", field.placeholder), field.clone());
        }

        let protocol = Protocol {
            user_fields,
            location_fields,
            icon: protocol_config.avatar_url.unwrap_or_else(|| "mxc://".to_string()),
            field_types,
            instances,
        };

        response.insert(protocol_id, protocol);
    }

    // If no protocols configured, return empty map
    if response.is_empty() {
        info!("No third-party protocols configured");
    }

    Ok(Json(response))
}
