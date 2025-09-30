use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use surrealdb::engine::any::Any;
use tracing::{info, warn};
use uuid::Uuid;

use crate::auth::MatrixAuthError;
use matryx_surrealdb::repository::{AuthRepository, uia::{UiaRepository, UiaSession}};

// Re-export UiaFlow for use by other modules in the server crate
pub use matryx_surrealdb::repository::uia::UiaFlow;

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
    auth_repo: AuthRepository<Any>,
    homeserver_name: String,
    session_lifetime: Duration,
    require_captcha: bool,
    require_email_verification: bool,
    _cleanup_interval_hours: u64, // TODO: Implement periodic cleanup task
}

impl UiaService {
    pub fn new(uia_repo: UiaRepository<Any>, auth_repo: AuthRepository<Any>, homeserver_name: String, config: UiaConfig) -> Self {
        Self {
            uia_repo,
            auth_repo,
            homeserver_name,
            session_lifetime: Duration::minutes(config.session_lifetime_minutes),
            require_captcha: config.require_captcha,
            require_email_verification: config.require_email_verification,
            _cleanup_interval_hours: config.cleanup_interval_hours,
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
        // Parse password auth data using proper struct
        let auth_data_value = serde_json::to_value(&auth.auth_data)
            .map_err(|_| MatrixAuthError::InvalidXMatrixFormat)?;
        let password_auth: PasswordAuth = serde_json::from_value(auth_data_value)
            .map_err(|_| MatrixAuthError::InvalidXMatrixFormat)?;

        // Determine user ID - either from session or identifier
        let user_id = if let Some(user_id) = &session.user_id {
            user_id.clone()
        } else if let Some(identifier) = &password_auth.identifier {
            self.resolve_user_identifier(identifier).await?
        } else if let Some(user) = &password_auth.user {
            user.clone()
        } else {
            return Err(MatrixAuthError::InvalidXMatrixFormat);
        };

        if self.verify_user_password(&user_id, &password_auth.password).await? {
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
        // Parse CAPTCHA auth data using proper struct
        let auth_data_value = serde_json::to_value(&auth.auth_data)
            .map_err(|_| MatrixAuthError::InvalidXMatrixFormat)?;
        let captcha_auth: CaptchaAuth = serde_json::from_value(auth_data_value)
            .map_err(|_| MatrixAuthError::InvalidXMatrixFormat)?;

        // Verify CAPTCHA response (implementation depends on CAPTCHA service)
        if self.verify_captcha_response(&captcha_auth.response).await? {
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
        // Parse email auth data using proper struct
        let auth_data_value = serde_json::to_value(&auth.auth_data)
            .map_err(|_| MatrixAuthError::InvalidXMatrixFormat)?;
        let email_auth: EmailAuth = serde_json::from_value(auth_data_value)
            .map_err(|_| MatrixAuthError::InvalidXMatrixFormat)?;

        // Verify email token (implementation depends on email service)
        if self.verify_email_token(&email_auth.threepid_creds).await? {
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

        // Get the database connection - UiaRepository<Any> already has Surreal<Any>
        let user_repo = UserRepository::new(self.uia_repo.get_db().clone());

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
        threepid_creds: &ThreepidCredentials,
    ) -> Result<bool, MatrixAuthError> {
        // Log deprecated identity server fields if present (Matrix spec deprecation)
        if threepid_creds.id_server.is_some() || threepid_creds.id_access_token.is_some() {
            tracing::debug!("Identity server parameters provided (deprecated in Matrix spec)");
        }
        
        // Use existing ThirdPartyValidationSessionRepository
        use matryx_surrealdb::repository::{
            ThirdPartyValidationSessionRepository,
            ThirdPartyValidationSessionRepositoryTrait,
        };

        let any_db = self.uia_repo.get_db().clone();
        let threepid_repo = ThirdPartyValidationSessionRepository::new(any_db);

        // Extract session ID and client secret from threepid_creds struct
        let sid = &threepid_creds.sid;
        let client_secret = &threepid_creds.client_secret;

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

    /// Resolve user identifier to Matrix user ID
    async fn resolve_user_identifier(&self, identifier: &UserIdentifier) -> Result<String, MatrixAuthError> {
        match identifier.id_type.as_str() {
            "m.id.user" => {
                // Direct user ID or localpart
                if let Some(user) = &identifier.user {
                    if user.contains(':') {
                        // Already a full MXID
                        Ok(user.clone())
                    } else {
                        // Convert localpart to full MXID
                        Ok(format!("@{}:{}", user, self.homeserver_name))
                    }
                } else {
                    Err(MatrixAuthError::InvalidXMatrixFormat)
                }
            },
            "m.id.thirdparty" => {
                // Third-party identifier (email, phone, etc.)
                let medium = identifier.medium.as_ref()
                    .ok_or(MatrixAuthError::InvalidXMatrixFormat)?;
                let address = identifier.address.as_ref()
                    .ok_or(MatrixAuthError::InvalidXMatrixFormat)?;
                
                // Look up user by third-party identifier
                match self.auth_repo.get_user_by_threepid(medium, address).await {
                    Ok(Some(user_id)) => Ok(user_id),
                    Ok(None) => Err(MatrixAuthError::InvalidCredentials),
                    Err(_) => Err(MatrixAuthError::DatabaseError("Failed to resolve user identifier".to_string())),
                }
            },
            "m.id.phone" => {
                // Phone number identifier
                let phone = identifier.address.as_ref()
                    .ok_or(MatrixAuthError::InvalidXMatrixFormat)?;
                
                match self.auth_repo.get_user_by_threepid("msisdn", phone).await {
                    Ok(Some(user_id)) => Ok(user_id),
                    Ok(None) => Err(MatrixAuthError::InvalidCredentials),
                    Err(_) => Err(MatrixAuthError::DatabaseError("Failed to resolve phone identifier".to_string())),
                }
            },
            _ => {
                warn!("Unknown user identifier type: {}", identifier.id_type);
                Err(MatrixAuthError::InvalidXMatrixFormat)
            }
        }
    }

    /// Get default UIA flows for different operations
    fn get_default_flows(&self) -> Vec<UiaFlow> {
        let mut flows = vec![
            UiaFlow { stages: vec!["m.login.password".to_string()] },
        ];
        
        // Add captcha flow if configured
        if self.require_captcha {
            flows.push(UiaFlow {
                stages: vec![
                    "m.login.recaptcha".to_string(),
                    "m.login.password".to_string(),
                ],
            });
        }
        
        // Add email verification flow if configured
        if self.require_email_verification {
            flows.push(UiaFlow {
                stages: vec![
                    "m.login.email.identity".to_string(),
                    "m.login.password".to_string(),
                ],
            });
        }
        
        flows
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
