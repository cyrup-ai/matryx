#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::cross_signing::{CrossSigningVerifier, TestCryptoProvider};
    use matryx_surrealdb::repository::cross_signing::{
        CrossSigningKey,
        CrossSigningKeys,
        DeviceKey,
    };
    use std::collections::HashMap;
    use std::sync::Arc;
    use surrealdb::{Surreal, engine::any::Any};

    async fn setup_test_db() -> Surreal<Any> {
        let db = surrealdb::engine::any::connect("memory").await.unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        db
    }

    fn create_test_master_key(user_id: &str) -> CrossSigningKey {
        CrossSigningKey {
            user_id: user_id.to_string(),
            usage: vec!["master".to_string()],
            keys: {
                let mut keys = HashMap::new();
                keys.insert("ed25519:master_key_id".to_string(), "master_public_key".to_string());
                keys
            },
            signatures: None,
        }
    }

    fn create_test_self_signing_key(user_id: &str) -> CrossSigningKey {
        CrossSigningKey {
            user_id: user_id.to_string(),
            usage: vec!["self_signing".to_string()],
            keys: {
                let mut keys = HashMap::new();
                keys.insert(
                    "ed25519:self_signing_key_id".to_string(),
                    "self_signing_public_key".to_string(),
                );
                keys
            },
            signatures: Some({
                let mut sigs = HashMap::new();
                let mut user_sigs = HashMap::new();
                user_sigs.insert(
                    "ed25519:master_key_id".to_string(),
                    "master_signature_on_self_signing".to_string(),
                );
                sigs.insert(user_id.to_string(), user_sigs);
                sigs
            }),
        }
    }

    fn create_test_device_key(user_id: &str, device_id: &str) -> DeviceKey {
        DeviceKey {
            user_id: user_id.to_string(),
            device_id: device_id.to_string(),
            algorithms: vec![
                "m.olm.v1.curve25519-aes-sha2".to_string(),
                "m.megolm.v1.aes-sha2".to_string(),
            ],
            keys: {
                let mut keys = HashMap::new();
                keys.insert(
                    format!("curve25519:{}", device_id),
                    "device_curve25519_key".to_string(),
                );
                keys.insert(format!("ed25519:{}", device_id), "device_ed25519_key".to_string());
                keys
            },
            signatures: {
                let mut signatures = HashMap::new();
                let mut user_sigs = HashMap::new();
                user_sigs
                    .insert(format!("ed25519:{}", device_id), "device_self_signature".to_string());
                user_sigs.insert(
                    "ed25519:self_signing_key_id".to_string(),
                    "self_signing_signature_on_device".to_string(),
                );
                signatures.insert(user_id.to_string(), user_sigs);
                signatures
            },
            unsigned: None,
        }
    }

    #[tokio::test]
    async fn test_cross_signing_verifier_creation() {
        let db = setup_test_db().await;
        let crypto_provider = Arc::new(TestCryptoProvider);
        let verifier = CrossSigningVerifier::new(crypto_provider, db);

        // Test that verifier is properly initialized
        assert!(
            verifier
                .crypto
                .verify_ed25519_signature("sig", "msg", "key")
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn test_store_and_retrieve_cross_signing_keys() {
        let db = setup_test_db().await;
        let crypto_provider = Arc::new(TestCryptoProvider);
        let verifier = CrossSigningVerifier::new(crypto_provider, db);

        let user_id = "@alice:example.com";
        let master_key = create_test_master_key(user_id);
        let self_signing_key = create_test_self_signing_key(user_id);

        let keys = CrossSigningKeys {
            master_key: Some(master_key.clone()),
            self_signing_key: Some(self_signing_key.clone()),
            user_signing_key: None,
        };

        // Test storing cross-signing keys
        verifier.store_cross_signing_keys(user_id, &keys).await.unwrap();

        // Test retrieving cross-signing keys
        let retrieved_keys = verifier.get_cross_signing_keys(user_id).await.unwrap();
        assert!(retrieved_keys.master_key.is_some());
        assert!(retrieved_keys.self_signing_key.is_some());
        assert!(retrieved_keys.user_signing_key.is_none());

        let retrieved_master = retrieved_keys.master_key.unwrap();
        assert_eq!(retrieved_master.user_id, user_id);
        assert_eq!(retrieved_master.usage, vec!["master"]);
        assert!(retrieved_master.keys.contains_key("ed25519:master_key_id"));
    }

    #[tokio::test]
    async fn test_device_signature_verification() {
        let db = setup_test_db().await;
        let crypto_provider = Arc::new(TestCryptoProvider);
        let verifier = CrossSigningVerifier::new(crypto_provider, db);

        let user_id = "@alice:example.com";
        let device_id = "DEVICE1";
        let self_signing_key = create_test_self_signing_key(user_id);
        let device_key = create_test_device_key(user_id, device_id);

        // Test device signature verification
        let verification_result = verifier
            .verify_device_signature(&device_key, &self_signing_key)
            .await
            .unwrap();
        assert!(verification_result); // TestCryptoProvider always returns true
    }

    #[tokio::test]
    async fn test_device_signature_verification_with_repository() {
        let db = setup_test_db().await;
        let crypto_provider = Arc::new(TestCryptoProvider);
        let verifier = CrossSigningVerifier::new(crypto_provider, db);

        let user_id = "@alice:example.com";
        let device_id = "DEVICE1";
        let self_signing_key = create_test_self_signing_key(user_id);
        let device_key = create_test_device_key(user_id, device_id);

        // Store self-signing key
        verifier
            .repository
            .store_self_signing_key(user_id, &self_signing_key)
            .await
            .unwrap();

        // Test device signature verification using repository
        let verification_result = verifier
            .verify_device_signature_with_repository(user_id, &device_key)
            .await
            .unwrap();
        assert!(verification_result);
    }

    #[tokio::test]
    async fn test_self_signing_key_verification() {
        let db = setup_test_db().await;
        let crypto_provider = Arc::new(TestCryptoProvider);
        let verifier = CrossSigningVerifier::new(crypto_provider, db);

        let user_id = "@alice:example.com";
        let master_key = create_test_master_key(user_id);
        let self_signing_key = create_test_self_signing_key(user_id);

        // Test self-signing key verification
        let verification_result = verifier
            .verify_self_signing_key(&self_signing_key, &master_key)
            .await
            .unwrap();
        assert!(verification_result); // TestCryptoProvider always returns true
    }

    #[tokio::test]
    async fn test_self_signing_key_verification_with_repository() {
        let db = setup_test_db().await;
        let crypto_provider = Arc::new(TestCryptoProvider);
        let verifier = CrossSigningVerifier::new(crypto_provider, db);

        let user_id = "@alice:example.com";
        let master_key = create_test_master_key(user_id);
        let self_signing_key = create_test_self_signing_key(user_id);

        // Store keys
        verifier.repository.store_master_key(user_id, &master_key).await.unwrap();
        verifier
            .repository
            .store_self_signing_key(user_id, &self_signing_key)
            .await
            .unwrap();

        // Test self-signing key verification using repository
        let verification_result =
            verifier.verify_self_signing_key_with_repository(user_id).await.unwrap();
        assert!(verification_result);
    }

    #[tokio::test]
    async fn test_device_key_signing() {
        let db = setup_test_db().await;
        let crypto_provider = Arc::new(TestCryptoProvider);
        let verifier = CrossSigningVerifier::new(crypto_provider, db);

        let user_id = "@alice:example.com";
        let device_id = "DEVICE1";
        let self_signing_key = create_test_self_signing_key(user_id);
        let device_key = create_test_device_key(user_id, device_id);

        // Store self-signing key
        verifier
            .repository
            .store_self_signing_key(user_id, &self_signing_key)
            .await
            .unwrap();

        // Test device key signing
        let signature = verifier
            .sign_device_key_with_repository(user_id, device_id, &device_key)
            .await
            .unwrap();
        assert_eq!(signature.algorithm, "ed25519");
        assert!(!signature.signature.is_empty());
    }

    #[tokio::test]
    async fn test_device_trust_management() {
        let db = setup_test_db().await;
        let crypto_provider = Arc::new(TestCryptoProvider);
        let verifier = CrossSigningVerifier::new(crypto_provider, db);

        let user_id = "@alice:example.com";
        let device_id = "DEVICE1";
        let trusted_by = "@bob:example.com";

        // Test marking device as trusted
        verifier.mark_device_trusted(user_id, device_id, trusted_by).await.unwrap();

        // Test getting trusted devices
        let trusted_devices = verifier.get_trusted_devices(user_id).await.unwrap();
        assert!(trusted_devices.contains(&device_id.to_string()));

        // Test device trust in cross-signing chain
        let master_key = create_test_master_key(user_id);
        let self_signing_key = create_test_self_signing_key(user_id);

        let keys = CrossSigningKeys {
            master_key: Some(master_key),
            self_signing_key: Some(self_signing_key),
            user_signing_key: None,
        };

        verifier.store_cross_signing_keys(user_id, &keys).await.unwrap();

        let chain_verified = verifier.verify_cross_signing_chain(user_id, device_id).await.unwrap();
        assert!(chain_verified);
    }

    #[tokio::test]
    async fn test_complete_cross_signing_chain_verification() {
        let db = setup_test_db().await;
        let crypto_provider = Arc::new(TestCryptoProvider);
        let verifier = CrossSigningVerifier::new(crypto_provider, db);

        let user_id = "@alice:example.com";
        let device_id = "DEVICE1";

        // Setup complete cross-signing chain
        let master_key = create_test_master_key(user_id);
        let self_signing_key = create_test_self_signing_key(user_id);

        let keys = CrossSigningKeys {
            master_key: Some(master_key),
            self_signing_key: Some(self_signing_key),
            user_signing_key: None,
        };

        // Store keys and mark device as trusted
        verifier.store_cross_signing_keys(user_id, &keys).await.unwrap();
        verifier.mark_device_trusted(user_id, device_id, user_id).await.unwrap();

        // Verify complete chain
        let chain_verified = verifier.verify_cross_signing_chain(user_id, device_id).await.unwrap();
        assert!(chain_verified);

        // Test with untrusted device
        let untrusted_device = "DEVICE2";
        let untrusted_chain = verifier
            .verify_cross_signing_chain(user_id, untrusted_device)
            .await
            .unwrap();
        assert!(!untrusted_chain);
    }

    #[tokio::test]
    async fn test_canonical_json_generation() {
        let db = setup_test_db().await;
        let crypto_provider = Arc::new(TestCryptoProvider);
        let verifier = CrossSigningVerifier::new(crypto_provider, db);

        let user_id = "@alice:example.com";
        let device_id = "DEVICE1";
        let device_key = create_test_device_key(user_id, device_id);

        // Test canonical JSON generation (private method tested indirectly)
        let canonical_result = verifier.canonical_json(&device_key);
        assert!(canonical_result.is_ok());

        let canonical_json = canonical_result.unwrap();
        // Canonical JSON should not contain signatures or unsigned fields
        assert!(!canonical_json.contains("signatures"));
        assert!(!canonical_json.contains("unsigned"));
        assert!(canonical_json.contains("user_id"));
        assert!(canonical_json.contains("device_id"));
        assert!(canonical_json.contains("keys"));
    }

    #[tokio::test]
    async fn test_error_handling() {
        let db = setup_test_db().await;
        let crypto_provider = Arc::new(TestCryptoProvider);
        let verifier = CrossSigningVerifier::new(crypto_provider, db);

        let user_id = "@alice:example.com";
        let device_id = "DEVICE1";

        // Test verification with missing self-signing key
        let device_key = create_test_device_key(user_id, device_id);
        let result = verifier
            .verify_device_signature_with_repository(user_id, &device_key)
            .await;
        assert!(result.is_err());

        // Test verification with missing master key
        let result = verifier.verify_self_signing_key_with_repository(user_id).await;
        assert!(result.is_err());

        // Test signing with missing self-signing key
        let result = verifier
            .sign_device_key_with_repository(user_id, device_id, &device_key)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cross_signing_keys_with_user_signing() {
        let db = setup_test_db().await;
        let crypto_provider = Arc::new(TestCryptoProvider);
        let verifier = CrossSigningVerifier::new(crypto_provider, db);

        let user_id = "@alice:example.com";

        let user_signing_key = CrossSigningKey {
            user_id: user_id.to_string(),
            usage: vec!["user_signing".to_string()],
            keys: {
                let mut keys = HashMap::new();
                keys.insert(
                    "ed25519:user_signing_key_id".to_string(),
                    "user_signing_public_key".to_string(),
                );
                keys
            },
            signatures: Some({
                let mut sigs = HashMap::new();
                let mut user_sigs = HashMap::new();
                user_sigs.insert(
                    "ed25519:master_key_id".to_string(),
                    "master_signature_on_user_signing".to_string(),
                );
                sigs.insert(user_id.to_string(), user_sigs);
                sigs
            }),
        };

        let keys = CrossSigningKeys {
            master_key: Some(create_test_master_key(user_id)),
            self_signing_key: Some(create_test_self_signing_key(user_id)),
            user_signing_key: Some(user_signing_key),
        };

        // Test storing and retrieving all three key types
        verifier.store_cross_signing_keys(user_id, &keys).await.unwrap();

        let retrieved_keys = verifier.get_cross_signing_keys(user_id).await.unwrap();
        assert!(retrieved_keys.master_key.is_some());
        assert!(retrieved_keys.self_signing_key.is_some());
        assert!(retrieved_keys.user_signing_key.is_some());

        let retrieved_user_signing = retrieved_keys.user_signing_key.unwrap();
        assert_eq!(retrieved_user_signing.usage, vec!["user_signing"]);
    }
}
