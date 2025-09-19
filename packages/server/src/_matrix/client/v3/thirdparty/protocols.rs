use axum::extract::ConnectInfo;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::net::SocketAddr;
use tracing::{error, info};

use crate::auth::{MatrixAuthError, authenticate_user};
use crate::state::AppState;

#[derive(Serialize, Deserialize)]
pub struct Protocol {
    pub user_fields: Vec<FieldType>,
    pub location_fields: Vec<FieldType>,
    pub icon: String,
    pub field_types: HashMap<String, FieldType>,
    pub instances: Vec<ProtocolInstance>,
}

#[derive(Serialize, Deserialize)]
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
    let auth_result = authenticate_user(&state, &headers).await;
    let user_id = match auth_result {
        Ok(user_id) => user_id,
        Err(MatrixAuthError::MissingToken) => return Err(StatusCode::UNAUTHORIZED),
        Err(MatrixAuthError::InvalidToken) => return Err(StatusCode::UNAUTHORIZED),
        Err(MatrixAuthError::ExpiredToken) => return Err(StatusCode::UNAUTHORIZED),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    info!("Third-party protocols request from user {} at {}", user_id, addr);

    // Query available protocols from database
    let query = r#"
        SELECT protocol_id, display_name, avatar_url, user_fields, location_fields, instances
        FROM thirdparty_protocols
    "#;

    let protocols = match state.db.query(query).await {
        Ok(mut result) => {
            match result
                .take::<Vec<(String, String, Option<String>, Vec<Value>, Vec<Value>, Vec<Value>)>>(
                    0,
                ) {
                Ok(protocols) => protocols,
                Err(e) => {
                    error!("Failed to parse third-party protocols: {}", e);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                },
            }
        },
        Err(e) => {
            error!("Failed to query third-party protocols: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        },
    };

    // Convert to response format
    let mut response = HashMap::new();

    for (protocol_id, display_name, avatar_url, user_fields, location_fields, instances) in
        protocols
    {
        // Parse field types
        let user_field_types: Vec<FieldType> = user_fields
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect();

        let location_field_types: Vec<FieldType> = location_fields
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect();

        let protocol_instances: Vec<ProtocolInstance> = instances
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect();

        // Build field_types map
        let mut field_types = HashMap::new();
        for field in &user_field_types {
            field_types.insert(format!("user.{}", field.placeholder), field.clone());
        }
        for field in &location_field_types {
            field_types.insert(format!("location.{}", field.placeholder), field.clone());
        }

        let protocol = Protocol {
            user_fields: user_field_types,
            location_fields: location_field_types,
            icon: avatar_url.unwrap_or_else(|| "mxc://".to_string()),
            field_types,
            instances: protocol_instances,
        };

        response.insert(protocol_id, protocol);
    }

    // If no protocols configured, return empty map
    if response.is_empty() {
        info!("No third-party protocols configured");
    }

    Ok(Json(response))
}
