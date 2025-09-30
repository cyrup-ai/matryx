use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::error;


use crate::auth::AuthenticatedUser;
use crate::state::AppState;
use crate::event_replacements::ReplacementValidator;
use crate::mentions::MentionsProcessor;

use matryx_entity::types::MembershipState;
use matryx_surrealdb::repository::{
    EventRepository,
    MembershipRepository,
    PowerLevelsRepository,
    RoomRepository,
};

#[derive(Deserialize)]
pub struct SendEventRequest {
    #[serde(flatten)]
    content: Value,
}

#[derive(Serialize)]
pub struct SendEventResponse {
    event_id: String,
}

/// PUT /_matrix/client/v3/rooms/{roomId}/send/{eventType}/{txnId}
pub async fn put(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((room_id, event_type, txn_id)): Path<(String, String, String)>,
    Json(request): Json<SendEventRequest>,
) -> Result<Json<SendEventResponse>, StatusCode> {
    // Initialize repositories
    let event_repo = EventRepository::new(state.db.clone());
    let room_repo = RoomRepository::new(state.db.clone());
    let membership_repo = MembershipRepository::new(state.db.clone());
    let power_levels_repo = PowerLevelsRepository::new(state.db.clone());

    // Check if transaction ID has been used before (idempotency)
    if let Some(existing_event_id) = event_repo
        .check_transaction_idempotency(&auth.user_id, &txn_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Ok(Json(SendEventResponse { event_id: existing_event_id }));
    }

    // Verify room exists
    let _room = room_repo
        .get_by_id(&room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Check if user is authorized to send events in this room
    if !auth.can_access_room(&state, room_id.to_string()).await {
        return Err(StatusCode::FORBIDDEN);
    }

    // Check resource-level access for specific event types that require elevated permissions
    let can_access = auth.can_access_resource(&state, "room", room_id.as_str()).await
        .map_err(|e| {
            error!("Failed to check room access for user {} in room {}: {}", auth.user_id, room_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    if !can_access {
        return Err(StatusCode::FORBIDDEN);
    }

    // Verify user is joined to the room
    let membership = membership_repo
        .get_by_room_user(&room_id, &auth.user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::FORBIDDEN)?;

    if membership.membership != MembershipState::Join {
        return Err(StatusCode::FORBIDDEN);
    }

    // Validate event type
    if !is_valid_event_type(&event_type) {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Get room's current power levels for permission check
    let power_levels = power_levels_repo
        .get_power_levels(&room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Check if user has permission to send this event type
    if !can_send_event_type(&auth.user_id, &event_type, &power_levels) {
        return Err(StatusCode::FORBIDDEN);
    }

    // Check if this is a replacement event (message edit)
    if let Some(relates_to) = request.content.get("m.relates_to") {
        if relates_to.get("rel_type").and_then(|v| v.as_str()) == Some("m.replace") {
            if let Some(event_id) = relates_to.get("event_id").and_then(|v| v.as_str()) {
                // Validate replacement using ReplacementValidator
                let replacement_validator = ReplacementValidator::new(&state);
                
                // Create a temporary event JSON for validation
                let mut temp_event = request.content.clone();
                temp_event["room_id"] = serde_json::json!(room_id);
                
                replacement_validator
                    .validate_replacement(event_id, &temp_event, &auth.user_id)
                    .await
                    .map_err(|e| {
                        tracing::warn!("Replacement validation failed: {}", e);
                        StatusCode::FORBIDDEN
                    })?;
                    
                tracing::info!("Validated replacement for event {} in room {}", event_id, room_id);
            }
        }
    }

    // Process mentions in message content
    let mentions_processor = MentionsProcessor::new();
    let mentions_metadata = mentions_processor
        .process_mentions(&request.content, &room_id, &auth.user_id, &state)
        .await
        .map_err(|e| {
            tracing::error!("Failed to process mentions: {}", e);
            // Don't fail the request if mention processing fails
            // Just log the error and continue
        })
        .ok()
        .flatten();

    // Add mentions metadata to event content if present
    let mut event_content = request.content.clone();
    if let Some(mentions) = mentions_metadata {
        // Add m.mentions to content
        let mut mentions_json = serde_json::Map::new();
        if let Some(user_ids) = mentions.user_ids {
            mentions_json.insert(
                "user_ids".to_string(),
                serde_json::json!(user_ids)
            );
        }
        if let Some(room) = mentions.room {
            mentions_json.insert(
                "room".to_string(),
                serde_json::json!(room)
            );
        }
        event_content["m.mentions"] = serde_json::Value::Object(mentions_json);
        
        tracing::info!("Processed mentions for event in room {}", room_id);
    }

    // Create complete event with DAG relationships using repository
    let mut event = event_repo
        .create_complete_event(
            &room_id,
            &event_type,
            &auth.matrix_id(),
            event_content.clone(),
            None, // Message events don't have state keys
            Some(txn_id.clone()),
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Calculate content hashes according to Matrix specification
    let hashes_value = crate::utils::matrix_events::calculate_content_hashes(&event)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let hashes: HashMap<String, String> =
        serde_json::from_value(hashes_value).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    event.hashes = Some(hashes);

    // Sign event with server's Ed25519 private key
    let signatures_value = crate::utils::matrix_events::sign_event(&state, &event)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let signatures: HashMap<String, HashMap<String, String>> =
        serde_json::from_value(signatures_value).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    event.signatures = Some(signatures);

    // Update the event in database with hashes and signatures
    let updated_event = event_repo
        .create(&event)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Update room's latest event timestamp
    room_repo
        .update_room_latest_event(&room_id, &event.event_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // If this was a replacement event, store the relationship
    if let Some(relates_to) = event_content.get("m.relates_to") {
        if relates_to.get("rel_type").and_then(|v| v.as_str()) == Some("m.replace") {
            if let Some(original_event_id) = relates_to.get("event_id").and_then(|v| v.as_str()) {
                let replacement_validator = ReplacementValidator::new(&state);
                
                // Create event JSON with the generated event_id
                let mut replacement_event = event_content.clone();
                replacement_event["event_id"] = serde_json::json!(&updated_event.event_id);
                replacement_event["room_id"] = serde_json::json!(&room_id);
                
                if let Err(e) = replacement_validator
                    .apply_replacement(original_event_id, &replacement_event)
                    .await
                {
                    tracing::warn!(
                        "Failed to store replacement relationship for event {}: {}",
                        updated_event.event_id, e
                    );
                    // Don't fail the request if storage fails
                    // The event was already created successfully
                } else {
                    tracing::info!(
                        "Stored replacement: {} replaces {} in room {}",
                        updated_event.event_id, original_event_id, room_id
                    );
                }
            }
        }
    }

    Ok(Json(SendEventResponse { event_id: updated_event.event_id }))
}

fn is_valid_event_type(event_type: &str) -> bool {
    // Validate event type format and allowed types
    match event_type {
        "m.room.message" | "m.room.encrypted" | "m.reaction" | "m.replace" | "m.room.redaction" => {
            true
        },
        _ if event_type.starts_with("m.") => {
            // Validate m.* event types more strictly
            event_type
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
        },
        _ => {
            // Allow custom event types that don't start with m.
            event_type
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '/')
        },
    }
}

fn can_send_event_type(
    user_id: &str,
    event_type: &str,
    power_levels: &matryx_surrealdb::repository::PowerLevels,
) -> bool {
    // Get user's power level
    let user_power = power_levels
        .users
        .get(user_id)
        .copied()
        .unwrap_or(power_levels.users_default);

    // Get required power level for this event type
    let required_power = power_levels
        .events
        .get(event_type)
        .copied()
        .unwrap_or(power_levels.events_default);

    user_power >= required_power
}
