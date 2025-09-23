#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::error::RepositoryError;
    use chrono::Utc;
    use matryx_entity::types::{Device, DeviceKey};
    use std::collections::HashMap;
    use surrealdb::{Surreal, engine::any::Any};

    async fn create_test_device_repo() -> DeviceRepository {
        let db = surrealdb::engine::any::connect("surrealkv://test_data/device_test.db").await.expect("Failed to connect to database");
        db.use_ns("test")
            .use_db("test")
            .await
            .expect("Failed to select namespace/database");
        DeviceRepository::new(db)
    }

    fn create_test_device() -> Device {
        Device {
            device_id: "TEST_DEVICE".to_string(),
            user_id: "@test:example.com".to_string(),
            display_name: Some("Test Device".to_string()),
            last_seen_ip: Some("192.168.1.1".to_string()),
            last_seen_ts: Some(1234567890),
            created_at: Utc::now(),
            hidden: Some(false),
            device_keys: None,
            one_time_keys: None,
            fallback_keys: None,
            user_agent: None,
            initial_device_display_name: Some("Test Device".to_string()),
        }
    }

    fn create_test_device_key() -> DeviceKey {
        let mut keys = HashMap::new();
        keys.insert("curve25519:AAAAHQ".to_string(), "base64+curve25519+key".to_string());
        keys.insert("ed25519:AAAAHQ".to_string(), "base64+ed25519+key".to_string());

        let mut signatures = HashMap::new();
        let mut user_sigs = HashMap::new();
        user_sigs.insert("ed25519:AAAAHQ".to_string(), "base64+signature".to_string());
        signatures.insert("@test:example.com".to_string(), user_sigs);

        DeviceKey {
            user_id: "@test:example.com".to_string(),
            device_id: "TEST_DEVICE".to_string(),
            algorithms: vec!["m.olm.v1.curve25519-aes-sha2".to_string()],
            keys,
            signatures,
            unsigned: None,
        }
    }

    #[tokio::test]
    async fn test_device_creation_with_metadata() {
        let device_repo = create_test_device_repo().await;
        let device_info = create_test_device();
        let device_key = Some(create_test_device_key());

        let result = device_repo
            .create_device_with_metadata(device_info.clone(), device_key)
            .await;
        assert!(result.is_ok());

        let created = result.expect("Device creation should succeed");
        assert_eq!(created.device_id, "TEST_DEVICE");
        assert_eq!(created.user_id, "@test:example.com");
        assert_eq!(created.display_name, Some("Test Device".to_string()));
    }

    #[tokio::test]
    async fn test_device_creation_without_keys() {
        let device_repo = create_test_device_repo().await;
        let device_info = create_test_device();

        let result = device_repo.create_device_with_metadata(device_info.clone(), None).await;
        assert!(result.is_ok());

        let created = result.expect("Device creation should succeed");
        assert_eq!(created.device_id, "TEST_DEVICE");
        assert_eq!(created.user_id, "@test:example.com");
    }

    #[tokio::test]
    async fn test_device_activity_update() {
        let device_repo = create_test_device_repo().await;
        let device_info = create_test_device();

        // First create a device
        let _created = device_repo
            .create_device_with_metadata(device_info.clone(), None)
            .await
            .expect("Device creation should succeed");

        // Then update its activity
        let result = device_repo
            .update_device_activity(
                "TEST_DEVICE",
                "@test:example.com",
                Some("10.0.0.1".to_string()),
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_devices_for_users() {
        let device_repo = create_test_device_repo().await;

        // Create multiple devices for different users
        let device1 = Device {
            device_id: "DEVICE1".to_string(),
            user_id: "@user1:example.com".to_string(),
            display_name: Some("Device 1".to_string()),
            last_seen_ip: None,
            last_seen_ts: None,
            created_at: Utc::now(),
            hidden: Some(false),
            device_keys: None,
            one_time_keys: None,
            fallback_keys: None,
            user_agent: None,
            initial_device_display_name: Some("Device 1".to_string()),
        };

        let device2 = Device {
            device_id: "DEVICE2".to_string(),
            user_id: "@user2:example.com".to_string(),
            display_name: Some("Device 2".to_string()),
            last_seen_ip: None,
            last_seen_ts: None,
            created_at: Utc::now(),
            hidden: Some(false),
            device_keys: None,
            one_time_keys: None,
            fallback_keys: None,
            user_agent: None,
            initial_device_display_name: Some("Device 2".to_string()),
        };

        let _created1 = device_repo
            .create_device_with_metadata(device1, None)
            .await
            .expect("Device 1 creation should succeed");
        let _created2 = device_repo
            .create_device_with_metadata(device2, None)
            .await
            .expect("Device 2 creation should succeed");

        // Query devices for multiple users
        let user_ids = vec![
            "@user1:example.com".to_string(),
            "@user2:example.com".to_string(),
        ];
        let result = device_repo.get_devices_for_users(user_ids).await;

        assert!(result.is_ok());
        let devices_map = result.expect("Getting devices should succeed");
        assert_eq!(devices_map.len(), 2);
        assert!(devices_map.contains_key("@user1:example.com"));
        assert!(devices_map.contains_key("@user2:example.com"));
    }

    #[tokio::test]
    async fn test_get_devices_for_empty_user_list() {
        let device_repo = create_test_device_repo().await;

        let result = device_repo.get_devices_for_users(vec![]).await;
        assert!(result.is_ok());

        let devices_map = result.expect("Getting devices should succeed");
        assert!(devices_map.is_empty());
    }
}
