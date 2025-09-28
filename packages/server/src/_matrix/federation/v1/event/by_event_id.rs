use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::state::AppState;
use crate::federation::pdu_validator::{PduValidator, ValidationResult};
use crate::auth::x_matrix_parser::parse_x_matrix_header;
use matryx_entity::types::{PDU, Transaction};
use matryx_surrealdb::repository::{EventRepository, MembershipRepository, RoomRepository};



/// Validate Matrix event ID format
fn validate_event_id(event_id: &str) -> Result<(), StatusCode> {
    if !event_id.starts_with('$') || !event_id.contains(':') {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

/// GET /_matrix/federation/v1/event/{eventId}
///
/// Retrieves a single event. Returns a transaction containing a single PDU
/// which is the event requested.
pub async fn get(
    State(state): State<AppState>,
    Path(event_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Transaction>, StatusCode> {
    // Parse X-Matrix authentication header using RFC 9110 compliant parser
    let auth_header = headers
        .get("authorization")
        .ok_or(StatusCode::UNAUTHORIZED)?
        .to_str()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let x_matrix_auth = parse_x_matrix_header(auth_header)
        .map_err(|e| {
            warn!("Failed to parse X-Matrix authentication header: {}", e);
            StatusCode::BAD_REQUEST
        })?;

    debug!("Event retrieval request - origin: {}, event: {}", x_matrix_auth.origin, event_id);

    // Validate server signature
    let _server_validation = state
        .session_service
        .validate_server_signature(
            &x_matrix_auth.origin,
            &x_matrix_auth.key_id,
            &x_matrix_auth.signature,
            "GET",
            &format!("/_matrix/federation/v1/event/{}", event_id),
            &[],
        )
        .await
        .map_err(|e| {
            warn!("Server signature validation failed: {:?}", e);
            StatusCode::UNAUTHORIZED
        })?;

    // Validate event ID format
    validate_event_id(&event_id).map_err(|_| {
        warn!("Invalid event ID format: {}", event_id);
        StatusCode::BAD_REQUEST
    })?;

    // Retrieve event from database
    let event_repo = Arc::new(EventRepository::new(state.db.clone()));
    let event = event_repo
        .get_by_id(&event_id)
        .await
        .map_err(|e| {
            error!("Failed to query event {}: {}", event_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            warn!("Event {} not found", event_id);
            StatusCode::NOT_FOUND
        })?;

    // Validate event according to Matrix specification before serving to federation
    let pdu_validator = PduValidator::from_app_state(&state).map_err(|e| {
        error!("Failed to create PDU validator: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let event_json = serde_json::to_value(&event).map_err(|e| {
        error!("Failed to serialize event for validation: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let validated_event = match pdu_validator
        .validate_pdu(&event_json, &x_matrix_auth.origin)
        .await
    {
        Ok(ValidationResult::Valid(validated_event)) => {
            debug!("Event {} passed Matrix validation", event_id);
            validated_event
        },
        Ok(ValidationResult::Rejected { event_id, reason }) => {
            warn!("Event {} failed validation and was rejected: {}", event_id, reason);
            return Err(StatusCode::FORBIDDEN);
        },
        Ok(ValidationResult::SoftFailed { event, reason }) => {
            warn!("Event {} soft-failed validation: {}", event_id, reason);
            // For federation requests, we still serve soft-failed events
            // but log the issue for monitoring
            event
        },
        Err(e) => {
            error!("Event validation error for {}: {}", event_id, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Check if requesting server has permission to access this event
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
    let has_users = membership_repo.server_has_users_in_room(&validated_event.room_id, &x_matrix_auth.origin)
        .await
        .map_err(|e| {
            error!("Failed to check server membership: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let has_permission = if has_users {
        true
    } else {
        // Check if room is world-readable
        let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
        room_repo.is_room_world_readable(&validated_event.room_id)
            .await
            .map_err(|e| {
                error!("Failed to check room world-readable status: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
    };

    if !has_permission {
        warn!(
            "Server {} not authorized to access event {} in room {}",
            x_matrix_auth.origin, event_id, validated_event.room_id
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Convert validated Event to PDU format for federation response
    let pdu = PDU {
        event_id: validated_event.event_id.clone(),
        room_id: validated_event.room_id.clone(),
        sender: validated_event.sender.clone(),
        origin_server_ts: validated_event.origin_server_ts,
        event_type: validated_event.event_type.clone(),
        content: validated_event.content.clone(),
        state_key: validated_event.state_key.clone(),
        prev_events: validated_event.prev_events.clone().unwrap_or_default(),
        auth_events: validated_event.auth_events.clone().unwrap_or_default(),
        depth: validated_event.depth.unwrap_or(0),
        signatures: validated_event.signatures.clone().unwrap_or_default(),
        hashes: validated_event.hashes.clone().unwrap_or_default(),
        unsigned: validated_event.unsigned.clone().and_then(|v| serde_json::from_value(v).ok()),
    };

    // Create transaction response
    let transaction = Transaction {
        origin: state.homeserver_name.clone(),
        origin_server_ts: Utc::now().timestamp_millis(),
        pdus: vec![pdu],
        edus: vec![],
    };

    info!("Retrieved event {} for server {}", event_id, x_matrix_auth.origin);

    Ok(Json(transaction))
}


