use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use surrealdb::Connection;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::auth::MatrixAuthError;
use matryx_surrealdb::repository::captcha::{CaptchaChallenge, CaptchaRepository, CaptchaStats};

/// CAPTCHA verification request
#[derive(Debug, Deserialize)]
pub struct CaptchaVerificationRequest {
    pub challenge_id: String,
    pub response: String,
    pub remote_ip: Option<String>,
}

/// CAPTCHA verification response
#[derive(Debug, Serialize)]
pub struct CaptchaVerificationResponse {
    pub success: bool,
    pub challenge_ts: Option<DateTime<Utc>>,
    pub hostname: Option<String>,
    pub error_codes: Option<Vec<String>>,
}

/// reCAPTCHA response from Google
#[derive(Debug, Deserialize)]
struct RecaptchaResponse {
    pub success: bool,
    #[serde(rename = "challenge_ts")]
    pub challenge_ts: Option<String>,
    pub hostname: Option<String>,
    #[serde(rename = "error-codes")]
    pub error_codes: Option<Vec<String>>,
    pub score: Option<f64>,
    pub action: Option<String>,
}

/// hCaptcha response
#[derive(Debug, Deserialize)]
struct HcaptchaResponse {
    pub success: bool,
    #[serde(rename = "challenge_ts")]
    pub challenge_ts: Option<String>,
    pub hostname: Option<String>,
    #[serde(rename = "error-codes")]
    pub error_codes: Option<Vec<String>>,
    pub credit: Option<bool>,
}

/// CAPTCHA configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptchaConfig {
    pub provider: String, // "recaptcha", "hcaptcha", or "custom"
    #[allow(dead_code)] // Used in create_challenge and get_public_config methods
    pub site_key: String,
    pub secret_key: String,
    pub verify_url: String,
    pub min_score: f64, // For reCAPTCHA v3
    pub enabled: bool,
    #[allow(dead_code)] // Used in create_challenge method for expires_at calculation
    pub challenge_lifetime_minutes: i64,
}

impl CaptchaConfig {
    pub fn from_env() -> Self {
        let provider =
            std::env::var("CAPTCHA_PROVIDER").unwrap_or_else(|_| "recaptcha".to_string());
        let verify_url = match provider.as_str() {
            "hcaptcha" => "https://hcaptcha.com/siteverify".to_string(),
            "recaptcha" => "https://www.google.com/recaptcha/api/siteverify".to_string(),
            _ => std::env::var("CAPTCHA_VERIFY_URL")
                .unwrap_or_else(|_| "https://www.google.com/recaptcha/api/siteverify".to_string()),
        };

        Self {
            provider,
            site_key: std::env::var("CAPTCHA_SITE_KEY").unwrap_or_default(),
            secret_key: std::env::var("CAPTCHA_SECRET_KEY").unwrap_or_default(),
            verify_url,
            min_score: std::env::var("CAPTCHA_MIN_SCORE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.5),
            enabled: std::env::var("CAPTCHA_ENABLED")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(true),
            challenge_lifetime_minutes: std::env::var("CAPTCHA_CHALLENGE_LIFETIME_MINUTES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10),
        }
    }
}

/// Service for managing CAPTCHA challenges and verification
pub struct CaptchaService<C: Connection> {
    captcha_repo: CaptchaRepository<C>,
    config: CaptchaConfig,
    http_client: reqwest::Client,
}

impl<C: Connection> CaptchaService<C> {
    pub fn new(captcha_repo: CaptchaRepository<C>, config: CaptchaConfig) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self { captcha_repo, config, http_client }
    }

    /// Create a new CAPTCHA challenge
    pub async fn create_challenge(
        &self,
        ip_address: Option<String>,
        user_agent: Option<String>,
        session_id: Option<String>,
    ) -> Result<CaptchaChallenge, MatrixAuthError> {
        if !self.config.enabled {
            return Err(MatrixAuthError::InvalidXMatrixFormat);
        }

        let challenge_id = format!("captcha_{}", Uuid::new_v4());
        let now = Utc::now();
        let expires_at = now + Duration::minutes(self.config.challenge_lifetime_minutes);

        let challenge = CaptchaChallenge {
            challenge_id: challenge_id.clone(),
            challenge_type: self.config.provider.clone(),
            site_key: Some(self.config.site_key.clone()),
            question: None, // Not used for external providers like reCAPTCHA
            answer: None,   // Not used for external providers like reCAPTCHA
            created_at: now,
            expires_at,
            used: false,
            verified: false,
            user_id: None, // Will be set when user attempts the challenge
            client_ip: ip_address.clone(),
            user_agent,
            session_id,
        };

        let stored_challenge =
            self.captcha_repo.create_challenge(&challenge).await.map_err(|e| {
                MatrixAuthError::DatabaseError(format!("Failed to store CAPTCHA challenge: {}", e))
            })?;

        info!("Created CAPTCHA challenge: {} for IP: {:?}", challenge_id, ip_address);
        Ok(stored_challenge)
    }

    /// Verify CAPTCHA response
    pub async fn verify_captcha(
        &self,
        request: CaptchaVerificationRequest,
    ) -> Result<CaptchaVerificationResponse, MatrixAuthError> {
        if !self.config.enabled {
            return Ok(CaptchaVerificationResponse {
                success: true,
                challenge_ts: Some(Utc::now()),
                hostname: None,
                error_codes: None,
            });
        }

        // Get and validate challenge
        let mut challenge = self
            .captcha_repo
            .get_challenge(&request.challenge_id)
            .await
            .map_err(|e| {
                MatrixAuthError::DatabaseError(format!("Failed to get CAPTCHA challenge: {}", e))
            })?
            .ok_or(MatrixAuthError::UnknownToken)?;

        // Check if challenge is expired
        if Utc::now() > challenge.expires_at {
            return Ok(CaptchaVerificationResponse {
                success: false,
                challenge_ts: None,
                hostname: None,
                error_codes: Some(vec!["challenge-expired".to_string()]),
            });
        }

        // Check if challenge is already used
        if challenge.used {
            return Ok(CaptchaVerificationResponse {
                success: false,
                challenge_ts: None,
                hostname: None,
                error_codes: Some(vec!["challenge-already-used".to_string()]),
            });
        }

        // Verify with CAPTCHA provider
        let verification_result = match self.config.provider.as_str() {
            "recaptcha" => {
                self.verify_recaptcha(&request.response, request.remote_ip.as_deref())
                    .await?
            },
            "hcaptcha" => {
                self.verify_hcaptcha(&request.response, request.remote_ip.as_deref())
                    .await?
            },
            _ => {
                warn!("Unknown CAPTCHA provider: {}", self.config.provider);
                return Err(MatrixAuthError::InvalidXMatrixFormat);
            },
        };

        // Mark challenge as used
        challenge.used = true;
        challenge.verified = verification_result.success;
        self.captcha_repo.create_challenge(&challenge).await.map_err(|e| {
            MatrixAuthError::DatabaseError(format!("Failed to update CAPTCHA challenge: {}", e))
        })?;

        info!(
            "CAPTCHA verification result for challenge {}: success={}",
            request.challenge_id, verification_result.success
        );

        Ok(verification_result)
    }

    /// Check if CAPTCHA is required for an operation
    pub async fn is_captcha_required(
        &self,
        ip_address: &str,
        operation: &str,
    ) -> Result<bool, MatrixAuthError> {
        if !self.config.enabled {
            return Ok(false);
        }

        // Check rate limiting and suspicious activity for this IP
        let suspicious_activity = self.check_suspicious_activity(ip_address, operation).await?;

        Ok(suspicious_activity)
    }

    /// Get CAPTCHA configuration for client
    pub fn get_public_config(&self) -> HashMap<String, serde_json::Value> {
        let mut config = HashMap::new();

        if self.config.enabled {
            config.insert("enabled".to_string(), serde_json::Value::Bool(true));
            config.insert(
                "provider".to_string(),
                serde_json::Value::String(self.config.provider.clone()),
            );
            config.insert(
                "site_key".to_string(),
                serde_json::Value::String(self.config.site_key.clone()),
            );
        } else {
            config.insert("enabled".to_string(), serde_json::Value::Bool(false));
        }

        config
    }

    /// Clean up expired CAPTCHA challenges
    pub async fn cleanup_expired_challenges(&self) -> Result<u64, MatrixAuthError> {
        let cutoff = Utc::now();
        let count = self.captcha_repo.cleanup_expired_challenges(cutoff).await.map_err(|e| {
            MatrixAuthError::DatabaseError(format!("Failed to cleanup CAPTCHA challenges: {}", e))
        })?;

        if count > 0 {
            info!("Cleaned up {} expired CAPTCHA challenges", count);
        }

        Ok(count)
    }

    /// Verify reCAPTCHA response with Google
    async fn verify_recaptcha(
        &self,
        response: &str,
        remote_ip: Option<&str>,
    ) -> Result<CaptchaVerificationResponse, MatrixAuthError> {
        let mut params = vec![
            ("secret", self.config.secret_key.as_str()),
            ("response", response),
        ];

        if let Some(ip) = remote_ip {
            params.push(("remoteip", ip));
        }

        let response = self
            .http_client
            .post(&self.config.verify_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| {
                MatrixAuthError::DatabaseError(format!("reCAPTCHA verification failed: {}", e))
            })?;

        let recaptcha_response: RecaptchaResponse = response.json().await.map_err(|e| {
            MatrixAuthError::DatabaseError(format!("Failed to parse reCAPTCHA response: {}", e))
        })?;

        // For reCAPTCHA v3, check the score and action
        let mut success = recaptcha_response.success
            && recaptcha_response.score.is_none_or(|score| score >= self.config.min_score);

        // Validate action field for reCAPTCHA v3 (should match expected action like "login" or "register")
        if let Some(action) = &recaptcha_response.action {
            if !matches!(action.as_str(), "login" | "register" | "submit" | "matrix_auth") {
                warn!("reCAPTCHA action validation failed: unexpected action '{}'", action);
                success = false;
            } else {
                debug!("reCAPTCHA action validated: {}", action);
            }
        }

        let challenge_ts = recaptcha_response
            .challenge_ts
            .and_then(|ts| DateTime::parse_from_rfc3339(&ts).ok())
            .map(|dt| dt.with_timezone(&Utc));

        Ok(CaptchaVerificationResponse {
            success,
            challenge_ts,
            hostname: recaptcha_response.hostname,
            error_codes: recaptcha_response.error_codes,
        })
    }

    /// Verify hCaptcha response
    async fn verify_hcaptcha(
        &self,
        response: &str,
        remote_ip: Option<&str>,
    ) -> Result<CaptchaVerificationResponse, MatrixAuthError> {
        let mut params = vec![
            ("secret", self.config.secret_key.as_str()),
            ("response", response),
        ];

        if let Some(ip) = remote_ip {
            params.push(("remoteip", ip));
        }

        let response = self
            .http_client
            .post(&self.config.verify_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| {
                MatrixAuthError::DatabaseError(format!("hCaptcha verification failed: {}", e))
            })?;

        let hcaptcha_response: HcaptchaResponse = response.json().await.map_err(|e| {
            MatrixAuthError::DatabaseError(format!("Failed to parse hCaptcha response: {}", e))
        })?;

        let challenge_ts = hcaptcha_response
            .challenge_ts
            .and_then(|ts| DateTime::parse_from_rfc3339(&ts).ok())
            .map(|dt| dt.with_timezone(&Utc));

        // Check hCaptcha credit field - indicates whether solution was free or paid
        let success = hcaptcha_response.success;
        if let Some(credit) = hcaptcha_response.credit {
            if credit {
                debug!("hCaptcha solved with credit (paid solution)");
            } else {
                debug!("hCaptcha solved without credit (free solution)");
            }
            // Note: Both credited and non-credited solutions are valid for hCaptcha
        }

        Ok(CaptchaVerificationResponse {
            success,
            challenge_ts,
            hostname: hcaptcha_response.hostname,
            error_codes: hcaptcha_response.error_codes,
        })
    }

    /// Check for suspicious activity from IP address
    async fn check_suspicious_activity(
        &self,
        ip_address: &str,
        operation: &str,
    ) -> Result<bool, MatrixAuthError> {
        let window = chrono::Duration::hours(1);
        let attempt_count = self
            .captcha_repo
            .get_ip_challenge_count(ip_address, window)
            .await
            .map_err(|e| {
                MatrixAuthError::DatabaseError(format!(
                    "Failed to check suspicious activity for operation '{}': {}",
                    operation, e
                ))
            })?;

        // Operation-specific thresholds for suspicious activity detection
        let threshold = match operation {
            "login" => 5,          // Allow more login attempts
            "register" => 3,       // Stricter on registration
            "password_reset" => 2, // Very strict on password resets
            "room_join" => 10,     // More lenient on room operations
            _ => 3,                // Default threshold
        };

        info!(
            "Checking suspicious activity for operation '{}': {} attempts in last hour (threshold: {})",
            operation, attempt_count, threshold
        );

        // Require CAPTCHA if attempts exceed operation-specific threshold
        Ok(attempt_count > threshold)
    }

    /// Record rate limit violation for suspicious activity tracking
    pub async fn record_rate_limit_violation(
        &self,
        ip_address: &str,
        operation: &str,
    ) -> Result<(), MatrixAuthError> {
        // Create a challenge record to track the violation/failed attempt
        let challenge = CaptchaChallenge {
            challenge_id: format!("violation_{}", Uuid::new_v4()),
            challenge_type: "rate_limit_violation".to_string(),
            site_key: Some("".to_string()),
            question: None,
            answer: None,
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::hours(1),
            used: true,
            verified: false,
            user_id: None,
            client_ip: Some(ip_address.to_string()),
            user_agent: None,
            session_id: Some(operation.to_string()),
        };

        self.captcha_repo.create_challenge(&challenge).await.map_err(|e| {
            MatrixAuthError::DatabaseError(format!("Failed to record rate limit violation: {}", e))
        })?;

        Ok(())
    }

    /// Get CAPTCHA statistics for monitoring and rate limit analysis
    #[allow(dead_code)] // Used in rate limit middleware but compiler doesn't detect it
    pub async fn get_captcha_stats(&self) -> Result<CaptchaStats, MatrixAuthError> {
        self.captcha_repo.get_captcha_stats().await.map_err(|e| {
            MatrixAuthError::DatabaseError(format!("Failed to get CAPTCHA stats: {}", e))
        })
    }
}
