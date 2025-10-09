use std::net::SocketAddr;
use std::time::Duration;

use axum::{
    Json,
    extract::{ConnectInfo, Path, State},
    http::{HeaderMap, StatusCode},
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{error, info, warn};

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
    utils::canonical_json::to_canonical_json,
};
use matryx_entity::types::ThirdPartySigned;
use matryx_surrealdb::repository::{
    EventRepository, MembershipRepository, RoomRepository, UserRepository, error::RepositoryError,
    room_join::RoomJoinService,
};

#[derive(Deserialize)]
pub struct JoinRequest {
    /// Optional reason for joining
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,

    /// Optional third-party signed token for invite validation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub third_party_signed: Option<Value>,
}

#[derive(Serialize)]
pub struct JoinResponse {
    pub room_id: String,
}

/// Matrix Client-Server API v1.11 Section 10.2.1
///
/// POST /_matrix/client/v3/join/{roomIdOrAlias}
///
/// Join a room by room ID or room alias. This endpoint allows authenticated
/// users to join public rooms or rooms they have been invited to.
///
/// For public rooms, the user can join directly. For invite-only rooms,
/// the user must have a pending invitation.
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(room_id_or_alias): Path<String>,
    Json(request): Json<JoinRequest>,
) -> Result<Json<JoinResponse>, StatusCode> {
    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("Room join failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Room join failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Room join failed - server authentication not allowed for room joins");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Room join failed - anonymous authentication not allowed for room joins");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    // Handle optional join reason for audit logging
    if let Some(reason) = &request.reason {
        info!(
            "Room join with reason - User: {}, Room: {}, Reason: '{}', From: {}",
            user_id, room_id_or_alias, reason, addr
        );
    } else {
        info!(
            "Processing room join request for user: {} to room: {} from: {}",
            user_id, room_id_or_alias, addr
        );
    }

    // Handle third-party signed invitations if provided
    if let Some(third_party_signed_value) = &request.third_party_signed {
        info!("Third-party signed invitation detected for user: {}", user_id);
        
        // Deserialize into ThirdPartySigned struct
        let third_party_signed: ThirdPartySigned = serde_json::from_value(third_party_signed_value.clone())
            .map_err(|e| {
                warn!("Invalid third_party_signed format: {}", e);
                StatusCode::BAD_REQUEST
            })?;
        
        // Validate the signature
        validate_third_party_invitation(&state, &third_party_signed, &user_id).await?;
        
        info!("Third-party invitation signature validated for user: {}", user_id);
    }

    // Create repository instances
    let room_repo = RoomRepository::new(state.db.clone());
    let membership_repo = MembershipRepository::new(state.db.clone());
    let event_repo = EventRepository::new(state.db.clone());
    let user_repo = UserRepository::new(state.db.clone());

    // Create room join service
    let join_service = RoomJoinService::new(room_repo, membership_repo, event_repo, user_repo);

    // Use the join service to handle the room join
    match join_service.join_room(&room_id_or_alias, &user_id).await {
        Ok(result) => {
            info!(
                "Successfully joined user {} to room {} with event {}",
                user_id, result.room_id, result.event_id
            );
            Ok(Json(JoinResponse { room_id: result.room_id }))
        },
        Err(e) => match e {
            RepositoryError::NotFound { .. } => {
                warn!("Room join failed - room not found: {}", room_id_or_alias);
                Err(StatusCode::NOT_FOUND)
            },
            RepositoryError::Unauthorized { .. } => {
                warn!(
                    "Room join failed - user {} not authorized to join room {}",
                    user_id, room_id_or_alias
                );
                Err(StatusCode::FORBIDDEN)
            },
            RepositoryError::Validation { .. } => {
                warn!("Room join failed - invalid room identifier format: {}", room_id_or_alias);
                Err(StatusCode::BAD_REQUEST)
            },
            _ => {
                error!("Room join failed - internal error: {}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            },
        },
    }
}

/// Validate third-party signed invitation from identity server
///
/// Verifies cryptographic signatures on third-party invitations to prevent
/// authentication bypass vulnerabilities. Implements Matrix specification
/// requirements for identity server signature validation.
async fn validate_third_party_invitation(
    state: &AppState,
    third_party_signed: &ThirdPartySigned,
    expected_mxid: &str,
) -> Result<(), StatusCode> {
    // 1. Validate mxid matches the user joining
    if third_party_signed.mxid != expected_mxid {
        warn!(
            "Third-party invitation mxid mismatch: expected {}, got {}",
            expected_mxid, third_party_signed.mxid
        );
        return Err(StatusCode::FORBIDDEN);
    }
    
    // 2. Extract identity server name from signatures
    let identity_server = third_party_signed.signatures.keys().next()
        .ok_or_else(|| {
            warn!("Third-party invitation has no signatures");
            StatusCode::FORBIDDEN
        })?;
    
    // 3. Check if identity server is trusted
    if !is_trusted_identity_server(state, identity_server).await {
        warn!("Untrusted identity server: {}", identity_server);
        return Err(StatusCode::FORBIDDEN); // M_SERVER_NOT_TRUSTED
    }
    
    // 4. Create canonical JSON of signed data (without signatures field)
    let signed_data = json!({
        "mxid": third_party_signed.mxid,
        "sender": third_party_signed.sender,
        "token": third_party_signed.token
    });
    
    let canonical_json = to_canonical_json(&signed_data)
        .map_err(|e| {
            error!("Failed to create canonical JSON: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    // 5. Verify at least one signature from the identity server
    let server_signatures = third_party_signed.signatures.get(identity_server)
        .ok_or_else(|| {
            warn!("No signatures from identity server {}", identity_server);
            StatusCode::FORBIDDEN
        })?;
    
    let mut verified = false;
    for (key_id, signature) in server_signatures {
        // 6. Fetch identity server public key
        match fetch_identity_server_key(state, identity_server, key_id).await {
            Ok(public_key) => {
                // 7. Verify signature using session_service
                match state.session_service.verify_ed25519_signature(
                    signature,
                    &canonical_json,
                    &public_key
                ) {
                    Ok(_) => {
                        info!(
                            "Verified third-party invitation signature from {}:{}",
                            identity_server, key_id
                        );
                        verified = true;
                        break;
                    }
                    Err(e) => {
                        warn!(
                            "Failed to verify signature {}:{}: {:?}",
                            identity_server, key_id, e
                        );
                    }
                }
            }
            Err(e) => {
                warn!(
                    "Failed to fetch identity server key {}:{}: {:?}",
                    identity_server, key_id, e
                );
            }
        }
    }
    
    if !verified {
        warn!("Failed to verify any third-party invitation signature");
        return Err(StatusCode::FORBIDDEN);
    }
    
    Ok(())
}

/// Fetch public key from identity server for signature verification
///
/// Implements identity server key fetching with caching to prevent DoS attacks.
/// Uses the Matrix Identity Service API endpoint for key retrieval.
async fn fetch_identity_server_key(
    state: &AppState,
    identity_server: &str,
    key_id: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Check cache first (reuse KeyServerRepository)
    if let Some(cached_key) = state.session_service
        .get_cached_server_public_key(identity_server, key_id)
        .await?
    {
        return Ok(cached_key);
    }
    
    // Fetch from identity server
    // Matrix Identity Service API: GET /_matrix/identity/v2/pubkey/{keyId}
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;
    
    let key_url = format!(
        "https://{}/_matrix/identity/v2/pubkey/{}",
        identity_server,
        key_id
    );
    
    let response = client.get(&key_url).send().await?;
    
    if !response.status().is_success() {
        return Err(format!(
            "Identity server key fetch failed: HTTP {}",
            response.status()
        ).into());
    }
    
    // Response format: {"public_key": "base64_key"}
    let key_response: serde_json::Value = response.json().await?;
    let public_key = key_response
        .get("public_key")
        .and_then(|v| v.as_str())
        .ok_or("Missing public_key in response")?
        .to_string();
    
    // Cache the key (24 hour expiration)
    let now = chrono::Utc::now();
    let expires_at = now + chrono::Duration::hours(24);
    state.session_service
        .cache_server_public_key(
            identity_server,
            key_id,
            &public_key,
            now,
            expires_at
        )
        .await?;
    
    Ok(public_key)
}

/// Check if an identity server is trusted for third-party invitations
///
/// Implements security policy for identity server trust validation.
/// Prevents arbitrary identity servers from forging invitations.
async fn is_trusted_identity_server(
    state: &AppState,
    identity_server: &str,
) -> bool {
    // Option 1: Hardcoded whitelist of known identity servers
    const TRUSTED_SERVERS: &[&str] = &[
        "matrix.org",
        "vector.im",
    ];
    
    if TRUSTED_SERVERS.contains(&identity_server) {
        return true;
    }
    
    // Option 2: Same domain as homeserver (trust our own identity server)
    if identity_server == state.homeserver_name {
        return true;
    }
    
    // Default: untrusted
    false
}
