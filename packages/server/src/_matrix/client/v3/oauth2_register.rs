use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};

use crate::auth::oauth2::OAuth2Service;
use crate::state::AppState;
use matryx_surrealdb::repository::oauth2::OAuth2Repository;

#[derive(Deserialize)]
pub struct ClientRegistrationRequest {
    pub client_name: String,
    pub redirect_uris: Vec<String>,
    pub client_type: String,
    pub application_type: Option<String>,
    pub contacts: Option<Vec<String>>,
    pub logo_uri: Option<String>,
    pub client_uri: Option<String>,
    pub policy_uri: Option<String>,
    pub tos_uri: Option<String>,
}

#[derive(Serialize)]
pub struct ClientRegistrationResponse {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub client_name: String,
    pub redirect_uris: Vec<String>,
    pub client_type: String,
    pub application_type: String,
    pub contacts: Option<Vec<String>>,
    pub logo_uri: Option<String>,
    pub client_uri: Option<String>,
    pub policy_uri: Option<String>,
    pub tos_uri: Option<String>,
    pub registration_access_token: String,
    pub registration_client_uri: String,
}

/// POST /_matrix/client/v3/oauth2/register
/// Register OAuth2 client application according to Matrix spec
pub async fn post(
    State(state): State<AppState>,
    Json(request): Json<ClientRegistrationRequest>,
) -> Result<Json<ClientRegistrationResponse>, StatusCode> {
    // Create OAuth2 service
    let oauth2_repo = OAuth2Repository::new(state.db.clone());
    let oauth2_service = OAuth2Service::new(
        oauth2_repo, 
        state.session_service.clone(), 
        state.homeserver_name.clone()
    );

    // Register the client
    let client = oauth2_service
        .register_client(
            &request.client_name,
            request.redirect_uris.clone(),
            &request.client_type,
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Generate registration access token for client management
    let registration_access_token = oauth2_service
        .generate_csrf_token(&client.client_id)
        .await;

    Ok(Json(ClientRegistrationResponse {
        client_id: client.client_id.clone(),
        client_secret: client.client_secret,
        client_name: client.client_name,
        redirect_uris: client.redirect_uris,
        client_type: request.client_type,
        application_type: request.application_type.unwrap_or_else(|| "web".to_string()),
        contacts: request.contacts,
        logo_uri: request.logo_uri,
        client_uri: request.client_uri,
        policy_uri: request.policy_uri,
        tos_uri: request.tos_uri,
        registration_access_token,
        registration_client_uri: format!("{}/_matrix/client/v3/oauth2/register/{}", 
            state.homeserver_name, client.client_id),
    }))
}