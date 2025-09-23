use crate::repository::captcha::CaptchaChallenge;
use crate::repository::error::RepositoryError;

use crate::repository::uia::{AuthStage, UiaFlow, UiaSession};
use crate::repository::{
    AuthRepository,
    SessionRepository,
    captcha::CaptchaRepository,
    oauth2::OAuth2Repository,
    uia::UiaRepository,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use surrealdb::Connection;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResult {
    pub success: bool,
    pub user_id: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenClaims {
    pub user_id: String,
    pub device_id: String,
    pub session_id: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub scope: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub token_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuth2Flow {
    pub authorization_url: String,
    pub code: String,
    pub state: String,
    pub expires_in: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIAResult {
    pub session_id: String,
    pub completed: bool,
    pub flows: Vec<UiaFlow>,
    pub completed_stages: Vec<String>,
    pub next_stage: Option<String>,
    pub error: Option<String>,
}

pub struct AuthenticationService<C: Connection> {
    auth_repo: AuthRepository<C>,
    session_repo: SessionRepository,
    oauth2_repo: OAuth2Repository<C>,
    captcha_repo: CaptchaRepository<C>,
    uia_repo: UiaRepository<C>,
}

impl<C: Connection> AuthenticationService<C> {
    pub fn new(
        auth_repo: AuthRepository<C>,
        session_repo: SessionRepository,
        oauth2_repo: OAuth2Repository<C>,
        captcha_repo: CaptchaRepository<C>,
        uia_repo: UiaRepository<C>,
    ) -> Self {
        Self {
            auth_repo,
            session_repo,
            oauth2_repo,
            captcha_repo,
            uia_repo,
        }
    }

    /// Authenticate user with password
    pub async fn authenticate_user(
        &self,
        user_id: &str,
        password: &str,
        device_id: &str,
    ) -> Result<AuthResult, RepositoryError> {
        // Validate user credentials
        if !self.auth_repo.validate_user_credentials(user_id, password).await? {
            return Err(RepositoryError::Unauthorized {
                reason: "Invalid credentials".to_string(),
            });
        }

        // Check if user is active
        if !self.auth_repo.is_user_active(user_id).await? {
            return Err(RepositoryError::Unauthorized {
                reason: "User account is inactive".to_string(),
            });
        }

        // Validate device
        if !self.auth_repo.validate_device(device_id, user_id).await? {
            return Err(RepositoryError::Validation {
                field: "device_id".to_string(),
                message: "Invalid device".to_string(),
            });
        }

        // Create refresh token
        let refresh_token = self.auth_repo.create_refresh_token(user_id, device_id).await?;

        // Generate access token (simplified - would use JWT in production)
        let access_token = uuid::Uuid::new_v4().to_string();
        let expires_in = 3600; // 1 hour

        // Update user last seen
        self.auth_repo.update_user_last_seen(user_id).await?;

        Ok(AuthResult {
            success: true,
            user_id: user_id.to_string(),
            access_token,
            refresh_token: refresh_token.token,
            expires_in,
            device_id: device_id.to_string(),
        })
    }

    /// Validate access token
    pub async fn validate_access_token(
        &self,
        token: &str,
    ) -> Result<Option<TokenClaims>, RepositoryError> {
        // In a real implementation, this would decode and validate a JWT
        // For now, we'll check if the token exists in sessions
        if let Some(session) = self.session_repo.get_by_access_token(token).await?
            && session.is_active && session.expires_at.is_some_and(|exp| exp > Utc::now()) {
            return Ok(Some(TokenClaims {
                user_id: session.user_id,
                device_id: session.device_id,
                session_id: session.session_id,
                issued_at: session.created_at,
                expires_at: session.expires_at.unwrap(),
                scope: vec!["matrix".to_string()],
            }));
        }

        Ok(None)
    }

    /// Refresh access token using refresh token
    pub async fn refresh_access_token(
        &self,
        refresh_token: &str,
    ) -> Result<TokenPair, RepositoryError> {
        // Validate refresh token
        if let Some(token_data) = self.auth_repo.validate_refresh_token(refresh_token).await? {
            // Generate new access token
            let access_token = uuid::Uuid::new_v4().to_string();
            let expires_in = 3600; // 1 hour

            // Create new refresh token
            let new_refresh_token = self
                .auth_repo
                .create_refresh_token(&token_data.user_id, &token_data.device_id)
                .await?;

            // Revoke old refresh token
            self.auth_repo.revoke_refresh_token(refresh_token).await?;

            Ok(TokenPair {
                access_token,
                refresh_token: new_refresh_token.token,
                expires_in,
                token_type: "Bearer".to_string(),
            })
        } else {
            Err(RepositoryError::Unauthorized { reason: "Invalid refresh token".to_string() })
        }
    }

    /// Logout user from specific session
    pub async fn logout_user(
        &self,
        user_id: &str,
        session_id: &str,
    ) -> Result<(), RepositoryError> {
        // Deactivate session
        self.session_repo.deactivate(session_id).await?;

        // Invalidate UIA sessions for user
        self.uia_repo.revoke_user_sessions(user_id).await?;

        Ok(())
    }

    /// Logout user from all devices except specified one
    pub async fn logout_all_devices(
        &self,
        user_id: &str,
        except_device: Option<&str>,
    ) -> Result<(), RepositoryError> {
        // Get user sessions
        let sessions = self.session_repo.get_user_sessions(user_id).await?;

        for session in sessions {
            if let Some(except) = except_device {
                if session.device_id != except {
                    self.session_repo.deactivate(&session.session_id).await?;
                }
            } else {
                self.session_repo.deactivate(&session.session_id).await?;
            }
        }

        // Invalidate UIA sessions
        self.uia_repo.revoke_user_sessions(user_id).await?;

        Ok(())
    }

    /// Start OAuth2 authorization flow
    pub async fn start_oauth2_flow(
        &self,
        client_id: &str,
        redirect_uri: &str,
        scope: &str,
    ) -> Result<OAuth2Flow, RepositoryError> {
        // Validate client
        if self.oauth2_repo.get_client(client_id).await?.is_none() {
            return Err(RepositoryError::NotFound {
                entity_type: "OAuth2 client".to_string(),
                id: client_id.to_string(),
            });
        }

        // Validate redirect URI
        if !self.oauth2_repo.validate_redirect_uri(client_id, redirect_uri).await? {
            return Err(RepositoryError::Validation {
                field: "redirect_uri".to_string(),
                message: "Invalid redirect URI".to_string(),
            });
        }

        // Validate scope
        if !self.oauth2_repo.validate_scope(client_id, scope).await? {
            return Err(RepositoryError::Validation {
                field: "scope".to_string(),
                message: "Invalid scope".to_string(),
            });
        }

        // Generate authorization code
        let code = uuid::Uuid::new_v4().to_string();
        let state = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        let _expires_at = now + Duration::minutes(10); // 10 minute expiry

        let _code = self
            .oauth2_repo
            .create_authorization_code(
                client_id,
                "", // Will be set when user authorizes
                redirect_uri,
                Some(scope),
                None,
                None,
            )
            .await?;

        let authorization_url = format!(
            "/oauth2/authorize?client_id={}&redirect_uri={}&scope={}&state={}",
            client_id, redirect_uri, scope, state
        );

        Ok(OAuth2Flow {
            authorization_url,
            code,
            state,
            expires_in: 600, // 10 minutes
        })
    }

    /// Complete OAuth2 authorization flow
    pub async fn complete_oauth2_flow(
        &self,
        code: &str,
        client_id: &str,
        code_verifier: Option<&str>,
    ) -> Result<TokenPair, RepositoryError> {
        // Get and consume authorization code
        if let Some(auth_code) = self.oauth2_repo.consume_authorization_code(code).await? {
            // Validate client
            if auth_code.client_id != client_id {
                return Err(RepositoryError::Validation {
                    field: "client_id".to_string(),
                    message: "Client ID mismatch".to_string(),
                });
            }

            // Validate PKCE if present
            if let (Some(challenge), Some(method), Some(verifier)) =
                (&auth_code.code_challenge, &auth_code.code_challenge_method, code_verifier)
                && !self
                    .oauth2_repo
                    .validate_pkce_challenge(verifier, challenge, method)
                    .await?
            {
                return Err(RepositoryError::Validation {
                    field: "code_verifier".to_string(),
                    message: "PKCE validation failed".to_string(),
                });
            }

            // Generate tokens
            let access_token = uuid::Uuid::new_v4().to_string();
            let refresh_token = uuid::Uuid::new_v4().to_string();
            let expires_in = 3600; // 1 hour

            Ok(TokenPair {
                access_token,
                refresh_token,
                expires_in,
                token_type: "Bearer".to_string(),
            })
        } else {
            Err(RepositoryError::NotFound {
                entity_type: "Authorization code".to_string(),
                id: code.to_string(),
            })
        }
    }

    /// Validate CAPTCHA response
    pub async fn validate_captcha(
        &self,
        challenge_id: &str,
        response: &str,
    ) -> Result<bool, RepositoryError> {
        self.captcha_repo.validate_response(challenge_id, response).await
    }

    /// Start User Interactive Authentication session
    pub async fn start_uia_session(
        &self,
        flows: Vec<UiaFlow>,
    ) -> Result<UiaSession, RepositoryError> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        let expires_at = now + Duration::minutes(30); // 30 minute expiry

        let session = UiaSession {
            session_id: session_id.clone(),
            user_id: None,
            device_id: None,
            flows,
            completed_stages: Vec::new(),
            current_stage: None, // Will be determined by first required stage
            auth_data: HashMap::new(),
            params: HashMap::new(),
            created_at: now,
            expires_at,
            completed: false,
        };

        self.uia_repo
            .create_session(
                session.user_id.as_deref(),
                session.device_id.as_deref(),
                session.flows,
                session.params,
                Duration::minutes(30),
            )
            .await
    }

    /// Complete a UIA stage
    pub async fn complete_uia_stage(
        &self,
        session_id: &str,
        stage: AuthStage,
    ) -> Result<UIAResult, RepositoryError> {
        if let Some(session) = self.uia_repo.get_session(session_id).await? {
            // Validate stage based on type
            let stage_valid = match stage.stage_type.as_str() {
                "m.login.password" => {
                    // Validate password
                    if let (Some(user_id), Some(password)) = (
                        stage.params.get("user").and_then(|v| v.as_str()),
                        stage.params.get("password").and_then(|v| v.as_str()),
                    ) {
                        self.auth_repo.validate_user_credentials(user_id, password).await?
                    } else {
                        false
                    }
                },
                "m.login.recaptcha" => {
                    // Validate CAPTCHA
                    if let Some(response) = stage.params.get("response").and_then(|v| v.as_str()) {
                        // In real implementation, would validate with external service
                        !response.is_empty()
                    } else {
                        false
                    }
                },
                _ => false,
            };

            if stage_valid {
                // Complete the stage
                self.uia_repo.complete_stage(session_id, &stage.stage_type).await?;

                // Get updated session
                if let Some(updated_session) = self.uia_repo.get_session(session_id).await? {
                    return Ok(UIAResult {
                        session_id: session_id.to_string(),
                        completed: updated_session.completed,
                        flows: updated_session.flows,
                        completed_stages: updated_session.completed_stages,
                        next_stage: updated_session.current_stage,
                        error: None,
                    });
                }
            }

            Ok(UIAResult {
                session_id: session_id.to_string(),
                completed: false,
                flows: session.flows,
                completed_stages: session.completed_stages,
                next_stage: session.current_stage,
                error: Some("Stage validation failed".to_string()),
            })
        } else {
            Err(RepositoryError::NotFound {
                entity_type: "UIA session".to_string(),
                id: session_id.to_string(),
            })
        }
    }

    /// Create CAPTCHA challenge
    pub async fn create_captcha_challenge(
        &self,
        user_id: Option<String>,
        client_ip: Option<String>,
        user_agent: Option<String>,
    ) -> Result<CaptchaChallenge, RepositoryError> {
        self.captcha_repo
            .create_math_challenge(user_id, client_ip, user_agent)
            .await
    }

    /// Cleanup expired tokens and sessions
    pub async fn cleanup_expired_data(&self) -> Result<(), RepositoryError> {
        let cutoff = Utc::now() - Duration::hours(24); // 24 hours ago

        // Cleanup expired refresh tokens
        self.auth_repo.cleanup_expired_refresh_tokens(cutoff).await?;

        // Cleanup expired sessions
        self.session_repo.cleanup_expired_sessions(cutoff).await?;

        // Cleanup expired OAuth2 codes
        self.oauth2_repo.cleanup_expired_codes().await?;

        // Cleanup expired CAPTCHA challenges
        self.captcha_repo.cleanup_expired_challenges(cutoff).await?;

        // Cleanup expired UIA sessions
        self.uia_repo.cleanup_expired_sessions().await?;

        Ok(())
    }
}
