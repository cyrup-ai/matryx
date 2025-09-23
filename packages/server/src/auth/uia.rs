use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use surrealdb::{Connection, engine::any::Any};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::auth::MatrixAuthError;
use matryx_surrealdb::repository::uia::{UiaFlow, UiaRepository, UiaSession};

/// UIA authentication request
#[derive(Debug, Deserialize)]
pub struct UiaAuthRequest {
    pub auth: Option<UiaAuth>,
    pub session: Option<String>,
}

/// UIA authentication data
#[derive(Debug, Deserialize)]
pub struct UiaAuth {
    #[serde(rename = "type")]
    pub auth_type: String,
    pub session: Option<String>,
    #[serde(flatten)]
    pub auth_data: HashMap<String, serde_json::Value>,
}

/// UIA authentication response
#[derive(Debug, Serialize)]
pub struct UiaAuthResponse {
    pub flows: Vec<UiaFlow>,
    pub params: HashMap<String, serde_json::Value>,
    pub session: String,
    pub completed: Option<Vec<String>>,
    pub error: Option<String>,
    pub errcode: Option<String>,
}

/// UIA error response
#[derive(Debug, Serialize)]
pub struct UiaErrorResponse {
    pub flows: Vec<UiaFlow>,
    pub params: HashMap<String, serde_json::Value>,
    pub session: Option<String>,
    pub completed: Option<Vec<String>>,
    pub error: String,
    pub errcode: String,
}

/// Password authentication stage
#[derive(Debug, Deserialize)]
pub struct PasswordAuth {
    pub password: String,
    pub user: Option<String>,
    pub identifier: Option<UserIdentifier>,
}

/// User identifier for authentication
#[derive(Debug, Deserialize)]
pub struct UserIdentifier {
    #[serde(rename = "type")]
    pub id_type: String,
    pub user: Option<String>,
    pub medium: Option<String>,
    pub address: Option<String>,
}

/// CAPTCHA authentication stage
#[derive(Debug, Deserialize)]
pub struct CaptchaAuth {
    pub response: String,
}

/// Email verification stage
#[derive(Debug, Deserialize)]
pub struct EmailAuth {
    pub threepid_creds: ThreepidCredentials,
}

/// Third-party ID credentials
#[derive(Debug, Deserialize)]
pub struct ThreepidCredentials {
    pub sid: String,
    pub client_secret: String,
    pub id_server: Option<String>,
    pub id_access_token: Option<String>,
}

/// Service for managing User-Interactive Authentication flows
pub struct UiaService {
    uia_repo: UiaRepository<Any>,
    session_lifetime: Duration,
}

impl UiaService {
    pub fn new(uia_repo: UiaRepository<Any>) -> Self {
        Self {
            uia_repo,
            session_lifetime: Duration::minutes(10), // 10 minutes for UIA sessions
        }
    }

    /// Start a new UIA session for sensitive operation
    pub async fn start_session(
        &self,
        user_id: Option<&str>,
        device_id: Option<&str>,
        flows: Vec<UiaFlow>,
        params: HashMap<String, serde_json::Value>,
    ) -> Result<UiaSession, MatrixAuthError> {
        let session_id = format!("uia_{}", Uuid::new_v4());
        let now = Utc::now();

        let _session = self
            .uia_repo
            .create_session(
                user_id,
                device_id,
                flows.clone(),
                params.clone(),
                self.session_lifetime,
            )
            .await
            .map_err(|e| {
                MatrixAuthError::DatabaseError(format!("Failed to create UIA session: {}", e))
            })?;

        let session = UiaSession {
            session_id: session_id.clone(),
            user_id: user_id.map(|s| s.to_string()),
            device_id: device_id.map(|s| s.to_string()),
            flows,
            completed_stages: Vec::new(),
            current_stage: None,
            auth_data: HashMap::new(),
            params,
            created_at: now,
            expires_at: now + self.session_lifetime,
            completed: false,
        };

        info!("Started UIA session: {} for user: {:?}", session_id, user_id);
        Ok(session)
    }

    /// Process UIA authentication step
    pub async fn process_auth(
        &self,
        session_id: &str,
        auth: UiaAuth,
    ) -> Result<UiaAuthResponse, UiaErrorResponse> {
        // Get and validate session
        let mut session = match self.uia_repo.get_session(session_id).await {
            Ok(Some(s)) => s,
            _ => {
                return Err(UiaErrorResponse {
                    flows: self.get_default_flows(),
                    params: HashMap::new(),
                    session: None,
                    completed: None,
                    error: "Invalid session".to_string(),
                    errcode: "M_UNKNOWN".to_string(),
                });
            },
        };

        // Check if session has expired
        if Utc::now() > session.expires_at {
            return Err(UiaErrorResponse {
                flows: session.flows.clone(),
                params: session.params.clone(),
                session: Some(session_id.to_string()),
                completed: Some(session.completed_stages.clone()),
                error: "Session expired".to_string(),
                errcode: "M_UNKNOWN".to_string(),
            });
        }

        // Process the authentication stage
        match self.process_auth_stage(&mut session, auth).await {
            Ok(stage_completed) => {
                if stage_completed {
                    // Check if all required stages are completed
                    if self.is_flow_completed(&session) {
                        session.completed = true;
                        self.uia_repo.update_session(&session).await.map_err(|_| {
                            UiaErrorResponse {
                                flows: session.flows.clone(),
                                params: session.params.clone(),
                                session: Some(session_id.to_string()),
                                completed: Some(session.completed_stages.clone()),
                                error: "Failed to update session".to_string(),
                                errcode: "M_UNKNOWN".to_string(),
                            }
                        })?;

                        Ok(UiaAuthResponse {
                            flows: session.flows.clone(),
                            params: session.params.clone(),
                            session: session_id.to_string(),
                            completed: Some(session.completed_stages.clone()),
                            error: None,
                            errcode: None,
                        })
                    } else {
                        // More stages needed
                        self.uia_repo.update_session(&session).await.map_err(|_| {
                            UiaErrorResponse {
                                flows: session.flows.clone(),
                                params: session.params.clone(),
                                session: Some(session_id.to_string()),
                                completed: Some(session.completed_stages.clone()),
                                error: "Failed to update session".to_string(),
                                errcode: "M_UNKNOWN".to_string(),
                            }
                        })?;

                        Err(UiaErrorResponse {
                            flows: session.flows.clone(),
                            params: session.params.clone(),
                            session: Some(session_id.to_string()),
                            completed: Some(session.completed_stages.clone()),
                            error: "Additional authentication required".to_string(),
                            errcode: "M_FORBIDDEN".to_string(),
                        })
                    }
                } else {
                    // Authentication stage failed
                    Err(UiaErrorResponse {
                        flows: session.flows.clone(),
                        params: session.params.clone(),
                        session: Some(session_id.to_string()),
                        completed: Some(session.completed_stages.clone()),
                        error: "Authentication failed".to_string(),
                        errcode: "M_FORBIDDEN".to_string(),
                    })
                }
            },
            Err(e) => {
                Err(UiaErrorResponse {
                    flows: session.flows.clone(),
                    params: session.params.clone(),
                    session: Some(session_id.to_string()),
                    completed: Some(session.completed_stages.clone()),
                    error: format!("Authentication error: {:?}", e),
                    errcode: "M_FORBIDDEN".to_string(),
                })
            },
        }
    }

    /// Check if UIA session is completed
    pub async fn is_session_completed(&self, session_id: &str) -> Result<bool, MatrixAuthError> {
        let session = self
            .uia_repo
            .get_session(session_id)
            .await
            .map_err(|e| {
                MatrixAuthError::DatabaseError(format!("Failed to get UIA session: {}", e))
            })?
            .ok_or(MatrixAuthError::UnknownToken)?;
        Ok(session.completed)
    }

    /// Get UIA session data
    pub async fn get_session_data(
        &self,
        session_id: &str,
    ) -> Result<HashMap<String, serde_json::Value>, MatrixAuthError> {
        let session = self
            .uia_repo
            .get_session(session_id)
            .await
            .map_err(|e| {
                MatrixAuthError::DatabaseError(format!("Failed to get UIA session: {}", e))
            })?
            .ok_or(MatrixAuthError::UnknownToken)?;
        Ok(session.auth_data)
    }

    /// Clean up expired UIA sessions
    pub async fn cleanup_expired_sessions(&self) -> Result<u64, MatrixAuthError> {
        let count = self.uia_repo.cleanup_expired_sessions().await.map_err(|e| {
            MatrixAuthError::DatabaseError(format!("Failed to cleanup UIA sessions: {}", e))
        })?;

        if count > 0 {
            info!("Cleaned up {} expired UIA sessions", count);
        }

        Ok(count)
    }

    /// Process specific authentication stage
    async fn process_auth_stage(
        &self,
        session: &mut UiaSession,
        auth: UiaAuth,
    ) -> Result<bool, MatrixAuthError> {
        match auth.auth_type.as_str() {
            "m.login.password" => self.process_password_auth(session, auth).await,
            "m.login.recaptcha" => self.process_captcha_auth(session, auth).await,
            "m.login.email.identity" => self.process_email_auth(session, auth).await,
            "m.login.terms" => self.process_terms_auth(session, auth).await,
            _ => {
                warn!("Unknown UIA auth type: {}", auth.auth_type);
                Err(MatrixAuthError::InvalidXMatrixFormat)
            },
        }
    }

    /// Process password authentication stage
    async fn process_password_auth(
        &self,
        session: &mut UiaSession,
        auth: UiaAuth,
    ) -> Result<bool, MatrixAuthError> {
        // Extract password from auth data
        let password = auth
            .auth_data
            .get("password")
            .and_then(|v| v.as_str())
            .ok_or(MatrixAuthError::InvalidXMatrixFormat)?;

        // For password auth, we need to verify against the user's stored password
        let user_id = session.user_id.as_ref().ok_or(MatrixAuthError::InvalidXMatrixFormat)?;

        if self.verify_user_password(user_id, password).await? {
            session.completed_stages.push("m.login.password".to_string());
            session
                .auth_data
                .insert("password_verified".to_string(), serde_json::Value::Bool(true));
            info!("Password authentication completed for UIA session: {}", session.session_id);
            Ok(true)
        } else {
            warn!("Password authentication failed for UIA session: {}", session.session_id);
            Ok(false)
        }
    }

    /// Process CAPTCHA authentication stage
    async fn process_captcha_auth(
        &self,
        session: &mut UiaSession,
        auth: UiaAuth,
    ) -> Result<bool, MatrixAuthError> {
        let response = auth
            .auth_data
            .get("response")
            .and_then(|v| v.as_str())
            .ok_or(MatrixAuthError::InvalidXMatrixFormat)?;

        // Verify CAPTCHA response (implementation depends on CAPTCHA service)
        if self.verify_captcha_response(response).await? {
            session.completed_stages.push("m.login.recaptcha".to_string());
            session
                .auth_data
                .insert("captcha_verified".to_string(), serde_json::Value::Bool(true));
            info!("CAPTCHA authentication completed for UIA session: {}", session.session_id);
            Ok(true)
        } else {
            warn!("CAPTCHA authentication failed for UIA session: {}", session.session_id);
            Ok(false)
        }
    }

    /// Process email authentication stage
    async fn process_email_auth(
        &self,
        session: &mut UiaSession,
        auth: UiaAuth,
    ) -> Result<bool, MatrixAuthError> {
        // Extract threepid credentials
        let threepid_creds = auth
            .auth_data
            .get("threepid_creds")
            .ok_or(MatrixAuthError::InvalidXMatrixFormat)?;

        // Verify email token (implementation depends on email service)
        if self.verify_email_token(threepid_creds).await? {
            session.completed_stages.push("m.login.email.identity".to_string());
            session
                .auth_data
                .insert("email_verified".to_string(), serde_json::Value::Bool(true));
            info!("Email authentication completed for UIA session: {}", session.session_id);
            Ok(true)
        } else {
            warn!("Email authentication failed for UIA session: {}", session.session_id);
            Ok(false)
        }
    }

    /// Process terms of service acceptance
    async fn process_terms_auth(
        &self,
        session: &mut UiaSession,
        _auth: UiaAuth,
    ) -> Result<bool, MatrixAuthError> {
        // Terms acceptance is typically just confirming the user has agreed
        session.completed_stages.push("m.login.terms".to_string());
        session
            .auth_data
            .insert("terms_accepted".to_string(), serde_json::Value::Bool(true));
        info!("Terms acceptance completed for UIA session: {}", session.session_id);
        Ok(true)
    }

    /// Check if any flow is completed
    fn is_flow_completed(&self, session: &UiaSession) -> bool {
        session
            .flows
            .iter()
            .any(|flow| flow.stages.iter().all(|stage| session.completed_stages.contains(stage)))
    }

    /// Verify user password using existing UserRepository
    async fn verify_user_password(
        &self,
        user_id: &str,
        password: &str,
    ) -> Result<bool, MatrixAuthError> {
        // Use existing UserRepository
        use matryx_surrealdb::repository::UserRepository;

        // Get the database connection - use into() to convert to Surreal<Any>
        let any_db = self.uia_repo.get_db().clone().into();
        let user_repo = UserRepository::new(any_db);

        match user_repo.get_by_id(user_id).await {
            Ok(Some(user)) => {
                // Use bcrypt to verify password against stored hash
                match bcrypt::verify(password, &user.password_hash) {
                    Ok(is_valid) => Ok(is_valid),
                    Err(_) => Ok(false),
                }
            },
            Ok(None) => Ok(false), // User not found
            Err(_) => Err(MatrixAuthError::DatabaseError("Failed to get user".to_string())),
        }
    }

    /// Verify CAPTCHA response using existing CaptchaService
    async fn verify_captcha_response(&self, response: &str) -> Result<bool, MatrixAuthError> {
        // Use existing CaptchaService from captcha.rs
        use crate::auth::CaptchaService;
        use crate::auth::captcha::{CaptchaConfig, CaptchaVerificationRequest};
        use matryx_surrealdb::repository::CaptchaRepository;

        let config = CaptchaConfig::from_env();
        let captcha_repo = CaptchaRepository::new(self.uia_repo.get_db().clone());
        let captcha_service = CaptchaService::new(captcha_repo, config);

        // Create verification request (challenge_id would be stored in UIA session)
        let request = CaptchaVerificationRequest {
            challenge_id: "challenge_from_session".to_string(), // Get from UIA session params
            response: response.to_string(),
            remote_ip: None, // Could extract from request context
        };

        match captcha_service.verify_captcha(request).await {
            Ok(verification_response) => Ok(verification_response.success),
            Err(_) => Ok(false),
        }
    }

    /// Verify email token using existing ThirdPartyValidationSessionRepository
    async fn verify_email_token(
        &self,
        threepid_creds: &serde_json::Value,
    ) -> Result<bool, MatrixAuthError> {
        // Use existing ThirdPartyValidationSessionRepository
        use matryx_surrealdb::repository::{
            ThirdPartyValidationSessionRepository,
            ThirdPartyValidationSessionRepositoryTrait,
        };

        let any_db = self.uia_repo.get_db().clone().into();
        let threepid_repo = ThirdPartyValidationSessionRepository::new(any_db);

        // Extract session ID and client secret from threepid_creds
        let sid = threepid_creds
            .get("sid")
            .and_then(|v| v.as_str())
            .ok_or(MatrixAuthError::InvalidXMatrixFormat)?;

        let client_secret = threepid_creds
            .get("client_secret")
            .and_then(|v| v.as_str())
            .ok_or(MatrixAuthError::InvalidXMatrixFormat)?;

        // Validate session using existing repository
        match threepid_repo.get_session_by_id_and_secret(sid, client_secret).await {
            Ok(Some(session)) => {
                // Check if session is verified and not expired
                Ok(session.verified && !session.is_expired())
            },
            Ok(None) => Ok(false), // Session not found or invalid
            Err(_) => {
                Err(MatrixAuthError::DatabaseError("Failed to validate email token".to_string()))
            },
        }
    }

    /// Get default UIA flows for different operations
    fn get_default_flows(&self) -> Vec<UiaFlow> {
        vec![
            UiaFlow { stages: vec!["m.login.password".to_string()] },
            UiaFlow {
                stages: vec![
                    "m.login.recaptcha".to_string(),
                    "m.login.password".to_string(),
                ],
            },
        ]
    }
}

/// Configuration for UIA service
#[derive(Debug, Clone)]
pub struct UiaConfig {
    pub session_lifetime_minutes: i64,
    pub require_captcha: bool,
    pub require_email_verification: bool,
    pub cleanup_interval_hours: u64,
}

impl UiaConfig {
    pub fn from_env() -> Self {
        Self {
            session_lifetime_minutes: std::env::var("UIA_SESSION_LIFETIME_MINUTES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10),
            require_captcha: std::env::var("UIA_REQUIRE_CAPTCHA")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(false),
            require_email_verification: std::env::var("UIA_REQUIRE_EMAIL_VERIFICATION")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(false),
            cleanup_interval_hours: std::env::var("UIA_CLEANUP_INTERVAL_HOURS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(24),
        }
    }
}
