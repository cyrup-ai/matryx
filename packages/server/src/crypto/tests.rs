#[cfg(test)]
mod crypto_tests {
    use crate::crypto::{

        CryptoError,
        MatryxCryptoProvider,

    };
    use serde_json::json;
    use std::collections::HashMap;
    use vodozemac::megolm::GroupSession;
    use vodozemac::olm::{Account, SessionConfig};

    #[tokio::test]
    async fn test_device_key_validation_with_vodozemac() {
        let db = surrealdb::engine::any::connect("memory").await.unwrap();
        let crypto_provider = MatryxCryptoProvider::new(db);
        let account = Account::new();

        // Generate valid device keys using vodozemac
        let identity_keys = account.identity_keys();
        let device_keys = create_device_keys_from_account(&account, "@test:example.com", "DEVICE1");

        // Verify that identity keys match what we expect
        assert!(!identity_keys.curve25519.to_base64().is_empty());
        assert!(!identity_keys.ed25519.to_base64().is_empty());

        // Should pass validation
        let result = crypto_provider.verify_device_keys(&device_keys).await;
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_cross_signing_verification() {
        let db = surrealdb::engine::any::connect("memory").await.unwrap();
        let crypto_provider = MatryxCryptoProvider::new(db);

        // Create complete cross-signing chain using vodozemac
        let (master_key, self_signing_key, device_keys) = create_test_cross_signing_chain().await;

        let result = crypto_provider
            .verify_cross_signing_chain(
                "@test:example.com",
                &master_key,
                &self_signing_key,
                &device_keys,
            )
            .await;

        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_olm_session_creation() {
        let db = surrealdb::engine::any::connect("memory").await.unwrap();
        let crypto_provider = MatryxCryptoProvider::new(db);

        let alice_account = Account::new();
        let mut bob_account = Account::new();
        bob_account.generate_one_time_keys(1);

        let bob_otk = *bob_account.one_time_keys().values().next().unwrap();

        let session = crypto_provider
            .create_olm_session(
                &alice_account,
                &bob_account.curve25519_key().to_base64(),
                &bob_otk.to_base64(),
            )
            .await;

        assert!(session.is_ok());
    }

    #[tokio::test]
    async fn test_megolm_group_session() {
        let db = surrealdb::engine::any::connect("memory").await.unwrap();
        let crypto_provider = MatryxCryptoProvider::new(db);

        let group_session = crypto_provider.create_group_session().await;
        assert!(group_session.is_ok());

        let mut session = group_session.unwrap();
        let encrypted = session.encrypt("Hello, room!".as_bytes());
        // MegolmMessage doesn't have is_ok(), it's returned directly
        let _message = encrypted;
    }

    #[tokio::test]
    async fn test_invalid_device_key_signature() {
        let db = surrealdb::engine::any::connect("memory").await.unwrap();
        let crypto_provider = MatryxCryptoProvider::new(db);

        // Create device keys with invalid signature
        let mut device_keys = create_test_device_keys();
        device_keys
            .signatures
            .get_mut("@test:example.com")
            .unwrap()
            .insert("ed25519:DEVICE1".to_string(), "invalid_signature".to_string());

        let result = crypto_provider.verify_device_keys(&device_keys).await;
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_missing_required_keys() {
        let db = surrealdb::engine::any::connect("memory").await.unwrap();
        let crypto_provider = MatryxCryptoProvider::new(db);

        // Create device keys missing ed25519 key
        let mut device_keys = create_test_device_keys();
        device_keys.keys.remove("ed25519");

        let result = crypto_provider.verify_device_keys(&device_keys).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_backup_auth_data_validation() {
        let db = surrealdb::engine::any::connect("memory").await.unwrap();
        let crypto_provider = MatryxCryptoProvider::new(db);

        let auth_data = create_test_backup_auth_data();
        let result = crypto_provider
            .validate_backup_auth_data(&auth_data, "@test:example.com")
            .await;

        // Should pass with valid auth data
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_room_key_encryption_for_backup() {
        let db = surrealdb::engine::any::connect("memory").await.unwrap();
        let crypto_provider = MatryxCryptoProvider::new(db);

        let room_key_data = create_test_room_key_backup_data();
        let auth_data = create_test_backup_auth_data();

        let encrypted = crypto_provider
            .encrypt_room_key_for_backup(&room_key_data, &auth_data)
            .await;
        assert!(encrypted.is_ok());
    }

    #[tokio::test]
    async fn test_canonical_json_generation() {
        let db = surrealdb::engine::any::connect("memory").await.unwrap();
        let crypto_provider = MatryxCryptoProvider::new(db);
        let device_keys = create_test_device_keys();

        let canonical_json = crypto_provider.canonical_json_device_keys(&device_keys);
        assert!(canonical_json.is_ok());

        // Verify canonical JSON is deterministic
        let canonical_json2 = crypto_provider.canonical_json_device_keys(&device_keys);
        assert_eq!(canonical_json.unwrap(), canonical_json2.unwrap());
    }

    #[tokio::test]
    async fn test_one_time_key_validation() {
        let db = surrealdb::engine::any::connect("memory").await.unwrap();
        let crypto_provider = MatryxCryptoProvider::new(db);

        let mut account = Account::new();
        account.generate_one_time_keys(1);
        let one_time_keys = account.one_time_keys();
        let otk = one_time_keys.values().next().unwrap();

        let key_json = json!(otk.to_base64());
        let result = crypto_provider
            .validate_one_time_key("signed_curve25519:AAABBB", &key_json)
            .await;

        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_cross_signing_key_extraction() {
        let db = surrealdb::engine::any::connect("memory").await.unwrap();
        let crypto_provider = MatryxCryptoProvider::new(db);

        let master_key = create_test_master_key();
        let ed25519_key = crypto_provider.extract_ed25519_key(&master_key.keys);

        assert!(ed25519_key.is_ok());
    }

    #[tokio::test]
    async fn test_session_encryption_decryption() {
        let alice_account = Account::new();
        let mut bob_account = Account::new();
        bob_account.generate_one_time_keys(1);

        let bob_otk = *bob_account.one_time_keys().values().next().unwrap();

        // Alice creates outbound session
        let mut alice_session = alice_account.create_outbound_session(
            SessionConfig::version_2(),
            bob_account.curve25519_key(),
            bob_otk,
        );

        // Alice encrypts message
        let message = alice_session.encrypt("Hello Bob!");

        // Bob creates inbound session and decrypts
        // The first message from encrypt() is a PreKeyMessage
        let pre_key_message = match &message {
            vodozemac::olm::OlmMessage::PreKey(msg) => msg,
            vodozemac::olm::OlmMessage::Normal(_) => panic!("First message should be PreKey"),
        };
        
        let mut bob_session_result = bob_account
            .create_inbound_session(alice_account.curve25519_key(), pre_key_message)
            .unwrap();

        let decrypted = bob_session_result.session.decrypt(&message).unwrap();
        assert_eq!(decrypted, "Hello Bob!".as_bytes());
    }

    #[tokio::test]
    async fn test_group_session_ratcheting() {
        let mut group_session = GroupSession::new(Default::default());

        // Encrypt multiple messages to test ratcheting
        let msg1 = group_session.encrypt("First message".as_bytes());
        let msg2 = group_session.encrypt("Second message".as_bytes());

        // MegolmMessage is returned directly, not as Result
        // Verify message indices are different
        assert_ne!(msg1.message_index(), msg2.message_index());
    }

    #[tokio::test]
    async fn test_error_handling_invalid_base64() {
        let db = surrealdb::engine::any::connect("memory").await.unwrap();
        let crypto_provider = MatryxCryptoProvider::new(db);

        let mut device_keys = create_test_device_keys();
        device_keys
            .keys
            .insert("ed25519".to_string(), "invalid_base64!@#".to_string());

        let result = crypto_provider.verify_device_keys(&device_keys).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            CryptoError::InvalidKey(_) => {}, // Expected error type
            _ => panic!("Expected InvalidKey error"),
        }
    }

    // Helper functions for creating test data

    fn create_device_keys_from_account(
        account: &Account,
        user_id: &str,
        device_id: &str,
    ) -> matryx_entity::types::DeviceKey {
        let identity_keys = account.identity_keys();

        let mut keys = HashMap::new();
        keys.insert(format!("curve25519:{}", device_id), identity_keys.curve25519.to_base64());
        keys.insert(format!("ed25519:{}", device_id), identity_keys.ed25519.to_base64());

        let mut device_keys = matryx_entity::types::DeviceKey {
            user_id: user_id.to_string(),
            device_id: device_id.to_string(),
            algorithms: vec![
                "m.olm.v1.curve25519-aes-sha2".to_string(),
                "m.megolm.v1.aes-sha2".to_string(),
            ],
            keys,
            signatures: HashMap::new(),
            unsigned: None,
        };

        // Sign the device keys
        let canonical_json = serde_json::to_string(&json!({
            "user_id": device_keys.user_id,
            "device_id": device_keys.device_id,
            "algorithms": device_keys.algorithms,
            "keys": device_keys.keys,
        }))
        .unwrap();

        let signature = account.sign(&canonical_json);
        let mut user_signatures = HashMap::new();
        user_signatures.insert(format!("ed25519:{}", device_id), signature.to_base64());
        device_keys.signatures.insert(user_id.to_string(), user_signatures);

        device_keys
    }

    fn create_test_device_keys() -> matryx_entity::types::DeviceKey {
        let account = Account::new();
        create_device_keys_from_account(&account, "@test:example.com", "DEVICE1")
    }

    fn create_test_backup_auth_data() -> crate::crypto::AuthData {
        let account = Account::new();
        let identity_keys = account.identity_keys();

        let mut signatures = HashMap::new();
        let mut user_signatures = HashMap::new();
        user_signatures
            .insert("ed25519:DEVICE1".to_string(), account.sign("backup_auth_data").to_base64());
        signatures.insert("@test:example.com".to_string(), user_signatures);

        crate::crypto::AuthData {
            public_key: identity_keys.curve25519.to_base64(),
            signatures,
        }
    }

    fn create_test_room_key_backup_data() -> crate::crypto::RoomKeyBackupData {
        crate::crypto::RoomKeyBackupData {
            session_data: json!({
                "ephemeral": "test_ephemeral_key",
                "ciphertext": "encrypted_session_key",
                "mac": "test_mac"
            }),
        }
    }

    fn create_test_master_key() -> matryx_entity::types::CrossSigningKey {
        let account = Account::new();
        let identity_keys = account.identity_keys();

        let mut keys = HashMap::new();
        keys.insert("ed25519:master_key".to_string(), identity_keys.ed25519.to_base64());

        matryx_entity::types::CrossSigningKey {
            user_id: "@test:example.com".to_string(),
            usage: vec!["master".to_string()],
            keys,
            signatures: None,
        }
    }

    async fn create_test_cross_signing_chain() -> (
        matryx_entity::types::CrossSigningKey,
        matryx_entity::types::CrossSigningKey,
        matryx_entity::types::DeviceKey,
    ) {
        // Create master key
        let master_account = Account::new();
        let master_identity = master_account.identity_keys();

        let mut master_keys = HashMap::new();
        master_keys.insert("ed25519:master".to_string(), master_identity.ed25519.to_base64());

        let master_key = matryx_entity::types::CrossSigningKey {
            user_id: "@test:example.com".to_string(),
            usage: vec!["master".to_string()],
            keys: master_keys,
            signatures: None,
        };

        // Create self-signing key
        let self_signing_account = Account::new();
        let self_signing_identity = self_signing_account.identity_keys();

        let mut self_signing_keys = HashMap::new();
        self_signing_keys
            .insert("ed25519:self_signing".to_string(), self_signing_identity.ed25519.to_base64());

        let mut self_signing_key = matryx_entity::types::CrossSigningKey {
            user_id: "@test:example.com".to_string(),
            usage: vec!["self_signing".to_string()],
            keys: self_signing_keys,
            signatures: None,
        };

        // Sign self-signing key with master key
        let self_signing_canonical = serde_json::to_string(&json!({
            "user_id": self_signing_key.user_id,
            "usage": self_signing_key.usage,
            "keys": self_signing_key.keys,
        }))
        .unwrap();

        let master_signature = master_account.sign(&self_signing_canonical);
        let mut master_sigs = HashMap::new();
        master_sigs.insert("ed25519:master".to_string(), master_signature.to_base64());
        let mut signatures = HashMap::new();
        signatures.insert("@test:example.com".to_string(), master_sigs);
        self_signing_key.signatures = Some(signatures);

        // Create device keys
        let device_account = Account::new();
        let mut device_keys =
            create_device_keys_from_account(&device_account, "@test:example.com", "DEVICE1");

        // Sign device keys with self-signing key
        let device_canonical = serde_json::to_string(&json!({
            "user_id": device_keys.user_id,
            "device_id": device_keys.device_id,
            "algorithms": device_keys.algorithms,
            "keys": device_keys.keys,
        }))
        .unwrap();

        let self_signing_signature = self_signing_account.sign(&device_canonical);
        device_keys
            .signatures
            .get_mut("@test:example.com")
            .unwrap()
            .insert("ed25519:self_signing".to_string(), self_signing_signature.to_base64());

        (master_key, self_signing_key, device_keys)
    }
}
