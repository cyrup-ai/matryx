use axum::{
    extract::{Query, State},
    response::Redirect,
};
use serde::Deserialize;
use std::sync::Arc;
use crate::error::matrix_errors::MatrixError;
use crate::state::AppState;
use matryx_surrealdb::repository::AuthRepository;

#[derive(Deserialize)]
pub struct RedirectQuery {
    #[serde(rename = "redirectUrl")]
    redirect_url: Option<String>,
}

/// GET /_matrix/client/v3/login/sso/redirect
/// 
/// Redirects to the configured SSO provider's login page.
/// The client provides a redirectUrl parameter indicating where to send
/// the user after successful SSO authentication.
pub async fn get(
    State(state): State<AppState>,
    Query(params): Query<RedirectQuery>,
) -> Result<Redirect, MatrixError> {
    // Query database for SSO providers
    let auth_repo = Arc::new(AuthRepository::new(state.db.clone()));
    
    let providers = auth_repo.get_sso_providers().await
        .map_err(|e| {
            tracing::error!("Failed to query SSO providers: {}", e);
            MatrixError::Unknown
        })?;
    
    // Return 404 if no SSO providers are configured
    if providers.is_empty() {
        tracing::warn!("SSO redirect requested but no providers configured");
        return Err(MatrixError::NotFound);
    }
    
    // Use the first provider as default
    let provider = &providers[0];
    
    // Build redirect URL with client's callback
    let mut sso_url = provider.redirect_url.clone();
    
    // Add redirectUrl as query parameter for SSO provider to use
    if let Some(client_redirect) = params.redirect_url {
        sso_url.push_str("?redirectUrl=");
        sso_url.push_str(&urlencoding::encode(&client_redirect));
    }
    
    tracing::info!("Redirecting to SSO provider: {}", provider.id);
    
    // Return HTTP 302 redirect
    Ok(Redirect::temporary(&sso_url))
}

pub mod by_idp_id;
