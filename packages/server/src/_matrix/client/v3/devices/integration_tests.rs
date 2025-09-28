#[cfg(test)]
mod integration_tests {
    use super::{TrustLevel, ClientDeviceInfo};
    use crate::federation::device_management::{DeviceListCache, DeviceListUpdate};
    use chrono::Utc;
    use std::collections::HashMap;
    use matryx_entity::DeviceKeys;

    #[tokio::test]
    async fn test_device_list_update_application() {
        let mut cache = DeviceListCache {
            devices: HashMap::new(),
            master_key: None,
            self_signing_key: None,
            stream_id: 0,
            last_updated: Utc::now(),
        };

        let update = DeviceListUpdate {
            user_id: "@test:example.com".to_string(),
            device_id: "NEW_DEVICE".to_string(),
            stream_id: 1,
            prev_id: vec![0],
            deleted: false,
            device_display_name: Some("New Device".to_string()),
            keys: None,
        };

        let result = cache.apply_update(&update).await;
        assert!(result.is_ok());
        assert_eq!(cache.stream_id, 1);
        assert!(cache.devices.contains_key("NEW_DEVICE"));
    }

    #[tokio::test]
    async fn test_device_list_update_deletion() {
        let mut cache = DeviceListCache {
            devices: HashMap::new(),
            master_key: None,
            self_signing_key: None,
            stream_id: 1,
            last_updated: Utc::now(),
        };

        // First add a device
        let add_update = DeviceListUpdate {
            user_id: "@test:example.com".to_string(),
            device_id: "DEVICE_TO_DELETE".to_string(),
            stream_id: 2,
            prev_id: vec![1],
            deleted: false,
            device_display_name: Some("Device to Delete".to_string()),
            keys: None,
        };

        let result = cache.apply_update(&add_update).await;
        assert!(result.is_ok());
        assert!(cache.devices.contains_key("DEVICE_TO_DELETE"));

        // Then delete it
        let delete_update = DeviceListUpdate {
            user_id: "@test:example.com".to_string(),
            device_id: "DEVICE_TO_DELETE".to_string(),
            stream_id: 3,
            prev_id: vec![2],
            deleted: true,
            device_display_name: None,
            keys: None,
        };

        let result = cache.apply_update(&delete_update).await;
        assert!(result.is_ok());
        assert_eq!(cache.stream_id, 3);
        assert!(!cache.devices.contains_key("DEVICE_TO_DELETE"));
    }

    #[tokio::test]
    async fn test_device_list_update_ordering() {
        let mut cache = DeviceListCache {
            devices: HashMap::new(),
            master_key: None,
            self_signing_key: None,
            stream_id: 0,
            last_updated: Utc::now(),
        };

        // Try to apply an update that depends on a missing previous update
        let out_of_order_update = DeviceListUpdate {
            user_id: "@test:example.com".to_string(),
            device_id: "OUT_OF_ORDER".to_string(),
            stream_id: 5,
            prev_id: vec![3, 4], // These don't exist yet
            deleted: false,
            device_display_name: Some("Out of Order".to_string()),
            keys: None,
        };

        let result = cache.apply_update(&out_of_order_update).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::federation::device_management::DeviceError::MissingPreviousUpdate
        ));
    }

    #[tokio::test]
    async fn test_trust_level_default() {
        let trust_level = TrustLevel::default();
        assert_eq!(trust_level, TrustLevel::Unverified);
    }

    #[tokio::test]
    async fn test_device_info_serialization() {
        let device_info = ClientDeviceInfo {
            device_id: "TEST123".to_string(),
            display_name: Some("Test Device".to_string()),
            last_seen_ip: Some("192.168.1.1".to_string()),
            last_seen_ts: Some(1234567890),
            user_id: "@test:example.com".to_string(),
            created_ts: 1234567890,
            device_keys: None,
            trust_level: TrustLevel::Verified,
            is_deleted: false,
        };

        let serialized = serde_json::to_string(&device_info).expect("Serialization should work");
        assert!(serialized.contains("verified"));
        assert!(serialized.contains("TEST123"));
        assert!(serialized.contains("@test:example.com"));

        let deserialized: ClientDeviceInfo =
            serde_json::from_str(&serialized).expect("Deserialization should work");
        assert_eq!(deserialized.device_id, "TEST123");
        assert_eq!(deserialized.trust_level, TrustLevel::Verified);
    }

    #[tokio::test]
    async fn test_device_keys_structure() {
        let mut keys = HashMap::new();
        keys.insert("curve25519:AAAAHQ".to_string(), "base64+key".to_string());

        let mut signatures = HashMap::new();
        let mut user_sigs = HashMap::new();
        user_sigs.insert("ed25519:AAAAHQ".to_string(), "base64+sig".to_string());
        signatures.insert("@test:example.com".to_string(), user_sigs);

        let device_keys = DeviceKeys {
            algorithms: vec!["m.olm.v1.curve25519-aes-sha2".to_string()],
            device_id: "TEST123".to_string(),
            keys,
            signatures,
            user_id: "@test:example.com".to_string(),
        };

        let serialized = serde_json::to_string(&device_keys).expect("Serialization should work");
        let deserialized: DeviceKeys =
            serde_json::from_str(&serialized).expect("Deserialization should work");

        assert_eq!(deserialized.device_id, "TEST123");
        assert_eq!(deserialized.user_id, "@test:example.com");
        assert!(!deserialized.algorithms.is_empty());
    }

    // Mock federation test scenario
    #[tokio::test]
    async fn test_federation_device_sync() {
        // This would be a more complex integration test in a real scenario
        // For now, we'll test the data structures work correctly together

        let device_update = DeviceListUpdate {
            user_id: "@alice:server1.com".to_string(),
            device_id: "ALICE_DEVICE".to_string(),
            stream_id: 1,
            prev_id: vec![0],
            deleted: false,
            device_display_name: Some("Alice's Phone".to_string()),
            keys: None,
        };

        let mut cache = DeviceListCache::new();
        let result = cache.apply_update(&device_update).await;

        assert!(result.is_ok());
        assert_eq!(cache.devices.len(), 1);

        let device = cache.devices.get("ALICE_DEVICE").expect("Device should exist");
        assert_eq!(device.user_id, "@alice:server1.com");
        assert_eq!(device.display_name, Some("Alice's Phone".to_string()));
    }
}
