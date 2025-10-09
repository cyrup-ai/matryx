# SPEC_FEDERATION_08: Implement 3PID onbind Endpoint - REQUIRES FULL IMPLEMENTATION

## Status
**STUB ONLY** - 0% implemented. Only infrastructure exists (route registered, types defined).

## Critical Issues

### 1. Handler Implementation Missing
**File**: `/packages/server/src/_matrix/federation/v1/threepid/onbind.rs`

**Current Code** (stub):
```rust
use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

pub async fn put(Json(_payload): Json<Value>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}
```

**Required Implementation**:
```rust
use axum::{extract::State, http::StatusCode, Json};
use serde_json::json;
use tracing::{debug, error, info, warn};
use matryx_entity::types::{
    ThirdPartyBindRequest, ThirdPartyInviteData, ThirdPartyInviteEventContent,
    ExchangeThirdPartyInviteRequest,
};
use crate::state::AppState;
use crate::federation::client::FederationClient;

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
        true, // use_https
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
```

### 2. FederationClient Method Missing
**File**: `/packages/server/src/federation/client.rs`

**Add after `query_user_devices` method**:

```rust
/// Exchange third-party invite with inviting server
///
/// Implements: PUT /_matrix/federation/v1/exchange_third_party_invite/{roomId}
/// Spec: /spec/server/11-room-invites.md:348-415
pub async fn exchange_third_party_invite(
    &self,
    destination: &str,
    room_id: &str,
    request: &ExchangeThirdPartyInviteRequest,
) -> Result<(), FederationClientError> {
    debug!(
        "Exchanging third-party invite with {} for room {}",
        destination, room_id
    );

    // Prevent federation requests to ourselves
    if destination == self.homeserver_name {
        return Err(FederationClientError::InvalidResponse);
    }

    // Construct federation API URL
    let protocol = if self.use_https { "https" } else { "http" };
    let url = format!(
        "{}://{}/_matrix/federation/v1/exchange_third_party_invite/{}",
        protocol,
        destination,
        urlencoding::encode(room_id)
    );

    // Serialize request to JSON
    let request_json = serde_json::to_value(request)
        .map_err(FederationClientError::JsonError)?;

    // Create HTTP PUT request
    let request_builder = self
        .http_client
        .put(&url)
        .json(request)
        .timeout(self.request_timeout);

    // Sign request with X-Matrix authentication
    let uri = format!(
        "/_matrix/federation/v1/exchange_third_party_invite/{}",
        urlencoding::encode(room_id)
    );
    let signed_request = self
        .event_signer
        .sign_federation_request(
            request_builder,
            "PUT",
            &uri,
            destination,
            Some(request_json),
        )
        .await
        .map_err(|_| FederationClientError::InvalidResponse)?;

    // Execute HTTP request
    let response = signed_request.send().await?;

    // Handle HTTP errors
    if !response.status().is_success() {
        warn!(
            "Exchange third-party invite failed: {} - {}",
            response.status(),
            response.status().canonical_reason().unwrap_or("Unknown error")
        );
        return Err(FederationClientError::ServerError {
            status_code: response.status().as_u16(),
            message: response
                .status()
                .canonical_reason()
                .unwrap_or("Unknown")
                .to_string(),
        });
    }

    info!(
        "Successfully exchanged third-party invite with {} for room {}",
        destination, room_id
    );

    Ok(())
}
```

**Also add import**:
```rust
use matryx_entity::types::{
    Transaction, TransactionResponse, ExchangeThirdPartyInviteRequest
};
```

## Requirements Checklist

### onbind.rs Handler
- [ ] Change signature to accept `State<AppState>`
- [ ] Change payload type from `Json<Value>` to `Json<ThirdPartyBindRequest>`
- [ ] Add all required imports (ThirdPartyBindRequest, ThirdPartyInviteData, ThirdPartyInviteEventContent, ExchangeThirdPartyInviteRequest)
- [ ] Validate mxid domain matches our homeserver
- [ ] Create FederationClient instance
- [ ] Loop through all invites in payload
- [ ] Extract sender domain from each invite.sender
- [ ] Construct ThirdPartyInviteData from invite.signed
- [ ] Construct ThirdPartyInviteEventContent with membership="invite"
- [ ] Construct ExchangeThirdPartyInviteRequest with all required fields
- [ ] Call federation_client.exchange_third_party_invite() for each invite
- [ ] Add proper logging (info/warn/error)
- [ ] Continue processing on individual failures
- [ ] Return empty JSON object on success

### FederationClient
- [ ] Add `exchange_third_party_invite()` method
- [ ] Method constructs correct URL path
- [ ] Method serializes request to JSON
- [ ] Method creates signed PUT request with X-Matrix auth
- [ ] Method handles HTTP errors properly
- [ ] Method returns Result<(), FederationClientError>
- [ ] Add ExchangeThirdPartyInviteRequest to imports

## Testing Requirements
After implementation:
1. Verify endpoint accepts ThirdPartyBindRequest
2. Verify rejection when mxid doesn't belong to our server
3. Verify federation client makes signed requests
4. Verify error handling for individual invite failures
5. Verify all invites are processed even if some fail

## Files to Modify
1. `/packages/server/src/_matrix/federation/v1/threepid/onbind.rs`
2. `/packages/server/src/federation/client.rs`

## Reference
- Spec: `/spec/server/11-room-invites.md:272-295`
- Similar pattern: `/packages/server/src/federation/client.rs` (send_transaction method)
