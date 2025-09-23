#[cfg(test)]
mod tests {
    use crate::repository::cross_signing::{CrossSigningKey, CrossSigningKeys};
    use crate::repository::crypto::{DeviceKey, FallbackKey, OneTimeKey, Signature};
    use crate::repository::key_backup::EncryptedRoomKey;
    use crate::repository::{
        CrossSigningRepository,
        CryptoRepository,
        CryptoService,
        KeyBackupRepository,
    };
    use chrono::Utc;
    use std::collections::HashMap;
    use surrealdb::{Surreal, engine::any::Any};

    async fn setup_test_db() -> Surreal<Any> {
        let db = surrealdb::engine::any::connect("surrealkv://test_data/crypto_test.db").await.unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        db
    }

    #[tokio::test]
    async fn test_crypto_repository_device_keys() {
        let db = setup_test_db().await;
        let repo = CryptoRepository::new(db);

        let user_id = "@alice:example.com";
        let device_id = "DEVICE1";

        // Create test device key
        let device_key = DeviceKey {
            user_id: user_id.to_string(),
            device_id: device_id.to_string(),
            algorithms: vec![
                "m.olm.v1.curve25519-aes-sha2".to_string(),
                "m.megolm.v1.aes-sha2".to_string(),
            ],
            keys: {
                let mut keys = HashMap::new();
                keys.insert("curve25519:DEVICE1".to_string(), "curve25519_key_data".to_string());
                keys.insert("ed25519:DEVICE1".to_string(), "ed25519_key_data".to_string());
                keys
            },
            signatures: {
                let mut signatures = HashMap::new();
                let mut user_sigs = HashMap::new();
                user_sigs.insert("ed25519:DEVICE1".to_string(), "signature_data".to_string());
                signatures.insert(user_id.to_string(), user_sigs);
                signatures
            },
            unsigned: None,
        };

        // Test storing device key
        repo.store_device_key(user_id, device_id, &device_key).await.unwrap();

        // Test retrieving device key
        let retrieved = repo.get_device_key(user_id, device_id).await.unwrap();
        assert!(retrieved.is_some());
        let retrieved_key = retrieved.unwrap();
        assert_eq!(retrieved_key.user_id, user_id);
        assert_eq!(retrieved_key.device_id, device_id);
        assert_eq!(retrieved_key.algorithms.len(), 2);

        // Test getting user device keys
        let user_keys = repo.get_user_device_keys(user_id).await.unwrap();
        assert_eq!(user_keys.len(), 1);
        assert!(user_keys.contains_key(device_id));

        // Test updating device key
        let mut updated_key = device_key.clone();
        updated_key.algorithms.push("new_algorithm".to_string());
        repo.update_device_key(user_id, device_id, &updated_key).await.unwrap();

        let retrieved_updated = repo.get_device_key(user_id, device_id).await.unwrap().unwrap();
        assert_eq!(retrieved_updated.algorithms.len(), 3);

        // Test deleting device keys
        repo.delete_device_keys(user_id, device_id).await.unwrap();
        let deleted = repo.get_device_key(user_id, device_id).await.unwrap();
        assert!(deleted.is_none());
    }

    #[tokio::test]
    async fn test_crypto_repository_one_time_keys() {
        let db = setup_test_db().await;
        let repo = CryptoRepository::new(db);

        let user_id = "@alice:example.com";
        let device_id = "DEVICE1";
        let key_id = "signed_curve25519:AAAAGw";

        let one_time_key = OneTimeKey {
            key_id: key_id.to_string(),
            key: "one_time_key_data".to_string(),
            algorithm: "signed_curve25519".to_string(),
            signatures: None,
            created_at: Utc::now(),
        };

        // Test storing one-time key
        repo.store_one_time_key(user_id, device_id, key_id, &one_time_key)
            .await
            .unwrap();

        // Test getting one-time key count
        let counts = repo.get_one_time_key_count(user_id, device_id).await.unwrap();
        assert_eq!(counts.get("signed_curve25519"), Some(&1));

        // Test claiming one-time key
        let claimed = repo
            .claim_one_time_key(user_id, device_id, "signed_curve25519")
            .await
            .unwrap();
        assert!(claimed.is_some());
        let claimed_key = claimed.unwrap();
        assert_eq!(claimed_key.key_id, key_id);

        // Verify key count decreased
        let counts_after = repo.get_one_time_key_count(user_id, device_id).await.unwrap();
        assert_eq!(counts_after.get("signed_curve25519"), Some(&0));

        // Test claiming when no keys available
        let no_key = repo
            .claim_one_time_key(user_id, device_id, "signed_curve25519")
            .await
            .unwrap();
        assert!(no_key.is_none());
    }

    #[tokio::test]
    async fn test_crypto_repository_fallback_keys() {
        let db = setup_test_db().await;
        let repo = CryptoRepository::new(db);

        let user_id = "@alice:example.com";
        let device_id = "DEVICE1";

        let fallback_key = FallbackKey {
            key_id: "signed_curve25519:fallback".to_string(),
            key: "fallback_key_data".to_string(),
            algorithm: "signed_curve25519".to_string(),
            signatures: None,
            created_at: Utc::now(),
            is_current: true,
        };

        // Test storing fallback key
        repo.store_fallback_key(user_id, device_id, &fallback_key).await.unwrap();

        // Test retrieving fallback key
        let retrieved = repo.get_fallback_key(user_id, device_id).await.unwrap();
        assert!(retrieved.is_some());
        let retrieved_key = retrieved.unwrap();
        assert_eq!(retrieved_key.key_id, fallback_key.key_id);
        assert!(retrieved_key.is_current);

        // Test storing new fallback key (should mark old as not current)
        let new_fallback = FallbackKey {
            key_id: "signed_curve25519:fallback2".to_string(),
            key: "new_fallback_key_data".to_string(),
            algorithm: "signed_curve25519".to_string(),
            signatures: None,
            created_at: Utc::now(),
            is_current: true,
        };

        repo.store_fallback_key(user_id, device_id, &new_fallback).await.unwrap();

        let current_fallback = repo.get_fallback_key(user_id, device_id).await.unwrap().unwrap();
        assert_eq!(current_fallback.key_id, new_fallback.key_id);
    }

    #[tokio::test]
    async fn test_cross_signing_repository() {
        let db = setup_test_db().await;
        let repo = CrossSigningRepository::new(db);

        let user_id = "@alice:example.com";

        // Create test cross-signing keys
        let master_key = CrossSigningKey {
            user_id: user_id.to_string(),
            usage: vec!["master".to_string()],
            keys: {
                let mut keys = HashMap::new();
                keys.insert("ed25519:master_key_id".to_string(), "master_key_data".to_string());
                keys
            },
            signatures: Some(HashMap::new()),
        };

        let self_signing_key = CrossSigningKey {
            user_id: user_id.to_string(),
            usage: vec!["self_signing".to_string()],
            keys: {
                let mut keys = HashMap::new();
                keys.insert(
                    "ed25519:self_signing_key_id".to_string(),
                    "self_signing_key_data".to_string(),
                );
                keys
            },
            signatures: Some({
                let mut sigs = HashMap::new();
                let mut user_sigs = HashMap::new();
                user_sigs
                    .insert("ed25519:master_key_id".to_string(), "master_signature".to_string());
                sigs.insert(user_id.to_string(), user_sigs);
                sigs
            }),
        };

        // Test storing cross-signing keys
        repo.store_master_key(user_id, &master_key).await.unwrap();
        repo.store_self_signing_key(user_id, &self_signing_key).await.unwrap();

        // Test retrieving keys
        let retrieved_master = repo.get_master_key(user_id).await.unwrap();
        assert!(retrieved_master.is_some());
        assert_eq!(retrieved_master.unwrap().usage, vec!["master"]);

        let retrieved_self_signing = repo.get_self_signing_key(user_id).await.unwrap();
        assert!(retrieved_self_signing.is_some());
        assert_eq!(retrieved_self_signing.unwrap().usage, vec!["self_signing"]);

        // Test getting all cross-signing keys
        let all_keys = repo.get_all_cross_signing_keys(user_id).await.unwrap();
        assert!(all_keys.master_key.is_some());
        assert!(all_keys.self_signing_key.is_some());
        assert!(all_keys.user_signing_key.is_none());

        // Test device trust
        let device_id = "DEVICE1";
        repo.mark_device_trusted(user_id, device_id, user_id).await.unwrap();

        let trusted_devices = repo.get_trusted_devices(user_id).await.unwrap();
        assert!(trusted_devices.contains(&device_id.to_string()));

        // Test revoking trust
        repo.revoke_device_trust(user_id, device_id).await.unwrap();
        let trusted_after_revoke = repo.get_trusted_devices(user_id).await.unwrap();
        assert!(!trusted_after_revoke.contains(&device_id.to_string()));
    }

    #[tokio::test]
    async fn test_key_backup_repository() {
        let db = setup_test_db().await;
        let repo = KeyBackupRepository::new(db);

        let user_id = "@alice:example.com";
        let algorithm = "m.megolm_backup.v1.curve25519-aes-sha2";
        let auth_data = serde_json::json!({
            "public_key": "backup_public_key",
            "signatures": {}
        });

        // Test creating backup version
        let version = repo.create_backup_version(user_id, algorithm, &auth_data).await.unwrap();
        assert!(!version.is_empty());

        // Test retrieving backup version
        let retrieved_version = repo.get_backup_version(user_id, &version).await.unwrap();
        assert!(retrieved_version.is_some());
        let backup_version = retrieved_version.unwrap();
        assert_eq!(backup_version.algorithm, algorithm);
        assert_eq!(backup_version.count, 0);

        // Test storing room key
        let room_id = "!room:example.com";
        let session_id = "session123";
        let encrypted_key = EncryptedRoomKey {
            room_id: room_id.to_string(),
            session_id: session_id.to_string(),
            first_message_index: 0,
            forwarded_count: 0,
            is_verified: true,
            session_data: serde_json::json!({"encrypted": "room_key_data"}),
        };

        repo.store_room_key(user_id, &version, room_id, session_id, &encrypted_key)
            .await
            .unwrap();

        // Test retrieving room key
        let retrieved_key =
            repo.get_room_key(user_id, &version, room_id, session_id).await.unwrap();
        assert!(retrieved_key.is_some());
        let room_key = retrieved_key.unwrap();
        assert_eq!(room_key.room_id, room_id);
        assert_eq!(room_key.session_id, session_id);

        // Test getting room keys for a room
        let room_keys = repo.get_room_keys(user_id, &version, Some(room_id)).await.unwrap();
        assert_eq!(room_keys.len(), 1);

        // Test getting all room keys
        let all_room_keys = repo.get_room_keys(user_id, &version, None).await.unwrap();
        assert_eq!(all_room_keys.len(), 1);

        // Test backup statistics
        let stats = repo.get_backup_statistics(user_id, &version).await.unwrap();
        assert_eq!(stats.total_keys, 1);
        assert_eq!(stats.total_rooms, 1);
        assert!(stats.last_backup.is_some());

        // Test backup integrity
        let integrity = repo.verify_backup_integrity(user_id, &version).await.unwrap();
        assert!(integrity);

        // Test deleting room key
        repo.delete_room_key(user_id, &version, room_id, session_id).await.unwrap();
        let deleted_key = repo.get_room_key(user_id, &version, room_id, session_id).await.unwrap();
        assert!(deleted_key.is_none());

        // Test deleting backup version
        repo.delete_backup_version(user_id, &version).await.unwrap();
        let deleted_version = repo.get_backup_version(user_id, &version).await.unwrap();
        assert!(deleted_version.is_none());
    }

    #[tokio::test]
    async fn test_crypto_service_integration() {
        let db = setup_test_db().await;
        let service = CryptoService::new(db);

        let user_id = "@alice:example.com";
        let device_id = "DEVICE1";

        // Test cross-signing setup
        let cross_signing_keys = CrossSigningKeys {
            master_key: Some(CrossSigningKey {
                user_id: user_id.to_string(),
                usage: vec!["master".to_string()],
                keys: {
                    let mut keys = HashMap::new();
                    keys.insert("ed25519:master".to_string(), "master_key".to_string());
                    keys
                },
                signatures: Some(HashMap::new()),
            }),
            self_signing_key: Some(CrossSigningKey {
                user_id: user_id.to_string(),
                usage: vec!["self_signing".to_string()],
                keys: {
                    let mut keys = HashMap::new();
                    keys.insert("ed25519:self_signing".to_string(), "self_signing_key".to_string());
                    keys
                },
                signatures: Some(HashMap::new()),
            }),
            user_signing_key: None,
        };

        service
            .setup_cross_signing(user_id, &cross_signing_keys, device_id)
            .await
            .unwrap();

        // Test verifying cross-signing setup
        let setup_verified = service.verify_cross_signing_setup(user_id).await.unwrap();
        assert!(setup_verified);

        // Test creating key backup
        let auth_data = serde_json::json!({
            "public_key": "backup_key",
            "signatures": {}
        });
        let backup_version = service
            .create_key_backup(user_id, "m.megolm_backup.v1.curve25519-aes-sha2", &auth_data)
            .await
            .unwrap();

        // Test backup info
        let (version_info, stats) =
            service.get_backup_info(user_id, &backup_version).await.unwrap();
        assert_eq!(version_info.algorithm, "m.megolm_backup.v1.curve25519-aes-sha2");
        assert_eq!(stats.total_keys, 0);

        // Test cleanup
        let cleaned = service.cleanup_old_crypto_data(30).await.unwrap();
        assert_eq!(cleaned, 0); // No old data to clean yet
    }

    #[tokio::test]
    async fn test_crypto_key_validation() {
        let db = setup_test_db().await;
        let repo = CryptoRepository::new(db);

        let signature = Signature {
            signature: "valid_signature_data".to_string(),
            key_id: "test_key".to_string(),
            algorithm: "ed25519".to_string(),
        };

        let key_data = serde_json::json!({
            "user_id": "@alice:example.com",
            "device_id": "DEVICE1",
            "keys": {
                "ed25519:DEVICE1": "device_key_data"
            }
        });

        let signing_key = "signing_key_base64_data";

        // Test key signature validation
        let is_valid = repo
            .validate_key_signature(&key_data, &signature, signing_key)
            .await
            .unwrap();
        assert!(is_valid); // Mock implementation returns true for valid format

        // Test signature generation
        let generated_sig = repo.generate_key_signature(&key_data, signing_key).await.unwrap();
        assert_eq!(generated_sig.algorithm, "ed25519");
        assert!(!generated_sig.signature.is_empty());
    }

    #[tokio::test]
    async fn test_cleanup_expired_keys() {
        let db = setup_test_db().await;
        let repo = CryptoRepository::new(db);

        let user_id = "@alice:example.com";
        let device_id = "DEVICE1";

        // Store and claim a one-time key
        let one_time_key = OneTimeKey {
            key_id: "test_key".to_string(),
            key: "key_data".to_string(),
            algorithm: "signed_curve25519".to_string(),
            signatures: None,
            created_at: Utc::now() - chrono::Duration::days(2), // 2 days ago
        };

        repo.store_one_time_key(user_id, device_id, "test_key", &one_time_key)
            .await
            .unwrap();
        repo.claim_one_time_key(user_id, device_id, "signed_curve25519")
            .await
            .unwrap();

        // Cleanup keys older than 1 day
        let cutoff = Utc::now() - chrono::Duration::days(1);
        let cleaned_count = repo.cleanup_expired_keys(cutoff).await.unwrap();
        assert_eq!(cleaned_count, 1);
    }
}
