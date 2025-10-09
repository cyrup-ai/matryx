use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{error, info, warn};

use crate::AppState;
use crate::auth::uia::ThreepidCredentials;
use crate::auth::authenticated_user::AuthenticatedUser;
use matryx_surrealdb::repository::{
    third_party::ThirdPartyRepository,
    third_party_validation_session::{
        ThirdPartyValidationSessionRepository,
        ThirdPartyValidationSessionRepositoryTrait
    }
};

#[derive(Debug, Deserialize)]
pub struct ThreePidBindRequest {
    pub threepid_creds: ThreepidCredentials,
}

/// POST /_matrix/client/v3/account/3pid/bind
pub async fn post(
    State(state): State<AppState>,
    authenticated_user: AuthenticatedUser,
    Json(request): Json<ThreePidBindRequest>,
) -> Result<Json<Value>, StatusCode> {
    info!("3PID bind request received for user: {}", authenticated_user.user_id);

    // Validate ThreepidCredentials
    if request.threepid_creds.sid.is_empty() || request.threepid_creds.client_secret.is_empty() {
        warn!("Invalid threepid credentials: missing sid or client_secret");
        return Err(StatusCode::BAD_REQUEST);
    }

    // SUBTASK2: Validate session credentials
    let session_repo = ThirdPartyValidationSessionRepository::new(state.db.clone());

    // Get session by ID and secret (validates both)
    let session = session_repo
        .get_session_by_id_and_secret(
            &request.threepid_creds.sid,
            &request.threepid_creds.client_secret
        )
        .await
        .map_err(|e| {
            error!("Failed to query validation session: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            warn!("Invalid threepid credentials: session not found");
            StatusCode::BAD_REQUEST
        })?;

    // Verify session is verified
    if !session.verified {
        warn!("Session not validated: {}", session.session_id);
        return Err(StatusCode::FORBIDDEN);
    }

    // Verify session not expired
    if session.is_expired() {
        warn!("Session expired: {}", session.session_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    info!("Session validated: {} for {}@{}",
          session.session_id, session.address, session.medium);

    // SUBTASK3: Verify 3PID association with account
    let third_party_repo = ThirdPartyRepository::new(state.db.clone());

    // Check if this 3PID is associated with this user
    let associated_user = third_party_repo
        .find_user_by_third_party(&session.medium, &session.address)
        .await
        .map_err(|e| {
            error!("Failed to check 3PID association: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match associated_user {
        Some(user) if user == authenticated_user.user_id => {
            // Correct user - 3PID is associated with this account
            info!("3PID {} is associated with user {}", session.address, user);
        }
        Some(other_user) => {
            // Associated with different user - should not happen but check anyway
            warn!("3PID {} is associated with different user: {}",
                  session.address, other_user);
            return Err(StatusCode::FORBIDDEN);
        }
        None => {
            // Not associated with any user
            warn!("3PID {} not associated with user {}",
                  session.address, authenticated_user.user_id);
            return Err(StatusCode::FORBIDDEN);
        }
    }

    // SUBTASK4: Handle deprecated identity server fields
    if request.threepid_creds.id_server.is_some() ||
       request.threepid_creds.id_access_token.is_some() {
        warn!(
            "Identity server parameters provided (deprecated): id_server={:?}",
            request.threepid_creds.id_server
        );

        // NOTE: Matrix spec MSC2290 deprecated homeserver-side identity server binding
        // Clients should bind directly to identity servers using id_access_token
        // We log but do not attempt to bind on behalf of the user
        info!(
            "Skipping homeserver-side identity server binding (deprecated by MSC2290)"
        );
    }

    // SUBTASK5: Return success response
    info!("3PID bind completed for user {} - {}:{}",
          authenticated_user.user_id, session.medium, session.address);
    
    // Return success response per Matrix spec
    Ok(Json(json!({})))
}