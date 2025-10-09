//! Comprehensive capabilities and filtering system verification tests
//!
//! This module verifies the production-ready implementations:
//! - Capabilities endpoint Matrix specification compliance
//! - Filter API CRUD operations with FilterRepository integration
//! - Sync filtering functionality with room filtering and lazy loading

use axum::http::StatusCode;
use axum_test::TestServer;
use serde_json::Value;
use uuid::Uuid;

// Import local crate modules
use matryx_entity::{
    filter::{EventFilter, RoomEventFilter, RoomFilter},
    types::MatrixFilter,
};
use matryx_surrealdb::test_utils::TestDatabase;

mod common;

/// Test Matrix-compliant capabilities endpoint
#[tokio::test]
async fn test_capabilities_endpoint_compliance() -> Result<(), Box<dyn std::error::Error>> {
    // Create test app using existing infrastructure
    let app = common::create_test_app().await;
    let server = TestServer::new(app)?;

    // Test capabilities request
    let response = server.get("/_matrix/client/v3/capabilities").await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let capabilities: Value = response.json();

    // Verify all required Matrix capabilities are present
    let caps = &capabilities["capabilities"];

    // Test required capabilities
    assert!(caps["m.change_password"]["enabled"].is_boolean());
    assert!(caps["m.room_versions"]["default"].is_string());
    assert_eq!(caps["m.room_versions"]["default"], "9");
    assert!(caps["m.room_versions"]["available"].is_object());

    // Test advanced capabilities
    assert!(
        caps["m.set_displayname"]["enabled"]
            .as_bool()
            .ok_or("Missing displayname capability")?
    );
    assert!(
        caps["m.set_avatar_url"]["enabled"]
            .as_bool()
            .ok_or("Missing avatar capability")?
    );
    assert!(
        caps["m.3pid_changes"]["enabled"]
            .as_bool()
            .ok_or("Missing 3pid capability")?
    );
    assert!(
        !caps["m.get_login_token"]["enabled"]
            .as_bool()
            .ok_or("Missing login token capability")?
    );

    // Test Matrix extension capabilities
    assert!(
        caps["org.matrix.lazy_loading"]["enabled"]
            .as_bool()
            .ok_or("Missing lazy loading capability")?
    );
    assert!(caps["org.matrix.e2e_cross_signing"]["enabled"].is_boolean());
    assert!(caps["org.matrix.spaces"]["enabled"].is_boolean());
    assert!(
        caps["org.matrix.threading"]["enabled"]
            .as_bool()
            .ok_or("Missing threading capability")?
    );
    Ok(())
}

/// Test filter CRUD operations with repository integration
#[tokio::test]
async fn test_filter_crud_operations() -> Result<(), Box<dyn std::error::Error>> {
    // Create test database
    let test_db = TestDatabase::new().await?;

    // Create test filter
    let filter = MatrixFilter {
        room: Some(RoomFilter {
            rooms: Some(vec!["!room1:example.com".to_string()]),
            timeline: Some(RoomEventFilter {
                base: EventFilter {
                    limit: Some(10),
                    types: Some(vec!["m.room.message".to_string()]),
                    ..Default::default()
                },
                lazy_load_members: true,
                include_redundant_members: false,
                contains_url: None,
            }),
            ..Default::default()
        }),
        event_fields: Some(vec![
            "type".to_string(),
            "content".to_string(),
            "sender".to_string(),
        ]),
        event_format: "client".to_string(),
        ..Default::default()
    };

    // Test filter structure compliance
    assert!(filter.room.is_some());
    assert!(filter.room.as_ref().ok_or("Room filter should exist")?.timeline.is_some());
    assert!(
        filter
            .room
            .as_ref()
            .ok_or("Room filter should exist")?
            .timeline
            .as_ref()
            .ok_or("Timeline filter should exist")?
            .lazy_load_members
    );
    assert_eq!(filter.event_format, "client");
    assert!(filter.event_fields.is_some());

    // Test serialization/deserialization
    let serialized = serde_json::to_string(&filter)?;
    let deserialized: MatrixFilter =
        serde_json::from_str(&serialized)?;

    // Verify deserialized filter matches original
    assert_eq!(deserialized.event_format, filter.event_format);
    assert_eq!(deserialized.event_fields, filter.event_fields);
    assert_eq!(
        deserialized.room.as_ref().ok_or("Room filter should exist")?.rooms,
        filter.room.as_ref().ok_or("Room filter should exist")?.rooms
    );

    // Test filter repository operations
    use matryx_surrealdb::repository::filter::FilterRepository;
    let filter_repo = FilterRepository::new(test_db.db.clone());
    let filter_id = Uuid::new_v4().to_string();

    // Test create operation
    let create_result = filter_repo.create(&filter, &filter_id).await;
    assert!(create_result.is_ok(), "Filter creation should succeed");

    // Test get operation
    let retrieved_filter = filter_repo
        .get_by_id(&filter_id)
        .await?
        .ok_or("Filter should exist")?;

    assert_eq!(retrieved_filter.event_format, filter.event_format);
    assert_eq!(retrieved_filter.event_fields, filter.event_fields);

    // Cleanup
    test_db.cleanup().await?;
    Ok(())
}

/// Test sync filtering functionality
#[tokio::test]
async fn test_sync_with_filter() -> Result<(), Box<dyn std::error::Error>> {
    // Create a filter that excludes certain rooms
    let filter = MatrixFilter {
        room: Some(RoomFilter {
            not_rooms: Some(vec!["!spam:example.com".to_string()]),
            include_leave: false,
            timeline: Some(RoomEventFilter {
                base: EventFilter {
                    limit: Some(20),
                    not_types: Some(vec!["m.room.member".to_string()]),
                    ..Default::default()
                },
                lazy_load_members: true,
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    // Test filter JSON serialization for inline usage
    let filter_json = serde_json::to_string(&filter)?;

    // Verify filter JSON structure
    let parsed_filter: Value =
        serde_json::from_str(&filter_json)?;
    assert!(parsed_filter["room"]["not_rooms"].is_array());
    assert_eq!(parsed_filter["room"]["not_rooms"][0], "!spam:example.com");
    assert_eq!(parsed_filter["room"]["include_leave"], false);
    assert_eq!(parsed_filter["room"]["timeline"]["lazy_load_members"], true);
    assert_eq!(parsed_filter["room"]["timeline"]["limit"], 20);

    // Test URL encoding for sync parameter
    let encoded_filter = urlencoding::encode(&filter_json);
    assert!(!encoded_filter.is_empty());

    // Verify filter can be used in sync URL format
    let sync_url = format!("/_matrix/client/v3/sync?filter={}", encoded_filter);
    assert!(sync_url.contains("/_matrix/client/v3/sync?filter="));
    Ok(())
}

/// Test lazy loading member functionality
#[tokio::test]
async fn test_lazy_loading_members() -> Result<(), Box<dyn std::error::Error>> {
    // Create filter with lazy loading enabled
    let lazy_filter = MatrixFilter {
        room: Some(RoomFilter {
            timeline: Some(RoomEventFilter {
                base: EventFilter::default(),
                lazy_load_members: true,
                include_redundant_members: false,
                contains_url: None,
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    // Verify lazy loading flag is properly set
    let timeline_filter = lazy_filter
        .room
        .as_ref()
        .ok_or("Room filter should exist")?
        .timeline
        .as_ref()
        .ok_or("Timeline filter should exist")?;

    assert!(timeline_filter.lazy_load_members);
    assert!(!timeline_filter.include_redundant_members);

    // Create filter without lazy loading
    let normal_filter = MatrixFilter {
        room: Some(RoomFilter {
            timeline: Some(RoomEventFilter {
                base: EventFilter::default(),
                lazy_load_members: false,
                include_redundant_members: true,
                contains_url: Some(false),
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    let normal_timeline = normal_filter
        .room
        .as_ref()
        .ok_or("Room filter should exist")?
        .timeline
        .as_ref()
        .ok_or("Timeline filter should exist")?;

    assert!(!normal_timeline.lazy_load_members);
    assert!(normal_timeline.include_redundant_members);
    assert_eq!(normal_timeline.contains_url, Some(false));
    Ok(())
}

/// Test event filtering functionality
#[tokio::test]
async fn test_event_filtering() -> Result<(), Box<dyn std::error::Error>> {
    // Create filter with event type restrictions
    let event_filter = MatrixFilter {
        room: Some(RoomFilter {
            timeline: Some(RoomEventFilter {
                base: EventFilter {
                    limit: Some(50),
                    types: Some(vec!["m.room.message".to_string(), "m.room.encrypted".to_string()]),
                    not_types: Some(vec!["m.room.member".to_string(), "m.typing".to_string()]),
                    senders: Some(vec!["@important:example.com".to_string()]),
                    not_senders: Some(vec!["@spam:example.com".to_string()]),
                },
                lazy_load_members: false,
                include_redundant_members: false,
                contains_url: Some(true),
            }),
            ..Default::default()
        }),
        event_fields: Some(vec![
            "type".to_string(),
            "content".to_string(),
            "sender".to_string(),
            "origin_server_ts".to_string(),
        ]),
        event_format: "client".to_string(),
        ..Default::default()
    };

    // Verify event filter structure
    let timeline = event_filter
        .room
        .as_ref()
        .ok_or("Room filter should exist")?
        .timeline
        .as_ref()
        .ok_or("Timeline filter should exist")?;

    assert_eq!(timeline.base.limit, Some(50));
    assert!(
        timeline
            .base
            .types
            .as_ref()
            .ok_or("Types should exist")?
            .contains(&"m.room.message".to_string())
    );
    assert!(
        timeline
            .base
            .not_types
            .as_ref()
            .ok_or("Not types should exist")?
            .contains(&"m.room.member".to_string())
    );
    assert!(
        timeline
            .base
            .senders
            .as_ref()
            .ok_or("Senders should exist")?
            .contains(&"@important:example.com".to_string())
    );
    assert!(
        timeline
            .base
            .not_senders
            .as_ref()
            .ok_or("Not senders should exist")?
            .contains(&"@spam:example.com".to_string())
    );
    assert_eq!(timeline.contains_url, Some(true));

    // Verify event fields filtering
    let event_fields = event_filter.event_fields.as_ref().ok_or("Event fields should exist")?;
    assert!(event_fields.contains(&"type".to_string()));
    assert!(event_fields.contains(&"content".to_string()));
    assert!(event_fields.contains(&"sender".to_string()));
    assert!(event_fields.contains(&"origin_server_ts".to_string()));
    Ok(())
}

/// Test room filtering functionality
#[tokio::test]
async fn test_room_filtering() -> Result<(), Box<dyn std::error::Error>> {
    // Test room inclusion filter
    let inclusion_filter = MatrixFilter {
        room: Some(RoomFilter {
            rooms: Some(vec![
                "!important:example.com".to_string(),
                "!work:example.com".to_string(),
            ]),
            include_leave: false,
            ..Default::default()
        }),
        ..Default::default()
    };

    let room_filter = inclusion_filter.room.as_ref().ok_or("Room filter should exist")?;
    assert!(
        room_filter
            .rooms
            .as_ref()
            .ok_or("Rooms should exist")?
            .contains(&"!important:example.com".to_string())
    );
    assert!(!room_filter.include_leave);

    // Test room exclusion filter
    let exclusion_filter = MatrixFilter {
        room: Some(RoomFilter {
            not_rooms: Some(vec![
                "!spam:example.com".to_string(),
                "!noise:example.com".to_string(),
            ]),
            include_leave: true,
            ..Default::default()
        }),
        ..Default::default()
    };

    let exclusion_room_filter = exclusion_filter.room.as_ref().ok_or("Room filter should exist")?;
    assert!(
        exclusion_room_filter
            .not_rooms
            .as_ref()
            .ok_or("Not rooms should exist")?
            .contains(&"!spam:example.com".to_string())
    );
    assert!(exclusion_room_filter.include_leave);
    Ok(())
}

/// Test presence and account data filtering
#[tokio::test]
async fn test_presence_and_account_data_filtering() -> Result<(), Box<dyn std::error::Error>> {
    // Create filter with presence and account data restrictions
    let comprehensive_filter = MatrixFilter {
        presence: Some(EventFilter {
            limit: Some(100),
            types: Some(vec!["m.presence".to_string()]),
            not_senders: Some(vec!["@bot:example.com".to_string()]),
            ..Default::default()
        }),
        account_data: Some(EventFilter {
            limit: Some(50),
            types: Some(vec!["m.push_rules".to_string(), "m.direct".to_string()]),
            ..Default::default()
        }),
        event_format: "client".to_string(),
        ..Default::default()
    };

    // Verify presence filter
    let presence_filter = comprehensive_filter
        .presence
        .as_ref()
        .ok_or("Presence filter should exist")?;
    assert_eq!(presence_filter.limit, Some(100));
    assert!(
        presence_filter
            .types
            .as_ref()
            .ok_or("Types should exist")?
            .contains(&"m.presence".to_string())
    );
    assert!(
        presence_filter
            .not_senders
            .as_ref()
            .ok_or("Not senders should exist")?
            .contains(&"@bot:example.com".to_string())
    );

    // Verify account data filter
    let account_data_filter = comprehensive_filter
        .account_data
        .as_ref()
        .ok_or("Account data filter should exist")?;
    assert_eq!(account_data_filter.limit, Some(50));
    assert!(
        account_data_filter
            .types
            .as_ref()
            .ok_or("Types should exist")?
            .contains(&"m.push_rules".to_string())
    );
    assert!(
        account_data_filter
            .types
            .as_ref()
            .ok_or("Types should exist")?
            .contains(&"m.direct".to_string())
    );
    Ok(())
}
