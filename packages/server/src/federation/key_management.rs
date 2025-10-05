use chrono::{DateTime, Utc};
use std::sync::Arc;
use surrealdb::engine::any::Any;
use tokio::time::{Duration, interval};
use tracing::{error, info, warn};

use crate::AppState;
use matryx_surrealdb::repository::{InfrastructureService, SigningKey};

/// Background service for automatic server key lifecycle management
///
/// This service implements Matrix specification requirements for key refresh:
/// - Monitors key expiration times
/// - Automatically generates new keys before expiration
/// - Maintains continuous service without interruption
pub struct KeyManagementService {
    app_state: Arc<AppState>,
    check_interval_hours: u64,
    refresh_threshold_days: i64,
}

impl KeyManagementService {
    /// Create a new key management service
    pub fn new(app_state: Arc<AppState>) -> Self {
        Self {
            app_state,
            check_interval_hours: 6,    // Check every 6 hours
            refresh_threshold_days: 30, // Refresh keys 30 days before expiration
        }
    }

    /// Start the background key management service
    ///
    /// This spawns a background task that periodically checks for keys
    /// approaching expiration and generates new ones as needed
    pub fn start(&self) {
        let service = self.clone();

        tokio::spawn(async move {
            info!("Starting key management background service");
            service.run().await;
        });
    }

    /// Main service loop
    async fn run(&self) {
        let mut interval = interval(Duration::from_secs(self.check_interval_hours * 3600));

        loop {
            interval.tick().await;

            if let Err(e) = self.check_and_refresh_keys().await {
                error!("Key management service error: {}", e);
            }
        }
    }

    /// Check for keys approaching expiration and refresh if needed
    async fn check_and_refresh_keys(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Running key expiration check");

        let infrastructure_service = self.create_infrastructure_service().await;
        let server_name = &self.app_state.homeserver_name;

        // Get current server keys
        let server_keys_response =
            infrastructure_service.get_server_keys(server_name, None).await?;

        let now = Utc::now();
        let refresh_threshold = now + chrono::Duration::days(self.refresh_threshold_days);

        for server_keys in &server_keys_response.server_keys {
            let valid_until = DateTime::from_timestamp(server_keys.valid_until_ts / 1000, 0)
                .ok_or_else(|| {
                    format!("Invalid timestamp in server keys: {}", server_keys.valid_until_ts)
                })?;

            if valid_until <= refresh_threshold {
                warn!(
                    "Server key for {} expires at {}, refreshing now (threshold: {})",
                    server_name, valid_until, refresh_threshold
                );

                // Generate new key before expiration
                if let Err(e) =
                    self.generate_new_signing_key(&infrastructure_service, server_name).await
                {
                    error!("Failed to generate new signing key for {}: {}", server_name, e);
                } else {
                    info!("Successfully generated new signing key for {}", server_name);
                }
            } else {
                info!(
                    "Server key for {} valid until {} (safe until {})",
                    server_name, valid_until, refresh_threshold
                );
            }
        }

        Ok(())
    }

    /// Generate a new signing key for the server
    async fn generate_new_signing_key(
        &self,
        infrastructure_service: &InfrastructureService<Any>,
        server_name: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use base64::{Engine, engine::general_purpose};
        use ed25519_dalek::SigningKey as Ed25519SigningKey;

        // Generate new Ed25519 keypair
        let mut secret_bytes = [0u8; 32];
        getrandom::fill(&mut secret_bytes).expect("Failed to generate random bytes");
        let signing_key = Ed25519SigningKey::from_bytes(&secret_bytes);
        let verifying_key = signing_key.verifying_key();

        // Extract raw bytes and encode as base64
        let private_key_bytes = signing_key.to_bytes();
        let public_key_bytes = verifying_key.to_bytes();
        let private_key_b64 = general_purpose::STANDARD.encode(private_key_bytes);
        let public_key_b64 = general_purpose::STANDARD.encode(public_key_bytes);

        let key_id = "ed25519:auto".to_string();
        let created_at = Utc::now();
        let expires_at = created_at + chrono::Duration::days(365); // 1 year validity

        // Create SigningKey entity
        let signing_key_entity = SigningKey {
            key_id: key_id.clone(),
            server_name: server_name.to_string(),
            signing_key: private_key_b64,
            verify_key: public_key_b64,
            created_at,
            expires_at: Some(expires_at),
        };

        // Store the new key
        infrastructure_service
            .store_signing_key(server_name, &key_id, &signing_key_entity)
            .await?;

        info!(
            "Generated new signing key {} for server {} (expires: {})",
            key_id, server_name, expires_at
        );

        Ok(())
    }

    /// Create infrastructure service instance
    async fn create_infrastructure_service(&self) -> InfrastructureService<Any> {
        let websocket_repo =
            matryx_surrealdb::repository::WebSocketRepository::new(self.app_state.db.clone());
        let transaction_repo =
            matryx_surrealdb::repository::TransactionRepository::new(self.app_state.db.clone());
        let key_server_repo =
            matryx_surrealdb::repository::KeyServerRepository::new(self.app_state.db.clone());
        let registration_repo =
            matryx_surrealdb::repository::RegistrationRepository::new(self.app_state.db.clone());
        let directory_repo =
            matryx_surrealdb::repository::DirectoryRepository::new(self.app_state.db.clone());
        let device_repo =
            matryx_surrealdb::repository::DeviceRepository::new(self.app_state.db.clone());
        let auth_repo =
            matryx_surrealdb::repository::AuthRepository::new(self.app_state.db.clone());

        InfrastructureService::new(
            websocket_repo,
            transaction_repo,
            key_server_repo,
            registration_repo,
            directory_repo,
            device_repo,
            auth_repo,
        )
    }
}

impl Clone for KeyManagementService {
    fn clone(&self) -> Self {
        Self {
            app_state: self.app_state.clone(),
            check_interval_hours: self.check_interval_hours,
            refresh_threshold_days: self.refresh_threshold_days,
        }
    }
}
