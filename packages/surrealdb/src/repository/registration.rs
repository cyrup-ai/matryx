use crate::repository::error::RepositoryError;
use chrono::{DateTime, Utc};
use matryx_entity::types::Device;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use surrealdb::{Connection, Surreal};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationResult {
    pub user_id: String,
    pub device_id: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in_ms: Option<i64>,
    pub home_server: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthFlow {
    pub flow_type: String,
    pub stages: Vec<String>,
    pub params: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationToken {
    pub token: String,
    pub uses_allowed: Option<i32>,
    pub uses_remaining: Option<i32>,
    pub pending: bool,
    pub completed: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationAttempt {
    pub ip_address: String,
    pub timestamp: DateTime<Utc>,
    pub success: bool,
    pub user_agent: Option<String>,
}

pub struct RegistrationRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> RegistrationRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    pub async fn check_username_availability(
        &self,
        username: &str,
    ) -> Result<bool, RepositoryError> {
        let query = "SELECT VALUE count() FROM user WHERE id = $user_id";
        let user_id = format!("@{}:localhost", username); // Construct full user ID
        let mut result = self.db.query(query).bind(("user_id", user_id)).await?;
        let count: Option<i64> = result.take(0)?;

        Ok(count.unwrap_or(0) == 0)
    }

    pub async fn register_user(
        &self,
        user_id: &str,
        password_hash: &str,
        device_id: &str,
        initial_device_display_name: Option<&str>,
    ) -> Result<RegistrationResult, RepositoryError> {
        let now = Utc::now();

        // Create user record
        let user_data = serde_json::json!({
            "id": user_id,
            "password_hash": password_hash,
            "created_at": now,
            "is_guest": false,
            "is_deactivated": false
        });

        let _user: Option<serde_json::Value> =
            self.db.create(("user", user_id)).content(user_data).await?;

        // Generate access token
        let access_token = format!("syt_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));

        // Create device
        let device = Device {
            device_id: device_id.to_string(),
            user_id: user_id.to_string(),
            display_name: initial_device_display_name.map(|s| s.to_string()),
            last_seen_ip: None,
            last_seen_ts: Some(now.timestamp()),
            created_at: now,
            hidden: Some(false),
            device_keys: None,
            one_time_keys: None,
            fallback_keys: None,
            user_agent: None,
            initial_device_display_name: initial_device_display_name.map(|s| s.to_string()),
        };

        let _device: Option<Device> = self.db.create(("device", device_id)).content(device).await?;

        // Create access token record
        let token_data = serde_json::json!({
            "token": access_token,
            "user_id": user_id,
            "device_id": device_id,
            "created_at": now,
            "last_seen": now
        });

        let _token: Option<serde_json::Value> =
            self.db.create(("access_token", &access_token)).content(token_data).await?;

        Ok(RegistrationResult {
            user_id: user_id.to_string(),
            device_id: device_id.to_string(),
            access_token,
            refresh_token: None,
            expires_in_ms: None,
            home_server: "localhost".to_string(),
        })
    }

    pub async fn create_initial_device(
        &self,
        user_id: &str,
        device_id: &str,
        display_name: Option<&str>,
        access_token: &str,
    ) -> Result<Device, RepositoryError> {
        // Validate access token corresponds to the user
        let session_query = "SELECT user_id FROM session WHERE access_token = $token AND is_active = true LIMIT 1";
        let mut result = self.db.query(session_query).bind(("token", access_token.to_string())).await?;
        let sessions: Vec<serde_json::Value> = result.take(0)?;
        
        if sessions.is_empty() {
            return Err(RepositoryError::Unauthorized { 
                reason: "Invalid access token".to_string() 
            });
        }
        
        let session_user_id = sessions[0]["user_id"].as_str()
            .ok_or_else(|| RepositoryError::Validation { 
                field: "user_id".to_string(),
                message: "Session user_id not found".to_string() 
            })?;
            
        if session_user_id != user_id {
            return Err(RepositoryError::Unauthorized { 
                reason: "Access token does not match user".to_string() 
            });
        }

        let now = Utc::now();

        let device = Device {
            device_id: device_id.to_string(),
            user_id: user_id.to_string(),
            display_name: display_name.map(|s| s.to_string()),
            last_seen_ip: None,
            last_seen_ts: Some(now.timestamp()),
            created_at: now,
            hidden: Some(false),
            device_keys: None,
            one_time_keys: None,
            fallback_keys: None,
            user_agent: None,
            initial_device_display_name: display_name.map(|s| s.to_string()),
        };

        // Create the device
        let created: Option<Device> =
            self.db.create(("device", device_id)).content(device.clone()).await?;

        let device_result = created.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to create device"))
        })?;

        // Create initial session for the device with the access token
        let session = matryx_entity::types::Session {
            session_id: uuid::Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            device_id: device_id.to_string(),
            access_token: access_token.to_string(),
            refresh_token: None,
            created_at: now,
            expires_at: Some(now + chrono::Duration::days(365)), // 1 year expiration
            last_seen: Some(now),
            last_used_at: Some(now),
            last_used_ip: None,
            user_agent: None,
            is_active: true,
            valid: true,
            puppets_user_id: None,
            is_guest: false,
        };

        // Store the session
        let _: Option<matryx_entity::types::Session> = self
            .db
            .create(("session", &session.session_id))
            .content(session)
            .await?;

        Ok(device_result)
    }

    pub async fn validate_registration_token(&self, token: &str) -> Result<bool, RepositoryError> {
        let query = "SELECT * FROM registration_token WHERE token = $token AND pending = false AND completed = false AND (expires_at IS NONE OR expires_at > $now) AND (uses_remaining IS NONE OR uses_remaining > 0) LIMIT 1";
        let mut result = self
            .db
            .query(query)
            .bind(("token", token.to_string()))
            .bind(("now", Utc::now()))
            .await?;

        let token_record: Option<RegistrationToken> = result.take(0)?;
        Ok(token_record.is_some())
    }

    pub async fn consume_registration_token(&self, token: &str) -> Result<(), RepositoryError> {
        let query = "UPDATE registration_token SET uses_remaining = uses_remaining - 1, completed = (uses_remaining <= 1) WHERE token = $token";
        let mut _result = self.db.query(query).bind(("token", token.to_string())).await?;

        Ok(())
    }

    pub async fn check_registration_rate_limit(
        &self,
        ip_address: &str,
    ) -> Result<bool, RepositoryError> {
        let cutoff = Utc::now() - chrono::Duration::hours(1);
        let query = "SELECT VALUE count() FROM registration_attempt WHERE ip_address = $ip_address AND timestamp > $cutoff";
        let mut result = self
            .db
            .query(query)
            .bind(("ip_address", ip_address.to_string()))
            .bind(("cutoff", cutoff))
            .await?;

        let count: Option<i64> = result.take(0)?;
        let attempts = count.unwrap_or(0);

        // Allow up to 5 registration attempts per hour per IP
        Ok(attempts < 5)
    }

    pub async fn record_registration_attempt(
        &self,
        ip_address: &str,
        success: bool,
    ) -> Result<(), RepositoryError> {
        let attempt = RegistrationAttempt {
            ip_address: ip_address.to_string(),
            timestamp: Utc::now(),
            success,
            user_agent: None,
        };

        let _created: Option<RegistrationAttempt> =
            self.db.create("registration_attempt").content(attempt).await?;

        Ok(())
    }

    pub async fn get_registration_flows(&self) -> Result<Vec<AuthFlow>, RepositoryError> {
        // Return default registration flows
        let flows = vec![
            AuthFlow {
                flow_type: "m.login.password".to_string(),
                stages: vec!["m.login.password".to_string()],
                params: HashMap::new(),
            },
            AuthFlow {
                flow_type: "m.login.registration_token".to_string(),
                stages: vec!["m.login.registration_token".to_string()],
                params: HashMap::new(),
            },
        ];

        Ok(flows)
    }
}
