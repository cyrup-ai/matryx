use axum::{extract::State, http::StatusCode, Json};
use serde_json::json;
use tracing::{debug, error, info, warn};
use matryx_entity::types::{
    ThirdPartyBindRequest, ThirdPartyInviteData, ThirdPartyInviteEventContent,
    ExchangeThirdPartyInviteRequest,
};
use crate::state::AppState;
use crate::federation::client::FederationClient;

/// PUT /_matrix/federation/v1/3pid/onbind
pub async fn put(
    State(state): State<AppState>,
    Json(payload): Json<ThirdPartyBindRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // 1. Validate mxid belongs to our server
    let user_domain = payload.mxid.split(':').nth(1).ok_or(StatusCode::BAD_REQUEST)?;
    if user_domain != state.homeserver_name {
        warn!("Rejecting onbind for user {} not on our server {}",
              payload.mxid, state.homeserver_name);
        return Err(StatusCode::BAD_REQUEST);
    }

    info!("Processing 3PID onbind for {} with {} invites",
          payload.mxid, payload.invites.len());

    // 2. Create federation client
    let federation_client = FederationClient::new(
        state.http_client.clone(),
        state.event_signer.clone(),
        state.homeserver_name.clone(),
        state.config.use_https,
    );

    // 3. Process each invite
    for invite in payload.invites {
        // Extract inviting server domain
        let sender_domain = match invite.sender.split(':').nth(1) {
            Some(domain) => domain,
            None => {
                warn!("Invalid sender format: {}", invite.sender);
                continue; // Skip but process others
            }
        };

        debug!("Processing invite from {} in room {}", invite.sender, invite.room_id);

        // Create ThirdPartyInviteData
        let third_party_invite_data = ThirdPartyInviteData {
            display_name: invite.address.clone(),
            signed: invite.signed.clone(),
        };

        // Create event content
        let content = ThirdPartyInviteEventContent {
            membership: "invite".to_string(),
            third_party_invite: third_party_invite_data,
        };

        // Create exchange request
        let exchange_request = ExchangeThirdPartyInviteRequest {
            content,
            room_id: invite.room_id.clone(),
            sender: invite.sender.clone(),
            state_key: payload.mxid.clone(),
            event_type: "m.room.member".to_string(),
        };

        // Call exchange endpoint on inviting server
        match federation_client.exchange_third_party_invite(
            sender_domain,
            &invite.room_id,
            &exchange_request
        ).await {
            Ok(_) => {
                info!("Successfully exchanged 3PID invite for {} in room {}",
                      payload.mxid, invite.room_id);
            },
            Err(e) => {
                error!("Failed to exchange 3PID invite for room {}: {:?}",
                       invite.room_id, e);
                // Continue processing other invites
            }
        }
    }

    Ok(Json(json!({})))
}
