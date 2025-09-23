use crate::state::AppState;
use axum::{Json, extract::State, http::StatusCode};
use matryx_surrealdb::repository::capabilities::CapabilitiesRepository;
use serde_json::{Value, json};

/// GET /_matrix/client/v3/capabilities
pub async fn get(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    // Create capabilities repository
    let capabilities_repo = CapabilitiesRepository::new(state.db.clone());

    // Get server capabilities
    let capabilities = match capabilities_repo.get_server_capabilities().await {
        Ok(caps) => caps,
        Err(e) => {
            tracing::error!("Failed to get server capabilities: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        },
    };

    // Get unstable features
    let unstable_features = match capabilities_repo.get_unstable_features().await {
        Ok(features) => features,
        Err(e) => {
            tracing::error!("Failed to get unstable features: {}", e);
            std::collections::HashMap::new()
        },
    };

    // Convert to Matrix API format
    let mut response = json!({
        "capabilities": {
            "m.change_password": {
                "enabled": capabilities.change_password
            },
            "m.room_versions": {
                "default": capabilities.room_versions.default,
                "available": capabilities.room_versions.available
            },
            "m.set_displayname": {
                "enabled": capabilities.set_displayname
            },
            "m.set_avatar_url": {
                "enabled": capabilities.set_avatar_url
            },
            "m.3pid_changes": {
                "enabled": capabilities.threepid_changes
            },
            "m.get_login_token": {
                "enabled": capabilities.get_login_token
            },
            "org.matrix.lazy_loading": {
                "enabled": capabilities.lazy_loading
            },
            "org.matrix.e2e_cross_signing": {
                "enabled": capabilities.e2e_cross_signing
            },
            "org.matrix.spaces": {
                "enabled": capabilities.spaces
            },
            "org.matrix.threading": {
                "enabled": capabilities.threading
            }
        }
    });

    // Add custom capabilities
    if let Some(capabilities_obj) = response["capabilities"].as_object_mut() {
        for (key, value) in capabilities.custom_capabilities {
            capabilities_obj.insert(key, value);
        }

        // Add unstable features
        if !unstable_features.is_empty() {
            for (feature, enabled) in unstable_features {
                capabilities_obj.insert(feature, json!({"enabled": enabled}));
            }
        }
    }

    Ok(Json(response))
}
