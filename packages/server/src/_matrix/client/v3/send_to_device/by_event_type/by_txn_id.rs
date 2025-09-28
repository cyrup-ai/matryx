use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};

use serde::Deserialize;
use serde_json::{Value, json};
use tracing::{error, info};
use uuid::Uuid;

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};
use matryx_surrealdb::repository::messaging::{MessagingRepository, ToDeviceMessage};

#[derive(Deserialize)]
pub struct SendToDeviceRequest {
    messages: std::collections::HashMap<String, std::collections::HashMap<String, Value>>,
}



/// PUT /_matrix/client/v3/sendToDevice/{eventType}/{txnId}
pub async fn put(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((event_type, txn_id)): Path<(String, String)>,
    Json(request): Json<SendToDeviceRequest>,
) -> Result<Json<Value>, StatusCode> {
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        error!("Send-to-device failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let sender_user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        _ => return Err(StatusCode::FORBIDDEN),
    };

    // Check for transaction ID idempotency
    let messaging_repo = MessagingRepository::new(state.db.clone());
    if messaging_repo
        .check_transaction_exists(&sender_user_id, &txn_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        // Transaction already processed - return success (idempotent)
        return Ok(Json(json!({})));
    }

    // Process messages for each user and device
    for (user_id, device_messages) in request.messages {
        for (device_id, content) in device_messages {
            let message_id = Uuid::new_v4().to_string();

            let to_device_message = ToDeviceMessage {
                id: message_id,
                sender: sender_user_id.clone(),
                event_type: event_type.clone(),
                content,
                target_user_id: user_id.clone(),
                target_device_id: if device_id == "*" {
                    None
                } else {
                    Some(device_id)
                },
                txn_id: txn_id.clone(),
                created_at: chrono::Utc::now(),
            };

            // Store message for delivery via /sync
            messaging_repo
                .store_to_device_message(&to_device_message)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }

    info!(
        "Send-to-device messages queued for delivery: event_type={}, txn_id={}",
        event_type, txn_id
    );
    Ok(Json(json!({})))
}
