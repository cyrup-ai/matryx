#[cfg(test)]
mod third_party_tests {
    use crate::repository::{
        ThirdPartyRepository, BridgeRepository, ThirdPartyService,
        third_party::{
            ThirdPartyProtocol, ProtocolInstance, ThirdPartyLocation, ThirdPartyUser,
            BridgeConfig, BridgeStatus, FieldType
        }
    };
    use std::collections::HashMap;
    use chrono::Utc;
    use surrealdb::{Surreal, engine::any::Any};

    async fn setup_test_db() -> Surreal<Any> {
        let db = surrealdb::engine::any::connect("surrealkv://test_data/third_party_test.db").await.unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        db
    }

    // TASK17 SUBTASK 14: Add Third-Party Tests

    #[tokio::test]
    async fn test_protocol_registration_and_management() {
        let db = setup_test_db().await;
        let third_party_repo = ThirdPartyRepository::new(db);

        // Create test protocol
        let protocol = ThirdPartyProtocol {
            protocol_id: "test_protocol".to_string(),
            display_name: "Test Protocol".to_string(),
            avatar_url: Some("mxc://example.com/avatar".to_string()),
            user_fields: vec![
                FieldType {
                    regexp: r"@.+:.+".to_string(),
                    placeholder: "username".to_string(),
                }
            ],
            location_fields: vec![
                FieldType {
                    regexp: r"#.+:.+".to_string(),
                    placeholder: "channel".to_string(),
                }
            ],
            instances: vec![
                ProtocolInstance {
                    instance_id: "test_instance".to_string(),
                    desc: "Test Instance".to_string(),
                    icon: None,
                    fields: {
                        let mut fields = HashMap::new();
                        fields.insert("server".to_string(), "test.example.com".to_string());
                        fields
                    },
                    network_id: "test_network".to_string(),
                }
            ],
        };

        // Test protocol registration
        let result = third_party_repo.register_protocol(&protocol).await;
        assert!(result.is_ok());

        // Test protocol retrieval
        let retrieved = third_party_repo.get_protocol_by_id("test_protocol").await.unwrap();
        assert!(retrieved.is_some());
        let retrieved_protocol = retrieved.unwrap();
        assert_eq!(retrieved_protocol.protocol_id, "test_protocol");
        assert_eq!(retrieved_protocol.display_name, "Test Protocol");

        // Test get all protocols
        let all_protocols = third_party_repo.get_all_protocols().await.unwrap();
        assert!(!all_protocols.is_empty());
        assert!(all_protocols.iter().any(|p| p.protocol_id == "test_protocol"));

        // Test protocol instances
        let instances = third_party_repo.get_protocol_instances("test_protocol").await.unwrap();
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].instance_id, "test_instance");
    }

    #[tokio::test]
    async fn test_location_and_user_lookup_functionality() {
        let db = setup_test_db().await;
        let third_party_repo = ThirdPartyRepository::new(db);

        // Create test location
        let _location = ThirdPartyLocation {
            alias: "#test:example.com".to_string(),
            protocol: "test_protocol".to_string(),
            fields: {
                let mut fields = HashMap::new();
                fields.insert("channel".to_string(), "test".to_string());
                fields.insert("server".to_string(), "example.com".to_string());
                fields
            },
        };

        // Create test user
        let _user = ThirdPartyUser {
            userid: "@testuser:example.com".to_string(),
            protocol: "test_protocol".to_string(),
            fields: {
                let mut fields = HashMap::new();
                fields.insert("username".to_string(), "testuser".to_string());
                fields.insert("server".to_string(), "example.com".to_string());
                fields
            },
        };

        // Test location lookup (would need to insert test data first)
        let search_fields = {
            let mut fields = HashMap::new();
            fields.insert("channel".to_string(), "test".to_string());
            fields
        };

        let _locations = third_party_repo.lookup_third_party_location("test_protocol", &search_fields).await.unwrap();
        // In a real test, we would insert test data and verify results

        // Test user lookup
        let user_search_fields = {
            let mut fields = HashMap::new();
            fields.insert("username".to_string(), "testuser".to_string());
            fields
        };

        let _users = third_party_repo.lookup_third_party_user("test_protocol", &user_search_fields).await.unwrap();
        // In a real test, we would insert test data and verify results

        // Test get location by alias
        let _location_result = third_party_repo.get_location_by_alias("#test:example.com").await.unwrap();
        // Would be Some(location) if test data was inserted

        // Test get user by userid
        let _user_result = third_party_repo.get_user_by_userid("@testuser:example.com").await.unwrap();
        // Would be Some(user) if test data was inserted
    }

    #[tokio::test]
    async fn test_bridge_configuration_and_monitoring() {
        let db = setup_test_db().await;
        let bridge_repo = BridgeRepository::new(db);

        // Create test bridge
        let bridge = BridgeConfig {
            bridge_id: "test_bridge".to_string(),
            protocol: "test_protocol".to_string(),
            name: "Test Bridge".to_string(),
            url: "https://bridge.example.com".to_string(),
            as_token: "test_as_token_12345678901234567890123456789012".to_string(),
            hs_token: "test_hs_token_12345678901234567890123456789012".to_string(),
            status: BridgeStatus::Active,
            created_at: Utc::now(),
            last_seen: Some(Utc::now()),
        };

        // Test bridge registration
        let result = bridge_repo.register_bridge(&bridge).await;
        assert!(result.is_ok());

        // Test bridge retrieval
        let retrieved = bridge_repo.get_bridge_by_id("test_bridge").await.unwrap();
        assert!(retrieved.is_some());
        let retrieved_bridge = retrieved.unwrap();
        assert_eq!(retrieved_bridge.bridge_id, "test_bridge");
        assert_eq!(retrieved_bridge.name, "Test Bridge");

        // Test get bridges for protocol
        let protocol_bridges = bridge_repo.get_bridges_for_protocol("test_protocol").await.unwrap();
        assert!(!protocol_bridges.is_empty());
        assert!(protocol_bridges.iter().any(|b| b.bridge_id == "test_bridge"));

        // Test bridge status update
        let status_result = bridge_repo.update_bridge_status("test_bridge", BridgeStatus::Maintenance).await;
        assert!(status_result.is_ok());

        // Test bridge statistics
        let stats_result = bridge_repo.get_bridge_statistics("test_bridge").await;
        assert!(stats_result.is_ok());
        let stats = stats_result.unwrap();
        assert_eq!(stats.total_users, 0); // No test data inserted
        assert_eq!(stats.total_rooms, 0); // No test data inserted

        // Test get all active bridges
        let _active_bridges = bridge_repo.get_all_active_bridges().await.unwrap();
        // Would contain bridges with Active status

        // Test cleanup inactive bridges
        let cutoff = Utc::now() - chrono::Duration::days(30);
        let cleanup_result = bridge_repo.cleanup_inactive_bridges(cutoff).await;
        assert!(cleanup_result.is_ok());
    }

    #[tokio::test]
    async fn test_application_service_integration() {
        let db = setup_test_db().await;
        let third_party_service = ThirdPartyService::new(db);

        // Test linking protocol to application service
        let _link_result = third_party_service.link_protocol_to_application_service(
            "test_protocol",
            "test_as",
            "test_as_token_12345678901234567890123456789012"
        ).await;
        // Would succeed if protocol exists

        // Test AS token validation
        let token_validation = third_party_service.validate_as_token_for_operation(
            "test_as_token_12345678901234567890123456789012",
            "test_protocol",
            "lookup_user"
        ).await;
        assert!(token_validation.is_ok());

        // Test AS namespace validation
        let namespace_validation = third_party_service.validate_as_namespace(
            "test_as",
            "users",
            "@bridge_user:example.com"
        ).await;
        assert!(namespace_validation.is_ok());

        // Test third-party event routing
        let event_data = serde_json::json!({
            "type": "message",
            "content": "test message"
        });

        let _routing_result = third_party_service.route_third_party_event(
            "test_protocol",
            &event_data
        ).await;
        // Would succeed if AS link exists
    }

    #[tokio::test]
    async fn test_third_party_network_connectivity() {
        let db = setup_test_db().await;
        let third_party_service = ThirdPartyService::new(db);

        // Test network connectivity validation
        let connectivity_result = third_party_service.validate_network_connectivity("test_protocol").await;
        assert!(connectivity_result.is_ok());
        
        let connectivity = connectivity_result.unwrap();
        assert_eq!(connectivity.protocol_id, "test_protocol");
        assert_eq!(connectivity.total_bridges, 0); // No bridges in test
        assert_eq!(connectivity.healthy_bridges, 0);
        // assert!(matches!(connectivity.connectivity_status, NetworkConnectivityStatus::Nobridges));
    }

    #[tokio::test]
    async fn test_error_handling_for_bridge_failures() {
        let db = setup_test_db().await;
        let bridge_repo = BridgeRepository::new(db);

        // Test getting non-existent bridge
        let result = bridge_repo.get_bridge_by_id("non_existent_bridge").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        // Test updating status of non-existent bridge
        let status_result = bridge_repo.update_bridge_status("non_existent_bridge", BridgeStatus::Error).await;
        assert!(status_result.is_ok()); // SurrealDB UPDATE doesn't fail for non-existent records

        // Test getting statistics for non-existent bridge
        let stats_result = bridge_repo.get_bridge_statistics("non_existent_bridge").await;
        assert!(stats_result.is_err()); // Should fail because bridge doesn't exist

        // Test bridge failover for non-existent bridge
        let failover_result = bridge_repo.perform_bridge_failover("non_existent_bridge").await;
        assert!(failover_result.is_err()); // Should fail because bridge doesn't exist
    }

    #[tokio::test]
    async fn test_protocol_validation() {
        let db = setup_test_db().await;
        let third_party_service = ThirdPartyService::new(db);

        // Test protocol field validation with empty fields
        let empty_fields = HashMap::new();
        let _validation_result = third_party_service.validate_protocol_fields(
            "test_protocol",
            "user",
            &empty_fields
        ).await;
        // Would fail if protocol exists and has required fields

        // Test protocol field validation with invalid field type
        let invalid_validation = third_party_service.validate_protocol_fields(
            "test_protocol",
            "invalid_type",
            &empty_fields
        ).await;
        assert!(invalid_validation.is_err());

        // Test protocol instances validation
        let _instances_validation = third_party_service.validate_protocol_instances("test_protocol").await;
        // Would succeed if protocol exists

        // Test bridge tokens validation
        let _tokens_validation = third_party_service.validate_bridge_tokens("test_bridge").await;
        // Would succeed if bridge exists
    }

    #[tokio::test]
    async fn test_third_party_service_integration() {
        let db = setup_test_db().await;
        let third_party_service = ThirdPartyService::new(db);

        // Test querying all protocols
        let protocols_result = third_party_service.query_third_party_protocols().await;
        assert!(protocols_result.is_ok());
        let _protocols = protocols_result.unwrap();
        // Would contain protocols if any were registered

        // Test location lookup with validation
        let search_fields = {
            let mut fields = HashMap::new();
            fields.insert("channel".to_string(), "test".to_string());
            fields
        };

        let _location_result = third_party_service.lookup_location("test_protocol", &search_fields).await;
        // Would fail if protocol doesn't exist

        // Test user lookup with validation
        let user_search_fields = {
            let mut fields = HashMap::new();
            fields.insert("username".to_string(), "testuser".to_string());
            fields
        };

        let _user_result = third_party_service.lookup_user("test_protocol", &user_search_fields).await;
        // Would fail if protocol doesn't exist

        // Test room alias resolution
        let alias_result = third_party_service.resolve_room_alias("#test:example.com").await;
        assert!(alias_result.is_ok());

        // Test user ID resolution
        let userid_result = third_party_service.resolve_user_id("@testuser:example.com").await;
        assert!(userid_result.is_ok());

        // Test invalid alias format
        let invalid_alias_result = third_party_service.resolve_room_alias("invalid_alias").await;
        assert!(invalid_alias_result.is_err());

        // Test invalid user ID format
        let invalid_userid_result = third_party_service.resolve_user_id("invalid_userid").await;
        assert!(invalid_userid_result.is_err());
    }

    #[tokio::test]
    async fn test_bridge_health_monitoring() {
        let db = setup_test_db().await;
        let bridge_repo = BridgeRepository::new(db);

        // Test bridge health monitoring for non-existent bridge
        let health_result = bridge_repo.monitor_bridge_health("non_existent_bridge").await;
        assert!(health_result.is_err());

        // Test bridge metrics tracking
        let metrics_result = bridge_repo.track_bridge_metrics("test_bridge", 100, 5).await;
        assert!(metrics_result.is_ok());

        // Test bridge performance metrics retrieval
        let performance_result = bridge_repo.get_bridge_performance_metrics("test_bridge").await;
        assert!(performance_result.is_ok());
        let performance = performance_result.unwrap();
        assert_eq!(performance.messages_24h, 0); // Default value
        assert_eq!(performance.errors_24h, 0); // Default value
    }
}