# SPEC_FEDERATION_08: Implement 3PID onbind Endpoint

## Status
STUB EXISTS - Endpoint exists as stub, needs full implementation

## Core Objective
Implement the PUT /_matrix/federation/v1/3pid/onbind endpoint to handle identity server callbacks when a third-party identifier (email/phone) gets bound to a Matrix ID. This endpoint processes pending room invites by forwarding them to the inviting servers for conversion into real Matrix invites.

## Architecture Context

### Existing Infrastructure

The endpoint stub **already exists** at:
- **File**: [`/packages/server/src/_matrix/federation/v1/threepid/onbind.rs`](../packages/server/src/_matrix/federation/v1/threepid/onbind.rs)
- **Route**: Already registered in [`main.rs:585`](../packages/server/src/main.rs#L585) as `.route("/v1/3pid/onbind", put(_matrix::federation::v1::threepid::onbind::put))`
- **Current Implementation**: Stub that accepts `Json<Value>` and returns empty object

### Entity Types (All Exist)

All required types are **already defined** in [`packages/entity/src/types/`](../packages/entity/src/types/):

- **`ThirdPartyBindRequest`** ([`third_party_bind_request.rs`](../packages/entity/src/types/third_party_bind_request.rs)): Request structure with `address`, `invites`, `medium`, `mxid`
- **`ThirdPartyInvite`** ([`third_party_invite.rs`](../packages/entity/src/types/third_party_invite.rs)): Individual invite with `address`, `medium`, `mxid`, `room_id`, `sender`, `signed`
- **`SignedThirdPartyInvite`** ([`signed_third_party_invite.rs`](../packages/entity/src/types/signed_third_party_invite.rs)): Signature block with `mxid`, `signatures`, `token`
- **`ExchangeThirdPartyInviteRequest`** ([`exchange_third_party_invite_request.rs`](../packages/entity/src/types/exchange_third_party_invite_request.rs)): Request for calling remote exchange endpoint
- **`ThirdPartyInviteEventContent`** ([`third_party_invite_event_content.rs`](../packages/entity/src/types/third_party_invite_event_content.rs)): Event content structure
- **`ThirdPartyInviteData`** ([`third_party_invite_data.rs`](../packages/entity/src/types/third_party_invite_data.rs)): Data for invite events

### Related Code to Study

Reference implementation patterns from:
- **Exchange endpoint**: [`/packages/server/src/_matrix/federation/v1/exchange_third_party_invite/by_room_id.rs`](../packages/server/src/_matrix/federation/v1/exchange_third_party_invite/by_room_id.rs) - Extensive validation and signature verification logic
- **Federation client**: [`/packages/server/src/federation/client.rs`](../packages/server/src/federation/client.rs) - Pattern for making signed federation requests
- **Spec reference**: [`/spec/server/11-room-invites.md:272-295`](../spec/server/11-room-invites.md#L272-L295) - Complete specification

## What the Endpoint Does

### Identity Server Callback Flow

```
1. User binds email alice@example.com to @alice:our-server.org
2. Identity server has pending invites for alice@example.com
3. Identity server calls PUT /_matrix/federation/v1/3pid/onbind on our-server.org
4. Our server receives: {
     "address": "alice@example.com",
     "medium": "email", 
     "mxid": "@alice:our-server.org",
     "invites": [
       {
         "room_id": "!room:inviting-server.org",
         "sender": "@bob:inviting-server.org",
         "signed": { identity server signature data }
       }
     ]
   }
5. For each invite, our server calls inviting-server.org:
   PUT /_matrix/federation/v1/exchange_third_party_invite/!room:inviting-server.org
6. Inviting server validates and creates real m.room.member invite event
7. Our server returns {} to identity server
```

### Key Differences from exchange_third_party_invite

- **onbind**: Called BY identity server TO invited user's homeserver (no auth required)
- **exchange_third_party_invite**: Called BY invited homeserver TO inviting server (requires X-Matrix auth)

## Implementation Requirements

### 1. Update Handler in `/packages/server/src/_matrix/federation/v1/threepid/onbind.rs`

**Current code:**
```rust
use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

pub async fn put(Json(_payload): Json<Value>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}
```

**Required changes:**

```rust
use axum::{extract::State, http::StatusCode, Json};
use serde_json::json;
use tracing::{debug, error, info, warn};
use matryx_entity::types::{
    ThirdPartyBindRequest, ThirdPartyInviteData, ThirdPartyInviteEventContent,
    ExchangeThirdPartyInviteRequest,
};
use crate::state::AppState;

/// PUT /_matrix/federation/v1/3pid/onbind
///
/// Called by identity servers to notify when a 3PID is bound to a Matrix ID.
/// This endpoint has NO AUTHENTICATION because it's a callback from identity server.
/// 
/// Spec: /spec/server/11-room-invites.md:272-295
pub async fn put(
    State(state): State<AppState>,
    Json(payload): Json<ThirdPartyBindRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Extract mxid domain to verify it belongs to our server
    // Validate mxid format and belongs to our server
    // For each invite in payload.invites:
    //   - Extract sender domain from invite.sender
    //   - Construct ThirdPartyInviteData from invite.signed
    //   - Create ExchangeThirdPartyInviteRequest 
    //   - Call FederationClient::exchange_third_party_invite(sender_domain, room_id, request)
    //   - Log success/failure but continue processing other invites
    // Return Ok(Json(json!({})))
}
```

**Step-by-step logic:**

1. **Validate mxid belongs to our server**:
   ```rust
   let user_domain = payload.mxid.split(':').nth(1).ok_or(StatusCode::BAD_REQUEST)?;
   if user_domain != state.homeserver_name {
       warn!("Rejecting onbind for user {} not on our server {}", 
             payload.mxid, state.homeserver_name);
       return Err(StatusCode::BAD_REQUEST);
   }
   ```

2. **Process each invite**:
   ```rust
   for invite in payload.invites {
       // Extract inviting server domain from sender
       let sender_domain = invite.sender.split(':').nth(1)
           .ok_or_else(|| {
               warn!("Invalid sender format: {}", invite.sender);
               StatusCode::BAD_REQUEST
           })?;
       
       // Create ThirdPartyInviteData from signed object
       let third_party_invite_data = ThirdPartyInviteData {
           display_name: invite.address.clone(), // Use address as display name
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
               info!("Successfully exchanged third-party invite for {} in room {}", 
                     payload.mxid, invite.room_id);
           },
           Err(e) => {
               // Log error but continue processing other invites
               error!("Failed to exchange third-party invite: {:?}", e);
           }
       }
   }
   ```

3. **Return success**: `Ok(Json(json!({})))`

### 2. Add Method to FederationClient

**File**: `/packages/server/src/federation/client.rs`

**Add new method** (after `query_user_devices`):

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

### 3. Update Imports

**In `/packages/server/src/_matrix/federation/v1/threepid/onbind.rs`:**

```rust
use axum::{extract::State, http::StatusCode, Json};
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::federation::client::FederationClient;
use crate::state::AppState;
use matryx_entity::types::{
    ExchangeThirdPartyInviteRequest, ThirdPartyBindRequest, ThirdPartyInviteData,
    ThirdPartyInviteEventContent,
};
```

**In `/packages/server/src/federation/client.rs`:**

```rust
use matryx_entity::types::{
    Transaction, TransactionResponse, ExchangeThirdPartyInviteRequest
};
```

## Specification Reference

From [`/spec/server/11-room-invites.md`](../spec/server/11-room-invites.md):

### Endpoint: PUT /_matrix/federation/v1/3pid/onbind

**Authentication**: NONE (identity server callback)

**Request Body** (`ThirdPartyBindRequest`):
- `address` (string): Third-party identifier (e.g., "alice@example.com")
- `medium` (string): Type of identifier ("email" or "msisdn")
- `mxid` (string): Matrix ID now bound to the 3PID
- `invites` (array of `ThirdPartyInvite`): Pending invites

**Each invite contains**:
- `address`, `medium`, `mxid`: Same as top level
- `room_id`: Room the invite is for
- `sender`: User who sent the original invite
- `signed`: `SignedThirdPartyInvite` with identity server signature

**Response**: `{}` (empty JSON object)

### Related Endpoint: PUT /_matrix/federation/v1/exchange_third_party_invite/{roomId}

This is what we CALL on the inviting server (already implemented in [`by_room_id.rs`](../packages/server/src/_matrix/federation/v1/exchange_third_party_invite/by_room_id.rs)).

## Security Considerations

### No Authentication Required
- This endpoint **deliberately has NO X-Matrix authentication**
- It's a callback from the identity server, not a federation request
- Security comes from validating:
  1. The mxid belongs to our server
  2. The identity server signatures in the `signed` object (validated by receiving server)

### Validation Steps
1. **Verify mxid domain**: `payload.mxid.split(':').nth(1) == state.homeserver_name`
2. **Extract sender domain**: For routing to correct inviting server
3. **No need to verify signatures**: The inviting server (exchange_third_party_invite) does this

### Error Handling
- **Continue on individual failures**: If one invite exchange fails, still process others
- **Log all failures**: For debugging and monitoring
- **Return success if mxid valid**: Even if all exchanges fail, return 200 to identity server

## Definition of Done

- [ ] Handler in `onbind.rs` accepts `ThirdPartyBindRequest` instead of `Value`
- [ ] Handler validates mxid belongs to our homeserver
- [ ] Handler processes each invite in the array
- [ ] For each invite, constructs proper `ExchangeThirdPartyInviteRequest`
- [ ] For each invite, calls inviting server's exchange endpoint via FederationClient
- [ ] New method `exchange_third_party_invite()` added to FederationClient
- [ ] Method creates signed PUT request to exchange endpoint
- [ ] Returns empty JSON object `{}` on success
- [ ] Logs info/warn/error messages at appropriate points

## Files to Modify

1. **`/packages/server/src/_matrix/federation/v1/threepid/onbind.rs`** - Update handler implementation
2. **`/packages/server/src/federation/client.rs`** - Add `exchange_third_party_invite()` method

## Files to Reference (DO NOT MODIFY)

- [`/packages/entity/src/types/third_party_bind_request.rs`](../packages/entity/src/types/third_party_bind_request.rs) - Request structure
- [`/packages/entity/src/types/third_party_invite.rs`](../packages/entity/src/types/third_party_invite.rs) - Invite structure  
- [`/packages/entity/src/types/exchange_third_party_invite_request.rs`](../packages/entity/src/types/exchange_third_party_invite_request.rs) - Exchange request
- [`/packages/server/src/_matrix/federation/v1/exchange_third_party_invite/by_room_id.rs`](../packages/server/src/_matrix/federation/v1/exchange_third_party_invite/by_room_id.rs) - Receiving end implementation
- [`/spec/server/11-room-invites.md`](../spec/server/11-room-invites.md) - Complete specification

## Implementation Pattern

This implementation follows the **Federation Client Pattern**:

1. Parse typed request body (not generic `Value`)
2. Validate request against our server's identity
3. For each item requiring federation call, extract destination domain
4. Use FederationClient method to make signed outbound request
5. Log results but don't fail on individual errors
6. Return simple success response

See [`send_transaction`](../packages/server/src/federation/client.rs#L170-L223) in FederationClient for similar pattern.

## Priority
MEDIUM - Required for email/phone invites to work correctly