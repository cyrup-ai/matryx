//! Module contains intentional library code not yet fully integrated
#![allow(dead_code)]

use async_trait::async_trait;
use serde::Serialize;
use std::sync::Arc;
use surrealdb::{Surreal, engine::any::Any};

use matryx_surrealdb::repository::CrossSigningRepository;
use matryx_surrealdb::repository::cross_signing::{
    CrossSigningKey, CrossSigningKeys, DeviceKey, Signature,
};

#[derive(Debug)]
pub enum CryptoError {
    MissingSignature,
    InvalidKey,
    InvalidSignature,
    SerializationError(String),
}

impl From<serde_json::Error> for CryptoError {
    fn from(e: serde_json::Error) -> Self {
        CryptoError::SerializationError(e.to_string())
    }
}

#[async_trait]
pub trait CryptoProvider: Send + Sync {
    async fn verify_ed25519_signature(
        &self,
        signature: &str,
        message: &str,
        public_key: &str,
    ) -> Result<bool, CryptoError>;
}

pub struct CrossSigningVerifier {
    pub crypto: Arc<dyn CryptoProvider>,
    pub repository: CrossSigningRepository,
}

impl CrossSigningVerifier {
    pub fn new(crypto: Arc<dyn CryptoProvider>, db: Surreal<Any>) -> Self {
        Self {
            crypto,
            repository: CrossSigningRepository::new(db),
        }
    }

    /// Store cross-signing keys using repository
    pub async fn store_cross_signing_keys(
        &self,
        user_id: &str,
        keys: &CrossSigningKeys,
    ) -> Result<(), CryptoError> {
        if let Some(ref master_key) = keys.master_key {
            self.repository
                .store_master_key(user_id, master_key)
                .await
                .map_err(|e| CryptoError::SerializationError(e.to_string()))?;
        }

        if let Some(ref self_signing_key) = keys.self_signing_key {
            self.repository
                .store_self_signing_key(user_id, self_signing_key)
                .await
                .map_err(|e| CryptoError::SerializationError(e.to_string()))?;
        }

        if let Some(ref user_signing_key) = keys.user_signing_key {
            self.repository
                .store_user_signing_key(user_id, user_signing_key)
                .await
                .map_err(|e| CryptoError::SerializationError(e.to_string()))?;
        }

        Ok(())
    }

    /// Get cross-signing keys from repository
    pub async fn get_cross_signing_keys(
        &self,
        user_id: &str,
    ) -> Result<CrossSigningKeys, CryptoError> {
        self.repository
            .get_all_cross_signing_keys(user_id)
            .await
            .map_err(|e| CryptoError::SerializationError(e.to_string()))
    }

    /// Verify device signature against self-signing key using repository
    pub async fn verify_device_signature_with_repository(
        &self,
        user_id: &str,
        device_keys: &DeviceKey,
    ) -> Result<bool, CryptoError> {
        // Get self-signing key from repository
        let self_signing_key = self
            .repository
            .get_self_signing_key(user_id)
            .await
            .map_err(|e| CryptoError::SerializationError(e.to_string()))?
            .ok_or(CryptoError::InvalidKey)?;

        self.verify_device_signature_internal(device_keys, &self_signing_key).await
    }

    /// Verify device signature against self-signing key
    pub async fn verify_device_signature(
        &self,
        device_keys: &DeviceKey,
        self_signing_key: &CrossSigningKey,
    ) -> Result<bool, CryptoError> {
        self.verify_device_signature_internal(device_keys, self_signing_key).await
    }

    /// Internal device signature verification
    async fn verify_device_signature_internal(
        &self,
        device_keys: &DeviceKey,
        self_signing_key: &CrossSigningKey,
    ) -> Result<bool, CryptoError> {
        // Extract device signature
        let user_signatures = device_keys
            .signatures
            .get(&device_keys.user_id)
            .ok_or(CryptoError::MissingSignature)?;

        let self_signing_key_id = format!(
            "ed25519:{}",
            self_signing_key
                .keys
                .keys()
                .next()
                .ok_or(CryptoError::InvalidKey)?
                .split(':')
                .nth(1)
                .ok_or(CryptoError::InvalidKey)?
        );

        let signature = user_signatures
            .get(&self_signing_key_id)
            .ok_or(CryptoError::MissingSignature)?;

        // Verify signature against canonical JSON
        let canonical_json = self.canonical_json(device_keys)?;
        self.crypto
            .verify_ed25519_signature(
                signature,
                &canonical_json,
                self_signing_key.keys.values().next().ok_or(CryptoError::InvalidKey)?,
            )
            .await
    }

    /// Verify self-signing key against master key using repository
    pub async fn verify_self_signing_key_with_repository(
        &self,
        user_id: &str,
    ) -> Result<bool, CryptoError> {
        // Get keys from repository
        let master_key = self
            .repository
            .get_master_key(user_id)
            .await
            .map_err(|e| CryptoError::SerializationError(e.to_string()))?
            .ok_or(CryptoError::InvalidKey)?;

        let self_signing_key = self
            .repository
            .get_self_signing_key(user_id)
            .await
            .map_err(|e| CryptoError::SerializationError(e.to_string()))?
            .ok_or(CryptoError::InvalidKey)?;

        self.verify_self_signing_key_internal(&self_signing_key, &master_key).await
    }

    /// Verify self-signing key against master key
    pub async fn verify_self_signing_key(
        &self,
        self_signing_key: &CrossSigningKey,
        master_key: &CrossSigningKey,
    ) -> Result<bool, CryptoError> {
        self.verify_self_signing_key_internal(self_signing_key, master_key).await
    }

    /// Internal self-signing key verification
    async fn verify_self_signing_key_internal(
        &self,
        self_signing_key: &CrossSigningKey,
        master_key: &CrossSigningKey,
    ) -> Result<bool, CryptoError> {
        let master_signatures = self_signing_key
            .signatures
            .as_ref()
            .and_then(|sigs| sigs.get(&self_signing_key.user_id))
            .ok_or(CryptoError::MissingSignature)?;

        let master_key_id = format!(
            "ed25519:{}",
            master_key
                .keys
                .keys()
                .next()
                .ok_or(CryptoError::InvalidKey)?
                .split(':')
                .nth(1)
                .ok_or(CryptoError::InvalidKey)?
        );

        let signature = master_signatures
            .get(&master_key_id)
            .ok_or(CryptoError::MissingSignature)?;

        let canonical_json = self.canonical_json(self_signing_key)?;
        self.crypto
            .verify_ed25519_signature(
                signature,
                &canonical_json,
                master_key.keys.values().next().ok_or(CryptoError::InvalidKey)?,
            )
            .await
    }

    /// Sign device key using repository
    pub async fn sign_device_key_with_repository(
        &self,
        user_id: &str,
        device_id: &str,
        device_key: &DeviceKey,
    ) -> Result<Signature, CryptoError> {
        // Get self-signing key from repository
        let self_signing_key = self
            .repository
            .get_self_signing_key(user_id)
            .await
            .map_err(|e| CryptoError::SerializationError(e.to_string()))?
            .ok_or(CryptoError::InvalidKey)?;

        self.repository
            .sign_device_key(user_id, device_id, device_key, &self_signing_key)
            .await
            .map_err(|e| CryptoError::SerializationError(e.to_string()))
    }

    /// Mark device as trusted using repository
    pub async fn mark_device_trusted(
        &self,
        user_id: &str,
        device_id: &str,
        trusted_by: &str,
    ) -> Result<(), CryptoError> {
        self.repository
            .mark_device_trusted(user_id, device_id, trusted_by)
            .await
            .map_err(|e| CryptoError::SerializationError(e.to_string()))
    }

    /// Get trusted devices using repository
    pub async fn get_trusted_devices(&self, user_id: &str) -> Result<Vec<String>, CryptoError> {
        self.repository
            .get_trusted_devices(user_id)
            .await
            .map_err(|e| CryptoError::SerializationError(e.to_string()))
    }

    /// Verify complete cross-signing chain using repository
    pub async fn verify_cross_signing_chain(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<bool, CryptoError> {
        // Get all cross-signing keys
        let keys = self.get_cross_signing_keys(user_id).await?;

        // Verify master key exists
        let master_key = keys.master_key.ok_or(CryptoError::InvalidKey)?;
        let self_signing_key = keys.self_signing_key.ok_or(CryptoError::InvalidKey)?;

        // Verify self-signing key is signed by master key
        if !self.verify_self_signing_key(&self_signing_key, &master_key).await? {
            return Ok(false);
        }

        // Check if device is in trusted list (indicating it's been cross-signed)
        let trusted_devices = self.get_trusted_devices(user_id).await?;
        Ok(trusted_devices.contains(&device_id.to_string()))
    }

    pub fn canonical_json<T: Serialize>(&self, object: &T) -> Result<String, CryptoError> {
        // Remove signatures field and create canonical JSON
        let mut value = serde_json::to_value(object)?;
        if let Some(obj) = value.as_object_mut() {
            obj.remove("signatures");
            obj.remove("unsigned");
        }

        // Sort keys and minimize whitespace
        Ok(serde_json::to_string(&value)?)
    }
}

// Placeholder crypto provider for testing
pub struct TestCryptoProvider;

#[async_trait]
impl CryptoProvider for TestCryptoProvider {
    async fn verify_ed25519_signature(
        &self,
        _signature: &str,
        _message: &str,
        _public_key: &str,
    ) -> Result<bool, CryptoError> {
        // Placeholder implementation - always returns true for testing
        Ok(true)
    }
}
