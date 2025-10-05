use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};

use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::sync::Arc;
use surrealdb::Connection;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::auth::{MatrixAuthError, MatrixSessionService};
use matryx_surrealdb::repository::oauth2::{OAuth2Client, OAuth2Repository};

/// OAuth 2.0 authorization request parameters
#[derive(Debug, Deserialize)]
pub struct AuthorizationRequest {
    pub response_type: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
}

/// OAuth 2.0 token exchange request
#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    pub grant_type: String,
    pub code: String,
    pub redirect_uri: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub code_verifier: Option<String>,
}

/// OAuth 2.0 token response
#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}

/// OAuth 2.0 error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub error_description: Option<String>,
    pub error_uri: Option<String>,
    pub state: Option<String>,
}

/// OAuth 2.0 service for Matrix authentication
pub struct OAuth2Service<C: Connection> {
    oauth2_repo: OAuth2Repository<C>,
    session_service: Arc<MatrixSessionService<C>>,
    homeserver_name: String,
    // Add CSRF token storage
    csrf_tokens: Arc<RwLock<HashMap<String, (String, i64)>>>, // token -> (user_id, expires_at)
}

impl<C: Connection> OAuth2Service<C> {
    pub fn new(
        oauth2_repo: OAuth2Repository<C>,
        session_service: Arc<MatrixSessionService<C>>,
        homeserver_name: String,
    ) -> Self {
        Self {
            oauth2_repo,
            session_service,
            homeserver_name,
            csrf_tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    // Generate CSRF token for authenticated user
    pub async fn generate_csrf_token(&self, user_id: &str) -> String {
        let csrf_token = Uuid::new_v4().to_string();
        let expires_at = chrono::Utc::now().timestamp() + 3600; // 1 hour expiry

        let mut tokens = self.csrf_tokens.write().await;
        tokens.insert(csrf_token.clone(), (user_id.to_string(), expires_at));

        // Cleanup expired tokens
        tokens.retain(|_, (_, exp)| *exp > chrono::Utc::now().timestamp());

        csrf_token
    }

    // Validate CSRF token in authorize method
    pub async fn validate_csrf_token(&self, state: &str, user_id: &str) -> bool {
        let mut tokens = self.csrf_tokens.write().await;

        if let Some((stored_user_id, expires_at)) = tokens.remove(state) {
            stored_user_id == user_id && expires_at > chrono::Utc::now().timestamp()
        } else {
            false
        }
    }

    /// Handle OAuth 2.0 authorization request
    pub async fn authorize(
        &self,
        params: AuthorizationRequest,
        authenticated_user_id: Option<String>,
    ) -> Result<Redirect, ErrorResponse> {
        // Validate response_type
        if params.response_type != "code" {
            return Err(ErrorResponse {
                error: "unsupported_response_type".to_string(),
                error_description: Some("Only 'code' response type is supported".to_string()),
                error_uri: None,
                state: params.state,
            });
        }

        // Validate client_id
        let client = self
            .oauth2_repo
            .get_client(&params.client_id)
            .await
            .map_err(|_| ErrorResponse {
                error: "invalid_client".to_string(),
                error_description: Some("Client not found or inactive".to_string()),
                error_uri: None,
                state: params.state.clone(),
            })?
            .ok_or_else(|| ErrorResponse {
                error: "invalid_client".to_string(),
                error_description: Some("Client not found or inactive".to_string()),
                error_uri: None,
                state: params.state.clone(),
            })?;

        // Validate redirect_uri
        if !client.redirect_uris.contains(&params.redirect_uri) {
            return Err(ErrorResponse {
                error: "invalid_request".to_string(),
                error_description: Some("Invalid redirect_uri".to_string()),
                error_uri: None,
                state: params.state.clone(),
            });
        }

        // Additional security: validate redirect URI is not to our own homeserver to prevent loops
        if params.redirect_uri.contains(&self.homeserver_name) {
            warn!(
                "OAuth2 redirect URI contains homeserver name - potential security issue: {}",
                params.redirect_uri
            );
            // Allow but log for security monitoring
        }

        // Validate PKCE parameters if present
        if let Some(ref challenge) = params.code_challenge {
            let method = params.code_challenge_method.as_deref().unwrap_or("plain");
            if method != "plain" && method != "S256" {
                return Err(ErrorResponse {
                    error: "invalid_request".to_string(),
                    error_description: Some("Invalid code_challenge_method".to_string()),
                    error_uri: None,
                    state: params.state.clone(),
                });
            }

            // Validate challenge length according to RFC 7636
            if challenge.len() < 43 || challenge.len() > 128 {
                return Err(ErrorResponse {
                    error: "invalid_request".to_string(),
                    error_description: Some(
                        "Code challenge must be between 43-128 characters".to_string(),
                    ),
                    error_uri: None,
                    state: params.state.clone(),
                });
            }

            debug!(
                "PKCE challenge validated: method={}, challenge_len={}",
                method,
                challenge.len()
            );
        }

        // CSRF Protection: Validate state parameter if user is authenticated
        if let Some(user_id) = &authenticated_user_id {
            if let Some(state) = &params.state {
                if !self.validate_csrf_token(state, user_id).await {
                    return Err(ErrorResponse {
                        error: "invalid_request".to_string(),
                        error_description: Some("Invalid CSRF token".to_string()),
                        error_uri: None,
                        state: params.state,
                    });
                }
            } else {
                return Err(ErrorResponse {
                    error: "invalid_request".to_string(),
                    error_description: Some("Missing CSRF token".to_string()),
                    error_uri: None,
                    state: None,
                });
            }
        }

        // Check if user is authenticated
        let user_id = authenticated_user_id.ok_or_else(|| {
            // Redirect to login with original request parameters
            let login_url = format!(
                "/login?client_id={}&redirect_uri={}&response_type={}&state={}",
                urlencoding::encode(&params.client_id),
                urlencoding::encode(&params.redirect_uri),
                urlencoding::encode(&params.response_type),
                params.state.as_deref().unwrap_or("")
            );

            ErrorResponse {
                error: "login_required".to_string(),
                error_description: Some("User authentication required".to_string()),
                error_uri: Some(login_url),
                state: params.state.clone(),
            }
        })?;

        // Generate authorization code
        let code = self
            .oauth2_repo
            .create_authorization_code(
                &params.client_id,
                &user_id,
                &params.redirect_uri,
                params.scope.as_deref(),
                params.code_challenge.as_deref(),
                params.code_challenge_method.as_deref(),
            )
            .await
            .map_err(|_| ErrorResponse {
                error: "server_error".to_string(),
                error_description: Some("Failed to create authorization code".to_string()),
                error_uri: None,
                state: params.state.clone(),
            })?;

        // Build redirect URL with authorization code
        let mut redirect_url = format!("{}?code={}", params.redirect_uri, code);
        if let Some(state) = params.state {
            redirect_url.push_str(&format!("&state={}", urlencoding::encode(&state)));
        }

        Ok(Redirect::temporary(&redirect_url))
    }

    /// Handle OAuth 2.0 token exchange
    pub async fn token_exchange(
        &self,
        request: TokenRequest,
    ) -> Result<TokenResponse, ErrorResponse> {
        // Validate grant_type
        if request.grant_type != "authorization_code" {
            return Err(ErrorResponse {
                error: "unsupported_grant_type".to_string(),
                error_description: Some(
                    "Only 'authorization_code' grant type is supported".to_string(),
                ),
                error_uri: None,
                state: None,
            });
        }

        // Validate client
        let client = self
            .oauth2_repo
            .get_client(&request.client_id)
            .await
            .map_err(|_| ErrorResponse {
                error: "invalid_client".to_string(),
                error_description: Some("Client not found or inactive".to_string()),
                error_uri: None,
                state: None,
            })?
            .ok_or_else(|| ErrorResponse {
                error: "invalid_client".to_string(),
                error_description: Some("Client not found or inactive".to_string()),
                error_uri: None,
                state: None,
            })?;

        // Validate client secret for confidential clients
        if client.client_type == "confidential" {
            if let Some(ref stored_secret) = client.client_secret {
                let provided_secret = request.client_secret.as_deref().unwrap_or("");
                if stored_secret != provided_secret {
                    return Err(ErrorResponse {
                        error: "invalid_client".to_string(),
                        error_description: Some("Invalid client secret".to_string()),
                        error_uri: None,
                        state: None,
                    });
                }
            } else {
                return Err(ErrorResponse {
                    error: "invalid_client".to_string(),
                    error_description: Some("Client secret required".to_string()),
                    error_uri: None,
                    state: None,
                });
            }
        }

        // Validate and consume authorization code
        let auth_code = self
            .oauth2_repo
            .consume_authorization_code(&request.code)
            .await
            .map_err(|_| ErrorResponse {
                error: "invalid_grant".to_string(),
                error_description: Some("Invalid or expired authorization code".to_string()),
                error_uri: None,
                state: None,
            })?
            .ok_or_else(|| ErrorResponse {
                error: "invalid_grant".to_string(),
                error_description: Some("Invalid or expired authorization code".to_string()),
                error_uri: None,
                state: None,
            })?;

        // Validate redirect_uri matches
        if auth_code.redirect_uri != request.redirect_uri {
            return Err(ErrorResponse {
                error: "invalid_grant".to_string(),
                error_description: Some("Redirect URI mismatch".to_string()),
                error_uri: None,
                state: None,
            });
        }

        // Validate client_id matches
        if auth_code.client_id != request.client_id {
            return Err(ErrorResponse {
                error: "invalid_grant".to_string(),
                error_description: Some("Client ID mismatch".to_string()),
                error_uri: None,
                state: None,
            });
        }

        // Validate PKCE if present
        if let Some(ref challenge) = auth_code.code_challenge {
            let verifier = request.code_verifier.as_deref().ok_or_else(|| ErrorResponse {
                error: "invalid_request".to_string(),
                error_description: Some("PKCE code_verifier required".to_string()),
                error_uri: None,
                state: None,
            })?;

            let method = auth_code.code_challenge_method.as_deref().unwrap_or("plain");
            if !self.verify_pkce_challenge(challenge, verifier, method) {
                return Err(ErrorResponse {
                    error: "invalid_grant".to_string(),
                    error_description: Some("PKCE verification failed".to_string()),
                    error_uri: None,
                    state: None,
                });
            }
        }

        // Generate access token and refresh token
        let device_id = format!("oauth2_{}", Uuid::new_v4());
        let access_token = format!("mxat_{}", Uuid::new_v4());
        let refresh_token = format!("mxrt_{}", Uuid::new_v4());

        // Create session
        match self
            .session_service
            .create_user_session(
                &auth_code.user_id,
                &device_id,
                &access_token,
                Some(&refresh_token),
            )
            .await
        {
            Ok(_) => {
                info!(
                    "OAuth2 token exchange successful for user: {} on homeserver: {}",
                    auth_code.user_id, self.homeserver_name
                );

                Ok(TokenResponse {
                    access_token,
                    token_type: "Bearer".to_string(),
                    expires_in: 3600, // 1 hour
                    refresh_token: Some(refresh_token),
                    scope: auth_code.scope,
                })
            },
            Err(_) => Err(ErrorResponse {
                error: "server_error".to_string(),
                error_description: Some("Failed to create session".to_string()),
                error_uri: None,
                state: None,
            }),
        }
    }

    /// Verify PKCE challenge
    fn verify_pkce_challenge(&self, challenge: &str, verifier: &str, method: &str) -> bool {
        match method {
            "plain" => challenge == verifier,
            "S256" => {
                use base64::{Engine, engine::general_purpose};
                use sha2::{Digest, Sha256};

                let mut hasher = Sha256::new();
                hasher.update(verifier.as_bytes());
                let hash = hasher.finalize();
                let encoded = general_purpose::URL_SAFE_NO_PAD.encode(hash);
                challenge == encoded
            },
            _ => false,
        }
    }

    /// Register a new OAuth2 client
    pub async fn register_client(
        &self,
        client_name: &str,
        redirect_uris: Vec<String>,
        client_type: &str,
        allowed_scopes: Option<Vec<String>>,
    ) -> Result<OAuth2Client, MatrixAuthError> {
        self.oauth2_repo
            .register_client(client_name, redirect_uris, client_type, allowed_scopes)
            .await
            .map_err(|e| {
                MatrixAuthError::DatabaseError(format!("Failed to register client: {}", e))
            })
    }
}

/// Authorization endpoint handler
pub async fn authorize_handler<C: Connection>(
    State(oauth2_service): State<Arc<OAuth2Service<C>>>,
    Query(params): Query<AuthorizationRequest>,
    // Extract authenticated user from session/cookies
    authenticated_user: Option<String>,
) -> impl IntoResponse {
    match oauth2_service.authorize(params, authenticated_user).await {
        Ok(redirect) => redirect.into_response(),
        Err(error) => {
            let status = match error.error.as_str() {
                "invalid_client" | "invalid_request" => StatusCode::BAD_REQUEST,
                "unsupported_response_type" => StatusCode::BAD_REQUEST,
                "login_required" => StatusCode::UNAUTHORIZED,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, Json(error)).into_response()
        },
    }
}

/// Token endpoint handler
pub async fn token_handler<C: Connection>(
    State(oauth2_service): State<Arc<OAuth2Service<C>>>,
    Json(request): Json<TokenRequest>,
) -> impl IntoResponse {
    match oauth2_service.token_exchange(request).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => {
            let status = match error.error.as_str() {
                "invalid_client" | "invalid_grant" | "invalid_request" => StatusCode::BAD_REQUEST,
                "unsupported_grant_type" => StatusCode::BAD_REQUEST,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, Json(error)).into_response()
        },
    }
}
