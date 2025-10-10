use crate::AppState;
use matryx_surrealdb::repository::InfrastructureService;

/// Create InfrastructureService instance from AppState
///
/// This helper creates the service with all necessary repositories for key server operations.
pub(super) async fn create_infrastructure_service(
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

/// Sign canonical JSON with Ed25519 private key
///
/// Takes a canonical JSON string and a base64-encoded private key,
/// returns a base64-encoded signature.
pub(super) fn sign_canonical_json(
    canonical_json: &str,
    private_key_b64: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    use base64::{Engine, engine::general_purpose};
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
