//! Module contains intentional library code not yet fully integrated
#![allow(dead_code)]

use aes::Aes256;
use base64::Engine;
use cbc::cipher::{BlockEncryptMut, KeyIvInit};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};

use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
use surrealdb::{Surreal, engine::any::Any};
use vodozemac::megolm::GroupSession;
use vodozemac::olm::{Account, Session, SessionConfig};
use vodozemac::{Curve25519PublicKey, Curve25519SecretKey, Ed25519PublicKey};

use matryx_surrealdb::repository::crypto::{DeviceKey, FallbackKey, OneTimeKey, Signature};
use matryx_surrealdb::repository::key_backup::{BackupVersion, EncryptedRoomKey};
use matryx_surrealdb::repository::{CryptoRepository, KeyBackupRepository};

/// High-level crypto provider using vodozemac
pub struct MatryxCryptoProvider {
    // Vodozemac handles all low-level crypto
    crypto_repo: CryptoRepository,
    key_backup_repo: KeyBackupRepository,
}

impl MatryxCryptoProvider {
    pub fn new(db: Surreal<Any>) -> Self {
        Self {
            crypto_repo: CryptoRepository::new(db.clone()),
            key_backup_repo: KeyBackupRepository::new(db),
        }
    }

    /// Store device keys using repository
    pub async fn store_device_keys(
        &self,
        user_id: &str,
        device_id: &str,
        device_keys: &DeviceKey,
    ) -> Result<(), CryptoError> {
        self.crypto_repo
            .store_device_key(user_id, device_id, device_keys)
            .await
            .map_err(|e| CryptoError::MissingField(e.to_string()))
    }

    /// Get device keys using repository
    pub async fn get_device_keys(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<Option<DeviceKey>, CryptoError> {
        self.crypto_repo
            .get_device_key(user_id, device_id)
            .await
            .map_err(|e| CryptoError::MissingField(e.to_string()))
    }

    /// Store one-time keys using repository
    pub async fn store_one_time_keys(
        &self,
        user_id: &str,
        device_id: &str,
        keys: &HashMap<String, OneTimeKey>,
    ) -> Result<(), CryptoError> {
        for (key_id, key) in keys {
            self.crypto_repo
                .store_one_time_key(user_id, device_id, key_id, key)
                .await
                .map_err(|e| CryptoError::MissingField(e.to_string()))?;
        }
        Ok(())
    }

    /// Claim one-time key using repository
    pub async fn claim_one_time_key(
        &self,
        user_id: &str,
        device_id: &str,
        algorithm: &str,
    ) -> Result<Option<OneTimeKey>, CryptoError> {
        self.crypto_repo
            .claim_one_time_key(user_id, device_id, algorithm)
            .await
            .map_err(|e| CryptoError::MissingField(e.to_string()))
    }

    /// Store fallback key using repository
    pub async fn store_fallback_key(
        &self,
        user_id: &str,
        device_id: &str,
        key: &FallbackKey,
    ) -> Result<(), CryptoError> {
        self.crypto_repo
            .store_fallback_key(user_id, device_id, key)
            .await
            .map_err(|e| CryptoError::MissingField(e.to_string()))
    }

    /// Get one-time key counts using repository
    pub async fn get_one_time_key_counts(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<HashMap<String, u32>, CryptoError> {
        self.crypto_repo
            .get_one_time_key_count(user_id, device_id)
            .await
            .map_err(|e| CryptoError::MissingField(e.to_string()))
    }

    /// Create key backup version using repository
    pub async fn create_key_backup(
        &self,
        user_id: &str,
        algorithm: &str,
        auth_data: &serde_json::Value,
    ) -> Result<String, CryptoError> {
        self.key_backup_repo
            .create_backup_version(user_id, algorithm, auth_data)
            .await
            .map_err(|e| CryptoError::MissingField(e.to_string()))
    }

    /// Store room key backup using repository
    pub async fn store_room_key_backup(
        &self,
        user_id: &str,
        version: &str,
        room_id: &str,
        session_id: &str,
        key_data: &EncryptedRoomKey,
    ) -> Result<(), CryptoError> {
        self.key_backup_repo
            .store_room_key(user_id, version, room_id, session_id, key_data)
            .await
            .map_err(|e| CryptoError::MissingField(e.to_string()))
    }

    /// Get room key backup using repository
    pub async fn get_room_key_backup(
        &self,
        user_id: &str,
        version: &str,
        room_id: &str,
        session_id: &str,
    ) -> Result<Option<EncryptedRoomKey>, CryptoError> {
        self.key_backup_repo
            .get_room_key(user_id, version, room_id, session_id)
            .await
            .map_err(|e| CryptoError::MissingField(e.to_string()))
    }

    /// Verify device key signatures using vodozemac's built-in validation
    pub async fn verify_device_keys(
        &self,
        device_keys: &matryx_entity::types::DeviceKey,
    ) -> Result<bool, CryptoError> {
        // Extract ed25519 key
        let ed25519_key_str = device_keys
            .keys
            .get("ed25519")
            .ok_or_else(|| CryptoError::MissingField("ed25519 key".to_string()))?;

        let ed25519_key = Ed25519PublicKey::from_base64(ed25519_key_str)
            .map_err(|e| CryptoError::InvalidKey(format!("Invalid ed25519 key: {}", e)))?;

        // Verify self-signature using vodozemac
        let canonical_json = self.canonical_json_device_keys(device_keys)?;
        let signature = device_keys
            .signatures
            .get(&device_keys.user_id)
            .ok_or_else(|| CryptoError::MissingField("user signatures".to_string()))?
            .get(&format!("ed25519:{}", device_keys.device_id))
            .ok_or_else(|| CryptoError::MissingField("device signature".to_string()))?;

        let signature_bytes = base64::engine::general_purpose::STANDARD
            .decode(signature)
            .map_err(|e| CryptoError::InvalidKey(format!("Invalid signature format: {}", e)))?;

        let vodozemac_signature = vodozemac::Ed25519Signature::from_slice(&signature_bytes)
            .map_err(|e| CryptoError::InvalidKey(format!("Invalid signature: {}", e)))?;

        Ok(ed25519_key.verify(canonical_json.as_bytes(), &vodozemac_signature).is_ok())
    }

    /// Verify complete cross-signing chain
    pub async fn verify_cross_signing_chain(
        &self,
        user_id: &str,
        master_key: &matryx_entity::types::CrossSigningKey,
        self_signing_key: &matryx_entity::types::CrossSigningKey,
        device_keys: &matryx_entity::types::DeviceKey,
    ) -> Result<bool, CryptoError> {
        // 1. Verify master key format
        if !master_key.usage.contains(&"master".to_string()) {
            return Ok(false);
        }

        // 2. Verify self-signing key is signed by master key
        let master_ed25519 = self.extract_ed25519_key(&master_key.keys)?;
        if !self
            .verify_key_signature_cross_signing(self_signing_key, &master_ed25519, user_id)
            .await?
        {
            return Ok(false);
        }

        // 3. Verify device keys are signed by self-signing key
        let self_signing_ed25519 = self.extract_ed25519_key(&self_signing_key.keys)?;
        self.verify_key_signature_device(device_keys, &self_signing_ed25519, user_id)
            .await
    }

    /// Create Olm session for E2E messaging
    pub async fn create_olm_session(
        &self,
        our_account: &Account,
        their_curve25519_key: &str,
        their_one_time_key: &str,
    ) -> Result<Session, CryptoError> {
        let their_curve25519 = Curve25519PublicKey::from_base64(their_curve25519_key)
            .map_err(|e| CryptoError::InvalidKey(format!("Invalid curve25519 key: {}", e)))?;
        let their_otk = Curve25519PublicKey::from_base64(their_one_time_key)
            .map_err(|e| CryptoError::InvalidKey(format!("Invalid one-time key: {}", e)))?;

        Ok(our_account.create_outbound_session(
            SessionConfig::version_2(),
            their_curve25519,
            their_otk,
        ))
    }

    /// Create Megolm group session for room encryption
    pub async fn create_group_session(&self) -> Result<GroupSession, CryptoError> {
        Ok(GroupSession::new(Default::default()))
    }

    /// Validate one-time key format
    pub async fn validate_one_time_key(
        &self,
        key_id: &str,
        key_data: &serde_json::Value,
    ) -> Result<bool, CryptoError> {
        // Parse algorithm from key_id (format: algorithm:key_id)
        let parts: Vec<&str> = key_id.split(':').collect();
        if parts.len() != 2 {
            return Ok(false);
        }

        let algorithm = parts[0];
        if algorithm != "signed_curve25519" {
            return Ok(false);
        }

        // Validate key structure
        if let Some(key_obj) = key_data.as_object()
            && key_obj.contains_key("key")
            && key_obj.contains_key("signatures")
        {
            // Try to parse the curve25519 key
            if let Some(key_str) = key_obj.get("key").and_then(|v| v.as_str()) {
                return Ok(Curve25519PublicKey::from_base64(key_str).is_ok());
            }
        }

        Ok(false)
    }

    /// Validate backup auth data
    pub async fn validate_backup_auth_data(
        &self,
        auth_data: &AuthData,
        user_id: &str,
    ) -> Result<bool, CryptoError> {
        // Validate public key format
        let _public_key = Curve25519PublicKey::from_base64(&auth_data.public_key)
            .map_err(|e| CryptoError::InvalidKey(format!("Invalid backup public key: {}", e)))?;

        // Validate signatures exist for user
        if !auth_data.signatures.contains_key(user_id) {
            return Ok(false);
        }

        Ok(true)
    }

    /// Encrypt room key for backup using Matrix backup algorithm m.megolm_backup.v1.curve25519-aes-sha2
    pub async fn encrypt_room_key_for_backup(
        &self,
        room_key_data: &RoomKeyBackupData,
        auth_data: &AuthData,
    ) -> Result<EncryptedKeyData, CryptoError> {
        // Parse the backup public key
        let backup_public_key = Curve25519PublicKey::from_base64(&auth_data.public_key)
            .map_err(|e| CryptoError::InvalidKey(format!("Invalid backup public key: {}", e)))?;

        // Generate ephemeral key pair
        let ephemeral_secret = Curve25519SecretKey::new();
        let ephemeral_public = Curve25519PublicKey::from(&ephemeral_secret);

        // Perform ECDH
        let shared_secret = ephemeral_secret.diffie_hellman(&backup_public_key);

        // Derive keys using HKDF-SHA256
        let hkdf = Hkdf::<Sha256>::new(None, shared_secret.as_bytes());
        let mut aes_key = [0u8; 32]; // AES-256 key
        let mut hmac_key = [0u8; 32]; // HMAC-SHA256 key
        hkdf.expand(b"MATRIX_BACKUP_AES_KEY", &mut aes_key)
            .map_err(|_| CryptoError::InvalidKey("Failed to derive AES key".to_string()))?;
        hkdf.expand(b"MATRIX_BACKUP_MAC_KEY", &mut hmac_key)
            .map_err(|_| CryptoError::InvalidKey("Failed to derive MAC key".to_string()))?;

        // Serialize the session data
        let plaintext = serde_json::to_string(&room_key_data.session_data)?;
        let plaintext_bytes = plaintext.as_bytes();

        // Generate random IV for AES-CBC
        let mut iv = [0u8; 16];
        getrandom::fill(&mut iv)
            .map_err(|e| CryptoError::InvalidKey(format!("Failed to generate random IV for AES-CBC: {}", e)))?;

        // Encrypt with AES-256-CBC
        let cipher = cbc::Encryptor::<Aes256>::new(&aes_key.into(), &iv.into());
        let mut buffer = plaintext_bytes.to_vec();

        // Add PKCS7 padding
        let padding_len = 16 - (buffer.len() % 16);
        buffer.extend(vec![padding_len as u8; padding_len]);

        let buffer_len = buffer.len();
        let _ciphertext = cipher
            .encrypt_padded_mut::<cbc::cipher::block_padding::NoPadding>(&mut buffer, buffer_len)
            .map_err(|_| CryptoError::InvalidKey("Encryption failed".to_string()))?;

        // Combine IV + ciphertext for MAC calculation (buffer now contains the ciphertext)
        let mut mac_input = iv.to_vec();
        mac_input.extend_from_slice(&buffer);

        // Calculate HMAC-SHA256
        let mut hmac = Hmac::<Sha256>::new_from_slice(&hmac_key)
            .map_err(|_| CryptoError::InvalidKey("Failed to create HMAC".to_string()))?;
        hmac.update(&mac_input);
        let mac_result = hmac.finalize().into_bytes();

        // Encode results as base64
        let ciphertext_b64 = base64::engine::general_purpose::STANDARD.encode(&mac_input); // IV + ciphertext
        let ephemeral_key_b64 = ephemeral_public.to_base64();
        let mac_b64 = base64::engine::general_purpose::STANDARD.encode(mac_result);

        Ok(EncryptedKeyData {
            ciphertext: ciphertext_b64,
            ephemeral_key: ephemeral_key_b64,
            mac: mac_b64,
        })
    }

    // Helper methods
    fn canonical_json_device_keys(
        &self,
        device_keys: &matryx_entity::types::DeviceKey,
    ) -> Result<String, CryptoError> {
        let mut value = serde_json::to_value(device_keys)?;
        if let Some(obj) = value.as_object_mut() {
            obj.remove("signatures");
            obj.remove("unsigned");
        }

        Ok(serde_json::to_string(&value)?)
    }

    fn extract_ed25519_key(
        &self,
        keys: &HashMap<String, String>,
    ) -> Result<Ed25519PublicKey, CryptoError> {
        let ed25519_entry = keys
            .iter()
            .find(|(k, _)| k.starts_with("ed25519:"))
            .ok_or_else(|| CryptoError::MissingField("ed25519 key".to_string()))?;

        Ed25519PublicKey::from_base64(ed25519_entry.1)
            .map_err(|e| CryptoError::InvalidKey(format!("Invalid ed25519 key: {}", e)))
    }

    async fn verify_key_signature_cross_signing(
        &self,
        signed_key: &matryx_entity::types::CrossSigningKey,
        signing_key: &Ed25519PublicKey,
        user_id: &str,
    ) -> Result<bool, CryptoError> {
        let canonical_json = self.canonical_json_cross_signing(signed_key)?;

        let signature = signed_key
            .signatures
            .as_ref()
            .ok_or_else(|| CryptoError::MissingField("signatures".to_string()))?
            .get(user_id)
            .ok_or_else(|| CryptoError::MissingField("user signature".to_string()))?
            .values()
            .next()
            .ok_or_else(|| CryptoError::MissingField("signature value".to_string()))?;

        let signature_bytes = base64::engine::general_purpose::STANDARD
            .decode(signature)
            .map_err(|e| CryptoError::InvalidKey(format!("Invalid signature format: {}", e)))?;

        let vodozemac_signature = vodozemac::Ed25519Signature::from_slice(&signature_bytes)
            .map_err(|e| CryptoError::InvalidKey(format!("Invalid signature: {}", e)))?;

        Ok(signing_key.verify(canonical_json.as_bytes(), &vodozemac_signature).is_ok())
    }

    async fn verify_key_signature_device(
        &self,
        device_keys: &matryx_entity::types::DeviceKey,
        signing_key: &Ed25519PublicKey,
        user_id: &str,
    ) -> Result<bool, CryptoError> {
        let canonical_json = self.canonical_json_device_keys(device_keys)?;

        let signature = device_keys
            .signatures
            .get(user_id)
            .ok_or_else(|| CryptoError::MissingField("user signature".to_string()))?
            .values()
            .next()
            .ok_or_else(|| CryptoError::MissingField("signature value".to_string()))?;

        let signature_bytes = base64::engine::general_purpose::STANDARD
            .decode(signature)
            .map_err(|e| CryptoError::InvalidKey(format!("Invalid signature format: {}", e)))?;

        let vodozemac_signature = vodozemac::Ed25519Signature::from_slice(&signature_bytes)
            .map_err(|e| CryptoError::InvalidKey(format!("Invalid signature: {}", e)))?;

        Ok(signing_key.verify(canonical_json.as_bytes(), &vodozemac_signature).is_ok())
    }

    fn canonical_json_cross_signing(
        &self,
        key: &matryx_entity::types::CrossSigningKey,
    ) -> Result<String, CryptoError> {
        let mut value = serde_json::to_value(key)?;
        if let Some(obj) = value.as_object_mut() {
            obj.remove("signatures");
            obj.remove("unsigned");
        }

        Ok(serde_json::to_string(&value)?)
    }

    /// Cleanup expired crypto data using repository
    pub async fn cleanup_expired_crypto_data(&self, cutoff_days: u32) -> Result<u64, CryptoError> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(cutoff_days as i64);
        self.crypto_repo
            .cleanup_expired_keys(cutoff)
            .await
            .map_err(|e| CryptoError::MissingField(e.to_string()))
    }

    /// Delete all device crypto data using repository
    pub async fn delete_device_crypto_data(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<(), CryptoError> {
        self.crypto_repo
            .delete_device_keys(user_id, device_id)
            .await
            .map_err(|e| CryptoError::MissingField(e.to_string()))
    }

    /// Get user's device keys using repository
    pub async fn get_user_device_keys(
        &self,
        user_id: &str,
    ) -> Result<HashMap<String, DeviceKey>, CryptoError> {
        self.crypto_repo
            .get_user_device_keys(user_id)
            .await
            .map_err(|e| CryptoError::MissingField(e.to_string()))
    }

    /// Validate key signature using repository
    pub async fn validate_key_signature(
        &self,
        key: &serde_json::Value,
        signature: &Signature,
        signing_key: &str,
    ) -> Result<bool, CryptoError> {
        self.crypto_repo
            .validate_key_signature(key, signature, signing_key)
            .await
            .map_err(|e| CryptoError::MissingField(e.to_string()))
    }

    /// Generate key signature using repository
    pub async fn generate_key_signature(
        &self,
        key: &serde_json::Value,
        signing_key: &str,
    ) -> Result<Signature, CryptoError> {
        self.crypto_repo
            .generate_key_signature(key, signing_key)
            .await
            .map_err(|e| CryptoError::MissingField(e.to_string()))
    }

    /// Get backup version using repository
    pub async fn get_backup_version(
        &self,
        user_id: &str,
        version: &str,
    ) -> Result<Option<BackupVersion>, CryptoError> {
        self.key_backup_repo
            .get_backup_version(user_id, version)
            .await
            .map_err(|e| CryptoError::MissingField(e.to_string()))
    }

    /// Delete backup version using repository
    pub async fn delete_backup_version(
        &self,
        user_id: &str,
        version: &str,
    ) -> Result<(), CryptoError> {
        self.key_backup_repo
            .delete_backup_version(user_id, version)
            .await
            .map_err(|e| CryptoError::MissingField(e.to_string()))
    }
}

// Note: Default implementation removed since MatryxCryptoProvider now requires a database connection

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("Invalid key format: {0}")]
    InvalidKey(String),
    #[error("Signature verification failed")]
    SignatureVerificationFailed,
    #[error("Missing required field: {0}")]
    MissingField(String),
    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Base64 decode error: {0}")]
    Base64Error(#[from] base64::DecodeError),
}

// Supporting structures
#[derive(Serialize, Deserialize)]
pub struct AuthData {
    pub public_key: String,
    pub signatures: HashMap<String, HashMap<String, String>>,
}

#[derive(Serialize, Deserialize)]
pub struct RoomKeyBackupData {
    pub session_data: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
pub struct EncryptedKeyData {
    pub ciphertext: String,
    pub ephemeral_key: String,
    pub mac: String,
}

#[cfg(test)]
mod tests;
