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

#[derive(Debug, Deserialize)]
pub struct ThreePidBindRequest {
    pub threepid_creds: ThreepidCredentials,
}

/// POST /_matrix/client/v3/account/3pid/bind
pub async fn post(
    State(state): State<AppState>,
    Json(request): Json<ThreePidBindRequest>,
) -> Result<Json<Value>, StatusCode> {
    info!("3PID bind request received");

    // Validate ThreepidCredentials
    if request.threepid_creds.sid.is_empty() || request.threepid_creds.client_secret.is_empty() {
        warn!("Invalid threepid credentials: missing sid or client_secret");
        return Err(StatusCode::BAD_REQUEST);
    }

    // TODO: Implement 3PID bind logic:
    // 1. Validate the threepid credentials (sid, client_secret)
    // 2. Verify the 3PID is already associated with the user's account
    // 3. Bind the 3PID to the identity server specified in id_server
    // 4. Use id_access_token if provided for identity server authentication
    
    info!("3PID bind completed for session: {}", request.threepid_creds.sid);
    
    // Return success response per Matrix spec
    Ok(Json(json!({})))
}