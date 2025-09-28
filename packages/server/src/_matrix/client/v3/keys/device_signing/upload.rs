use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{error, info};

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};

#[derive(Deserialize)]
pub struct DeviceSigningUploadRequest {
    pub master_key: Option<CrossSigningKey>,
    pub self_signing_key: Option<CrossSigningKey>,
    pub user_signing_key: Option<CrossSigningKey>,
    pub auth: Option<Value>,
}

#[derive(Serialize, Deserialize)]
pub struct CrossSigningKey {
    pub user_id: String,
    pub usage: Vec<String>,
    pub keys: std::collections::HashMap<String, String>,
    pub signatures: std::collections::HashMap<String, std::collections::HashMap<String, String>>,
}

fn validate_cross_signing_key(
    key: &CrossSigningKey,
    user_id: &str,
    expected_usage: &str,
) -> Result<(), StatusCode> {
    // Validate user_id matches
    if key.user_id != user_id {
        error!("Cross-signing key user_id mismatch");
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate usage
    if !key.usage.contains(&expected_usage.to_string()) {
        error!("Cross-signing key missing required usage: {}", expected_usage);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate key format
    if key.keys.is_empty() {
        error!("Cross-signing key missing keys");
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate signature exists
    if !key.signatures.contains_key(user_id) {
        error!("Cross-signing key missing user signature");
        return Err(StatusCode::BAD_REQUEST);
    }

    Ok(())
}

/// POST /_matrix/client/v3/keys/device_signing/upload
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DeviceSigningUploadRequest>,
) -> Result<Json<Value>, StatusCode> {
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        error!("Device signing upload failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        _ => return Err(StatusCode::FORBIDDEN),
    };

    // Handle User Interactive Authentication (UIA) if provided
    // According to Matrix spec, device signing key upload may require UIA for cross-signing setup
    if let Some(auth_data) = &request.auth {
        info!(
            "Device signing upload with UIA auth data for user: {} - Type: {}",
            user_id,
            auth_data.get("type").and_then(|v| v.as_str()).unwrap_or("unknown")
        );
        // TODO: Implement proper UIA (User Interactive Authentication) validation
        // This should validate the auth data against current UIA session flows
    }

    // Store master key
    if let Some(master_key) = request.master_key {
        validate_cross_signing_key(&master_key, &user_id, "master")?;

        let _: Option<CrossSigningKey> = state
            .db
            .create(("cross_signing_keys", format!("{}:master", user_id)))
            .content(json!({
                "user_id": user_id,
                "key_type": "master",
                "key_data": master_key,
                "signatures": master_key.signatures,
                "created_at": Utc::now()
            }))
            .await
            .map_err(|e| {
                error!("Failed to store master key: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        info!("Master key uploaded for user: {}", user_id);
    }

    // Store self-signing key
    if let Some(self_signing_key) = request.self_signing_key {
        validate_cross_signing_key(&self_signing_key, &user_id, "self_signing")?;

        let _: Option<CrossSigningKey> = state
            .db
            .create(("cross_signing_keys", format!("{}:self_signing", user_id)))
            .content(json!({
                "user_id": user_id,
                "key_type": "self_signing",
                "key_data": self_signing_key,
                "signatures": self_signing_key.signatures,
                "created_at": Utc::now()
            }))
            .await
            .map_err(|e| {
                error!("Failed to store self-signing key: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        info!("Self-signing key uploaded for user: {}", user_id);
    }

    // Store user-signing key
    if let Some(user_signing_key) = request.user_signing_key {
        validate_cross_signing_key(&user_signing_key, &user_id, "user_signing")?;

        let _: Option<CrossSigningKey> = state
            .db
            .create(("cross_signing_keys", format!("{}:user_signing", user_id)))
            .content(json!({
                "user_id": user_id,
                "key_type": "user_signing",
                "key_data": user_signing_key,
                "signatures": user_signing_key.signatures,
                "created_at": Utc::now()
            }))
            .await
            .map_err(|e| {
                error!("Failed to store user-signing key: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        info!("User-signing key uploaded for user: {}", user_id);
    }

    Ok(Json(json!({})))
}
