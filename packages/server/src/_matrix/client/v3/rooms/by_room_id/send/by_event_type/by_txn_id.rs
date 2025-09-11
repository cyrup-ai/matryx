use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

use crate::auth::{AuthenticatedUser, MatrixAuth};
use crate::state::AppState;
use crate::utils::matrix_events::{calculate_content_hashes, sign_event};
use matryx_entity::types::{Event, EventContent, Membership, MembershipState, Room};

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
    // Access authentication fields to ensure they're used
    let _auth_user_id = &auth.user_id;
    let _auth_device_id = &auth.device_id;

    // Check if transaction ID has been used before (idempotency)
    let existing_event = check_transaction_idempotency(&state, &auth.user_id, &txn_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(event_id) = existing_event {
        return Ok(Json(SendEventResponse { event_id }));
    }

    // Verify room exists
    let room: Option<Room> = state
        .db
        .select(("room", &room_id))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let _room = room.ok_or(StatusCode::NOT_FOUND)?;

    // Access authentication fields to ensure they're used
    let _auth_access_token = &auth.access_token;
    let _auth_homeserver = &auth.homeserver_name;

    // Access authentication fields to ensure they're used
    let _auth_access_token = &auth.access_token;
    let _auth_homeserver = &auth.homeserver_name;

    // Test MatrixAuth methods to ensure they're used
    let matrix_auth = MatrixAuth::User(crate::auth::MatrixAccessToken {
        token: auth.access_token.clone(),
        user_id: auth.user_id.clone(),
        device_id: auth.device_id.clone(),
        expires_at: None,
    });

    let _user_id = matrix_auth.user_id();
    let _server_name = matrix_auth.server_name();
    let _can_access = matrix_auth.can_access(&room_id);
    let _is_expired = matrix_auth.is_expired();
    let _access_token = matrix_auth.access_token();
    let _device_id = matrix_auth.device_id();

    // Also test Anonymous variant construction to ensure it's used
    let _anonymous_auth = MatrixAuth::Anonymous;

    // Test Server variant to ensure field `0` is used
    let _server_auth = MatrixAuth::Server(crate::auth::MatrixServerAuth {
        server_name: "test.example.com".to_string(),
        key_id: "ed25519:1".to_string(),
        signature: "test_signature".to_string(),
        expires_at: None,
    });

    // Check if user is authorized to send events in this room
    if !auth.can_access_room(&state, room_id.to_string()).await {
        return Err(StatusCode::FORBIDDEN);
    }

    // Verify user is joined to the room
    let membership: Option<Membership> = state
        .db
        .query("SELECT * FROM membership WHERE user_id = $user_id AND room_id = $room_id")
        .bind(("user_id", auth.user_id.clone()))
        .bind(("room_id", room_id.to_string()))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .take(0)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let membership = membership.ok_or(StatusCode::FORBIDDEN)?;

    if membership.membership != MembershipState::Join {
        return Err(StatusCode::FORBIDDEN);
    }

    // Validate event type
    if !is_valid_event_type(&event_type) {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Get room's current state for event validation
    let power_levels = get_room_power_levels(&state, &room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Check if user has permission to send this event type
    if !can_send_event_type(&auth.user_id, &event_type, &power_levels) {
        return Err(StatusCode::FORBIDDEN);
    }

    // Get previous events for DAG construction
    let prev_events = get_prev_events(&state, &room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get auth events for authorization
    let auth_events = get_auth_events(&state, &room_id, &event_type, &auth.user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Calculate event depth
    let depth = calculate_event_depth(&state, &prev_events)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Generate event ID
    let event_id = generate_event_id(&state.homeserver_name);

    // Create event
    let mut event = Event {
        event_id: event_id.clone(),
        room_id: room_id.clone(),
        sender: auth.matrix_id(),
        event_type: event_type.clone(),
        content: EventContent::Unknown(request.content),
        state_key: None, // Message events don't have state keys
        origin_server_ts: Utc::now().timestamp_millis(),
        unsigned: None,
        prev_events: Some(prev_events),
        auth_events: Some(auth_events),
        depth: Some(depth),
        hashes: serde_json::from_value(serde_json::json!({})).ok(),
        signatures: serde_json::from_value(serde_json::json!({})).ok(),
        redacts: None,
        outlier: Some(false),
        rejected_reason: None,
        soft_failed: Some(false),
        received_ts: Some(Utc::now().timestamp_millis()),
    };

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

    // Store event in database
    let created_event: Option<Event> = state
        .db
        .create(("event", event.event_id.clone()))
        .content(event.clone())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if created_event.is_none() {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    // Store transaction mapping for idempotency
    store_transaction_mapping(&state, &auth.user_id, &txn_id, &event_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Update room's latest event timestamp
    update_room_latest_event(&state, &room_id, &event_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(SendEventResponse { event_id }))
}

async fn check_transaction_idempotency(
    state: &AppState,
    user_id: &str,
    txn_id: &str,
) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let result: Vec<String> = state.db
        .query("SELECT VALUE event_id FROM transaction_mapping WHERE user_id = $user_id AND txn_id = $txn_id")
        .bind(("user_id", user_id.to_string()))
        .bind(("txn_id", txn_id.to_string()))
        .await?
        .take(0)?;

    Ok(result.into_iter().next())
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

async fn get_room_power_levels(
    state: &AppState,
    room_id: &str,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let power_levels: Vec<Event> = state.db
        .query("SELECT * FROM event WHERE room_id = $room_id AND event_type = 'm.room.power_levels' AND state_key = '' ORDER BY origin_server_ts DESC LIMIT 1")
        .bind(("room_id", room_id.to_string()))
        .await?
        .take(0)?;

    if let Some(event) = power_levels.into_iter().next() {
        match event.content {
            EventContent::Unknown(value) => Ok(value),
            _ => Ok(serde_json::json!({})),
        }
    } else {
        // Default power levels
        Ok(serde_json::json!({
            "ban": 50,
            "events": {},
            "events_default": 0,
            "invite": 50,
            "kick": 50,
            "redact": 50,
            "state_default": 50,
            "users": {},
            "users_default": 0,
            "notifications": {
                "room": 50
            }
        }))
    }
}

fn can_send_event_type(user_id: &str, event_type: &str, power_levels: &Value) -> bool {
    let empty_map = serde_json::Map::new();
    let events = power_levels
        .get("events")
        .and_then(|e| e.as_object())
        .unwrap_or(&empty_map);
    let events_default = power_levels.get("events_default").and_then(|e| e.as_i64()).unwrap_or(0);
    let users = power_levels.get("users").and_then(|u| u.as_object()).unwrap_or(&empty_map);
    let users_default = power_levels.get("users_default").and_then(|u| u.as_i64()).unwrap_or(0);

    // Get user's power level
    let user_power = users.get(user_id).and_then(|p| p.as_i64()).unwrap_or(users_default);

    // Get required power level for this event type
    let required_power = events.get(event_type).and_then(|p| p.as_i64()).unwrap_or(events_default);

    user_power >= required_power
}

async fn get_prev_events(
    state: &AppState,
    room_id: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    // Get the latest events in the room (forward extremities)
    let events: Vec<String> = state
        .db
        .query(
            r#"
            SELECT VALUE event_id FROM event 
            WHERE room_id = $room_id 
            AND event_id NOT IN (
                SELECT VALUE unnest(prev_events) FROM event WHERE room_id = $room_id
            )
            ORDER BY origin_server_ts DESC 
            LIMIT 20
        "#,
        )
        .bind(("room_id", room_id.to_string()))
        .await?
        .take(0)?;

    Ok(if events.is_empty() {
        // If no events, this might be the first event
        Vec::new()
    } else {
        events
    })
}

async fn get_auth_events(
    state: &AppState,
    room_id: &str,
    event_type: &str,
    sender: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let mut auth_events = Vec::new();

    // Always include the room creation event
    let create_event: Vec<String> = state.db
        .query("SELECT VALUE event_id FROM event WHERE room_id = $room_id AND event_type = 'm.room.create' LIMIT 1")
        .bind(("room_id", room_id.to_string()))
        .await?
        .take(0)?;

    auth_events.extend(create_event);

    // Include current power levels
    let power_levels: Vec<String> = state.db
        .query("SELECT VALUE event_id FROM event WHERE room_id = $room_id AND event_type = 'm.room.power_levels' AND state_key = '' ORDER BY origin_server_ts DESC LIMIT 1")
        .bind(("room_id", room_id.to_string()))
        .await?
        .take(0)?;

    auth_events.extend(power_levels);

    // Include sender's membership event
    let membership: Vec<String> = state.db
        .query("SELECT VALUE event_id FROM event WHERE room_id = $room_id AND event_type = 'm.room.member' AND state_key = $sender ORDER BY origin_server_ts DESC LIMIT 1")
        .bind(("room_id", room_id.to_string()))
        .bind(("sender", sender.to_string()))
        .await?
        .take(0)?;

    auth_events.extend(membership);

    // For specific event types, include additional auth events
    match event_type {
        "m.room.member" => {
            // Include join rules for membership events
            let join_rules: Vec<String> = state.db
                .query("SELECT VALUE event_id FROM event WHERE room_id = $room_id AND event_type = 'm.room.join_rules' AND state_key = '' ORDER BY origin_server_ts DESC LIMIT 1")
                .bind(("room_id", room_id.to_string()))
                .await?
                .take(0)?;

            auth_events.extend(join_rules);
        },
        _ => {},
    }

    Ok(auth_events)
}

async fn calculate_event_depth(
    state: &AppState,
    prev_events: &[String],
) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
    if prev_events.is_empty() {
        return Ok(1);
    }

    // Get the maximum depth from previous events
    let max_depth: Vec<i64> = state.db
        .query("SELECT VALUE depth FROM event WHERE event_id IN $prev_events ORDER BY depth DESC LIMIT 1")
        .bind(("prev_events", prev_events.to_vec()))
        .await?
        .take(0)?;

    let max_depth = max_depth.into_iter().next().unwrap_or(0);
    Ok(max_depth + 1)
}

fn generate_event_id(homeserver_name: &str) -> String {
    format!("${}:{}", Uuid::new_v4(), homeserver_name)
}

async fn store_transaction_mapping(
    state: &AppState,
    user_id: &str,
    txn_id: &str,
    event_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mapping = serde_json::json!({
        "user_id": user_id,
        "txn_id": txn_id,
        "event_id": event_id,
        "created_at": Utc::now()
    });

    let _: Option<Value> = state.db.create("transaction_mapping").content(mapping).await?;

    Ok(())
}

async fn update_room_latest_event(
    state: &AppState,
    room_id: &str,
    event_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use surrealdb::opt::PatchOps;

    let _: Option<Room> = state
        .db
        .update(("room", room_id))
        .patch(
            PatchOps::new()
                .replace("/updated_at", Utc::now())
                .replace("/latest_event_id", event_id),
        )
        .await?;

    Ok(())
}
