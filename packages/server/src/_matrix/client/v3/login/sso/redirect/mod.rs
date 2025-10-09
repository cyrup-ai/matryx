use axum::{
    extract::{Query, State},
    response::Redirect,
};
use serde::Deserialize;
use std::sync::Arc;
use url::Url;
use crate::error::matrix_errors::MatrixError;
use crate::state::AppState;
use matryx_surrealdb::repository::AuthRepository;

#[derive(Deserialize)]
pub struct RedirectQuery {
    #[serde(rename = "redirectUrl")]
    redirect_url: Option<String>,
}

/// Validates that a redirect URL is safe to use
/// 
/// Accepts:
/// - Relative URLs starting with `/`
/// - Absolute URLs pointing to the homeserver domain
/// 
/// Rejects all other URLs to prevent open redirect vulnerabilities
fn validate_redirect_url(redirect_url: &str, homeserver_domain: &str) -> Result<(), MatrixError> {
    // Allow relative URLs (most common case for Matrix clients)
    if redirect_url.starts_with('/') {
        return Ok(());
    }
    
    // Parse and validate absolute URLs
    if let Ok(parsed) = Url::parse(redirect_url) {
        if let Some(host) = parsed.host_str() {
            // Allow exact domain match or subdomain
            if host == homeserver_domain || host.ends_with(&format!(".{}", homeserver_domain)) {
                return Ok(());
            }
        }
    }
    
    // Reject potentially malicious redirects
    tracing::warn!(
        "Rejected SSO redirectUrl '{}' - must be relative or match homeserver domain '{}'",
        redirect_url,
        homeserver_domain
    );
    Err(MatrixError::InvalidParam)
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
    let mut sso_url = Url::parse(&provider.redirect_url)
        .map_err(|e| {
            tracing::error!("Invalid SSO provider redirect_url for '{}': {}", provider.id, e);
            MatrixError::Unknown
        })?;

    if let Some(client_redirect) = params.redirect_url {
        validate_redirect_url(&client_redirect, &state.homeserver_name)?;
        sso_url.query_pairs_mut()
            .append_pair("redirectUrl", &client_redirect);
    }

    tracing::info!("Redirecting to SSO provider: {}", provider.id);

    // Return HTTP 302 redirect
    Ok(Redirect::temporary(sso_url.as_str()))
}

pub mod by_idp_id;
