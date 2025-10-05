use axum::{Json, extract::State, http::StatusCode};
use base64::{Engine, engine::general_purpose};
use serde_json::{Value, json};
use std::env;
use tracing::{error, info};

use crate::AppState;
use matryx_entity::utils::canonical_json;
use matryx_surrealdb::repository::{InfrastructureService, SigningKey};

#[derive(serde::Deserialize)]
struct SigningKeyRecord {
    key_id: String,
    public_key: String,
    private_key: String,
    created_at: chrono::DateTime<chrono::Utc>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// GET /_matrix/key/v2/server
///
/// Returns the homeserver's published signing keys for federation using KeyServerRepository.
/// Other servers use these keys to verify signatures on events and requests.
pub async fn get(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    // Get server name from environment with proper error handling
    let server_name = env::var("HOMESERVER_NAME").map_err(|_| {
        error!("HOMESERVER_NAME environment variable not set");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Create InfrastructureService instance
    let infrastructure_service = create_infrastructure_service(&state).await;

    // Get or generate Ed25519 signing keys using repository
    let (verify_keys, old_verify_keys, signatures, key_record) =
        get_or_generate_signing_keys(&infrastructure_service, &server_name)
            .await
            .map_err(|e| {
                error!("Failed to get signing keys: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

    // Log key age for Matrix federation key management
    let key_age_days = (chrono::Utc::now() - key_record.created_at).num_days();
    info!("Serving signing key {} created {} days ago", key_record.key_id, key_age_days);

    // Use key expiry time if available, otherwise default to 7 days
    // Ensure minimum 1-hour response lifetime per Matrix spec to avoid repeated requests
    let now_ms = chrono::Utc::now().timestamp_millis();
    let one_hour_from_now = now_ms + (60 * 60 * 1000); // 1 hour in milliseconds
    let default_valid_until = now_ms + (7 * 24 * 60 * 60 * 1000); // 7 days from now

    let proposed_valid_until = if let Some(expires_at) = key_record.expires_at {
        expires_at.timestamp_millis()
    } else {
        default_valid_until
    };

    // Enforce minimum 1-hour response lifetime
    let valid_until_ms = std::cmp::max(proposed_valid_until, one_hour_from_now);

    Ok(Json(json!({
        "server_name": server_name,
        "verify_keys": verify_keys,
        "old_verify_keys": old_verify_keys,
        "signatures": signatures,
        "valid_until_ts": valid_until_ms
    })))
}

async fn create_infrastructure_service(
    state: &AppState,
) -> InfrastructureService<surrealdb::engine::any::Any> {
    let websocket_repo = matryx_surrealdb::repository::WebSocketRepository::new(state.db.clone());
    let transaction_repo =
        matryx_surrealdb::repository::TransactionRepository::new(state.db.clone());
    let key_server_repo = matryx_surrealdb::repository::KeyServerRepository::new(state.db.clone());
    let registration_repo =
        matryx_surrealdb::repository::RegistrationRepository::new(state.db.clone());
    let directory_repo = matryx_surrealdb::repository::DirectoryRepository::new(state.db.clone());
    let device_repo = matryx_surrealdb::repository::DeviceRepository::new(state.db.clone());
    let auth_repo = matryx_surrealdb::repository::AuthRepository::new(state.db.clone());

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

/// Get existing signing keys from repository or generate new ones
async fn get_or_generate_signing_keys(
    infrastructure_service: &InfrastructureService<surrealdb::engine::any::Any>,
    server_name: &str,
) -> Result<(Value, Value, Value, SigningKeyRecord), Box<dyn std::error::Error + Send + Sync>> {
    // Start by trying to get the private signing key directly (correct approach)
    let key_id = "ed25519:auto";

    let current_key = match infrastructure_service.get_signing_key(server_name, key_id).await {
        Ok(Some(signing_key_data)) => {
            // We have a stored signing key - use it to build the response
            info!("Found existing signing key {} for server {}", key_id, server_name);

            SigningKeyRecord {
                key_id: key_id.to_string(),
                public_key: signing_key_data.verify_key.clone(),
                private_key: signing_key_data.signing_key,
                created_at: signing_key_data.created_at,
                expires_at: signing_key_data.expires_at,
            }
        },
        Ok(None) | Err(_) => {
            // No signing key found, generate new one
            info!(
                "No signing key found for server {}, generating new Ed25519 key pair",
                server_name
            );
            generate_ed25519_keypair(infrastructure_service, server_name).await?
        },
    };

    // Build verify_keys JSON
    let mut verify_keys = serde_json::Map::new();
    verify_keys.insert(
        current_key.key_id.clone(),
        json!({
            "key": current_key.public_key
        }),
    );

    // Build old_verify_keys JSON (empty for now)
    let old_verify_keys = json!({});

    // Build signatures JSON
    let canonical_json = build_canonical_server_json(server_name, &verify_keys)?;
    let signature = sign_canonical_json(&canonical_json, &current_key.private_key)?;

    let mut server_signatures = serde_json::Map::new();
    server_signatures.insert(current_key.key_id.clone(), json!(signature));

    let mut signatures = serde_json::Map::new();
    signatures.insert(server_name.to_string(), json!(server_signatures));

    Ok((json!(verify_keys), old_verify_keys, json!(signatures), current_key))
}

/// Generate new Ed25519 keypair and store using repository
async fn generate_ed25519_keypair(
    infrastructure_service: &InfrastructureService<surrealdb::engine::any::Any>,
    server_name: &str,
) -> Result<SigningKeyRecord, Box<dyn std::error::Error + Send + Sync>> {
    use ed25519_dalek::SigningKey as Ed25519SigningKey;

    // Generate proper Ed25519 keypair using cryptographically secure random number generator
    let mut secret_bytes = [0u8; 32];
    getrandom::fill(&mut secret_bytes).expect("Failed to generate random bytes");
    let signing_key = Ed25519SigningKey::from_bytes(&secret_bytes);
    let verifying_key = signing_key.verifying_key();

    // Extract raw bytes
    let private_key_bytes = signing_key.to_bytes();
    let public_key_bytes = verifying_key.to_bytes();

    // Encode as base64
    let private_key_b64 = general_purpose::STANDARD.encode(private_key_bytes);
    let public_key_b64 = general_purpose::STANDARD.encode(public_key_bytes);

    let key_id = "ed25519:auto".to_string();
    let created_at = chrono::Utc::now();
    let expires_at = created_at + chrono::Duration::days(365); // 1 year validity

    // Create SigningKey struct for repository
    let signing_key_entity = SigningKey {
        key_id: key_id.clone(),
        server_name: server_name.to_string(),
        signing_key: private_key_b64.clone(),
        verify_key: public_key_b64.clone(),
        created_at,
        expires_at: Some(expires_at),
    };

    // Store using InfrastructureService
    infrastructure_service
        .store_signing_key(server_name, &key_id, &signing_key_entity)
        .await
        .map_err(|e| {
            error!("Failed to store signing key: {:?}", e);
            Box::new(std::io::Error::other("Failed to store signing key"))
                as Box<dyn std::error::Error + Send + Sync>
        })?;

    info!(
        "Generated and stored new Ed25519 keypair for server {} with key_id {}",
        server_name, key_id
    );

    Ok(SigningKeyRecord {
        key_id,
        public_key: public_key_b64,
        private_key: private_key_b64,
        created_at,
        expires_at: Some(expires_at),
    })
}

/// Build canonical JSON for server key signing
fn build_canonical_server_json(
    server_name: &str,
    verify_keys: &serde_json::Map<String, Value>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let current_time_ms = chrono::Utc::now().timestamp_millis();
    let one_hour_from_now = current_time_ms + (60 * 60 * 1000); // 1 hour in milliseconds
    let seven_days_from_now = current_time_ms + (7 * 24 * 60 * 60 * 1000); // 7 days from now

    // Ensure minimum 1-hour response lifetime per Matrix spec
    let valid_until_ms = std::cmp::max(seven_days_from_now, one_hour_from_now);

    let server_object = json!({
        "server_name": server_name,
        "verify_keys": verify_keys,
        "old_verify_keys": {},
        "valid_until_ts": valid_until_ms
    });

    Ok(canonical_json(&server_object)?)
}

/// Sign canonical JSON with Ed25519 private key
fn sign_canonical_json(
    canonical_json: &str,
    private_key_b64: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    use ed25519_dalek::{Signer, SigningKey};

    // Decode the base64 private key
    let private_key_bytes = general_purpose::STANDARD.decode(private_key_b64)?;

    // Validate private key length
    if private_key_bytes.len() != 32 {
        return Err("Invalid private key length".into());
    }

    // Convert to array and create SigningKey
    let private_key_array: [u8; 32] = private_key_bytes
        .try_into()
        .map_err(|_| "Failed to convert private key bytes to array")?;
    let signing_key = SigningKey::from_bytes(&private_key_array);

    // Sign the canonical JSON
    let signature = signing_key.sign(canonical_json.as_bytes());

    // Encode signature as base64
    let signature_b64 = general_purpose::STANDARD.encode(signature.to_bytes());

    Ok(signature_b64)
}
