use axum::{
    extract::{Path, Query, State},
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

/// GET /_matrix/client/v3/login/sso/redirect/{idpId}
/// 
/// Redirects to a specific SSO provider's login page identified by idpId.
/// Returns 404 if the provider doesn't exist.
pub async fn get(
    State(state): State<AppState>,
    Path(idp_id): Path<String>,
    Query(params): Query<RedirectQuery>,
) -> Result<Redirect, MatrixError> {
    // Query database for SSO providers
    let auth_repo = Arc::new(AuthRepository::new(state.db.clone()));
    
    let providers = auth_repo.get_sso_providers().await
        .map_err(|e| {
            tracing::error!("Failed to query SSO providers: {}", e);
            MatrixError::Unknown
        })?;
    
    // Find the requested provider by ID
    let provider = providers.iter()
        .find(|p| p.id == idp_id)
        .ok_or_else(|| {
            tracing::warn!("SSO provider not found: {}", idp_id);
            MatrixError::NotFound
        })?;
    
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
