//! Comprehensive integration tests for Matrix sync filtering functionality
//!
//! This module tests the complete filtering workflows end-to-end to verify
//! that all Matrix filtering requirements are properly implemented.

use axum_test::TestServer;
use matryx_entity::filter::{EventFilter, MatrixFilter, RoomEventFilter, RoomFilter};
use matryx_surrealdb::test_utils::TestDatabase;
use serde_json::{Value, json};

mod common;

/// Test complete sync filtering integration with all filter types
#[tokio::test]
async fn test_complete_sync_filtering_integration() {
    let test_db = TestDatabase::new().await
        .expect("Test setup: failed to create test database for sync filtering tests");
    let app = common::create_test_app_with_db(test_db.db.clone()).await;
    let server = TestServer::new(app)
        .expect("Test setup: failed to create test server for sync filtering integration tests");

    // Create room with diverse event types
    let room_id = "!test:example.com";
    let user_id = "@user:example.com";

    // Create test events in the room
    create_test_room_with_events(
        &test_db,
        room_id,
        vec![
            ("m.room.message", json!({"body": "Hello world"})),
            ("m.room.member", json!({"membership": "join"})),
            ("m.typing", json!({"user_ids": [user_id]})),
            ("m.room.message", json!({"body": "Check out https://matrix.org"})),
            ("m.room.message", json!({"body": "Another message"})),
        ],
    )
    .await
    .expect("Test setup: failed to create test events for sync filtering tests");

    // Test comprehensive filter
    let filter = MatrixFilter {
        event_fields: Some(vec!["type".to_string(), "content.body".to_string()]),
        room: Some(RoomFilter {
            timeline: Some(RoomEventFilter {
                base: EventFilter {
                    types: Some(vec!["m.room.message".to_string()]),
                    limit: Some(5),
                    ..Default::default()
                },
                lazy_load_members: true,
                contains_url: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    // Test sync with comprehensive filtering
    let filter_json = serde_json::to_string(&filter).expect("Failed to serialize filter");
    let encoded_filter = urlencoding::encode(&filter_json);

    let response = server
        .get(&format!("/_matrix/client/v3/sync?filter={}", encoded_filter))
        .add_header("Authorization", "Bearer test_token")
        .await;

    assert_eq!(response.status_code(), 200);
    let sync_response: Value = response.json();

    // Verify filtering was applied correctly
    if let Some(joined_rooms) = sync_response["rooms"]["join"].as_object()
        && let Some(room_response) = joined_rooms.get(room_id)
    {
        let timeline_events = &room_response["timeline"]["events"];

        // Verify event type filtering - only m.room.message events
        if let Some(events) = timeline_events.as_array() {
            for event in events {
                assert_eq!(event["type"], "m.room.message", "Event type filtering failed");
            }

            // Verify contains_url filtering - only events with URLs
            for event in events {
                if let Some(content) = event.get("content")
                    && let Some(body) = content.get("body").and_then(|b| b.as_str())
                {
                    assert!(body.contains("http"), "Contains URL filtering failed");
                }
            }

            // Verify event_fields filtering
            for event in events {
                assert!(event.get("type").is_some(), "Event type field should be present");
                assert!(
                    event.get("content").and_then(|c| c.get("body")).is_some(),
                    "Content body field should be present"
                );
                assert!(event.get("event_id").is_none(), "Event ID field should be filtered out");
            }

            // Verify event limit
            assert!(events.len() <= 5, "Event limit filtering failed");
        }

        // Verify lazy loading reduced membership events
        let state_events = &room_response["state"]["events"];
        if let Some(events) = state_events.as_array() {
            let member_events = events.iter().filter(|e| e["type"] == "m.room.member").count();
            assert!(member_events <= 2, "Lazy loading should reduce membership events");
        }
    }

    test_db.cleanup().await.expect("Failed to cleanup test database");
}

/// Test wildcard event type filtering patterns
#[tokio::test]
async fn test_wildcard_event_type_filtering() {
    let test_db = TestDatabase::new().await
        .expect("Test setup: failed to create test database for sync filtering tests");
    let app = common::create_test_app_with_db(test_db.db.clone()).await;
    let server = TestServer::new(app)
        .expect("Test setup: failed to create test server for sync filtering integration tests");

    let room_id = "!wildcard:example.com";

    // Create events with different types
    create_test_room_with_events(
        &test_db,
        room_id,
        vec![
            ("m.room.message", json!({"body": "Message"})),
            ("m.room.member", json!({"membership": "join"})),
            ("m.room.name", json!({"name": "Test Room"})),
            ("m.typing", json!({"user_ids": []})),
            ("m.receipt", json!({"receipts": {}})),
        ],
    )
    .await
    .expect("Test setup: failed to create test events for sync filtering tests");

    // Test wildcard patterns
    let test_cases = vec![
        ("*", 5),               // Should match all events
        ("m.room.*", 3),        // Should match m.room.message, m.room.member, m.room.name
        ("m.room.message*", 1), // Should match only m.room.message
    ];

    for (pattern, expected_count) in test_cases {
        let filter = MatrixFilter {
            room: Some(RoomFilter {
                timeline: Some(RoomEventFilter {
                    base: EventFilter {
                        types: Some(vec![pattern.to_string()]),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };

        let filter_json = serde_json::to_string(&filter).expect("Failed to serialize filter");
        let encoded_filter = urlencoding::encode(&filter_json);

        let response = server
            .get(&format!("/_matrix/client/v3/sync?filter={}", encoded_filter))
            .add_header("Authorization", "Bearer test_token")
            .await;

        assert_eq!(response.status_code(), 200);
        let sync_response: Value = response.json();

        if let Some(room_response) = sync_response["rooms"]["join"][room_id].as_object()
            && let Some(events) = room_response["timeline"]["events"].as_array()
        {
            assert_eq!(events.len(), expected_count, "Wildcard pattern '{}' failed", pattern);
        }
    }

    test_db.cleanup().await.expect("Failed to cleanup test database");
}

/// Test filter precedence rules (not_types takes precedence over types)
#[tokio::test]
async fn test_filter_precedence_rules() {
    let test_db = TestDatabase::new().await
        .expect("Test setup: failed to create test database for sync filtering tests");
    let app = common::create_test_app_with_db(test_db.db.clone()).await;
    let server = TestServer::new(app)
        .expect("Test setup: failed to create test server for sync filtering integration tests");

    let room_id = "!precedence:example.com";

    create_test_room_with_events(
        &test_db,
        room_id,
        vec![
            ("m.room.message", json!({"body": "Message"})),
            ("m.room.member", json!({"membership": "join"})),
            ("m.room.name", json!({"name": "Test Room"})),
        ],
    )
    .await
    .expect("Test setup: failed to create test events for sync filtering tests");

    // Test that not_types takes precedence over types
    let filter = MatrixFilter {
        room: Some(RoomFilter {
            timeline: Some(RoomEventFilter {
                base: EventFilter {
                    types: Some(vec!["m.room.*".to_string()]), // Should include all room events
                    not_types: Some(vec!["m.room.member".to_string()]), // But exclude member events
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    let filter_json = serde_json::to_string(&filter).expect("Failed to serialize filter");
    let encoded_filter = urlencoding::encode(&filter_json);

    let response = server
        .get(&format!("/_matrix/client/v3/sync?filter={}", encoded_filter))
        .add_header("Authorization", "Bearer test_token")
        .await;

    assert_eq!(response.status_code(), 200);
    let sync_response: Value = response.json();

    if let Some(room_response) = sync_response["rooms"]["join"][room_id].as_object()
        && let Some(events) = room_response["timeline"]["events"].as_array()
    {
        // Should have message and name events, but not member events
        assert_eq!(events.len(), 2, "Precedence rule failed");
        for event in events {
            assert_ne!(event["type"], "m.room.member", "not_types precedence failed");
        }
    }

    test_db.cleanup().await.expect("Failed to cleanup test database");
}

/// Test database-level filtering performance
#[tokio::test]
async fn test_database_level_filtering_performance() {
    let test_db = TestDatabase::new().await
        .expect("Test setup: failed to create test database for sync filtering tests");
    let app = common::create_test_app_with_db(test_db.db.clone()).await;
    let server = TestServer::new(app)
        .expect("Test setup: failed to create test server for sync filtering integration tests");

    let room_id = "!performance:example.com";

    // Create a large number of events
    let mut events = Vec::new();
    for i in 0..100 {
        events.push(("m.room.message", json!({"body": format!("Message {}", i)})));
        events.push(("m.room.member", json!({"membership": "join"})));
    }

    create_test_room_with_events(&test_db, room_id, events)
        .await
        .expect("Test setup: failed to create test events for sync filtering tests");

    let filter = MatrixFilter {
        room: Some(RoomFilter {
            timeline: Some(RoomEventFilter {
                base: EventFilter {
                    types: Some(vec!["m.room.message".to_string()]),
                    limit: Some(10),
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    let start_time = std::time::Instant::now();

    let filter_json = serde_json::to_string(&filter).expect("Failed to serialize filter");
    let encoded_filter = urlencoding::encode(&filter_json);

    let response = server
        .get(&format!("/_matrix/client/v3/sync?filter={}", encoded_filter))
        .add_header("Authorization", "Bearer test_token")
        .await;

    let duration = start_time.elapsed();

    assert_eq!(response.status_code(), 200);
    let sync_response: Value = response.json();

    // Verify filtering worked correctly
    if let Some(room_response) = sync_response["rooms"]["join"][room_id].as_object()
        && let Some(events) = room_response["timeline"]["events"].as_array()
    {
        assert!(events.len() <= 10, "Event limit not respected");
        for event in events {
            assert_eq!(event["type"], "m.room.message", "Event type filtering failed");
        }
    }

    // Performance assertion - should complete within reasonable time
    assert!(duration.as_millis() < 1000, "Filtering took too long: {:?}", duration);

    test_db.cleanup().await.expect("Failed to cleanup test database");
}

/// Helper function to create a room with test events
async fn create_test_room_with_events(
    test_db: &TestDatabase,
    room_id: &str,
    events: Vec<(&str, Value)>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create room
    let _: Option<Value> = test_db
        .db
        .create(("room", room_id))
        .content(json!({
            "room_id": room_id,
            "creator": "@test:example.com",
            "room_version": "9"
        }))
        .await?;

    // Create events
    for (i, (event_type, content)) in events.iter().enumerate() {
        let event_id = format!("$event_{}_{}", i, room_id);
        let _: Option<Value> = test_db
            .db
            .create(("event", &event_id))
            .content(json!({
                "event_id": event_id,
                "room_id": room_id,
                "event_type": event_type,
                "sender": "@test:example.com",
                "content": content,
                "origin_server_ts": 1234567890 + i as u64,
                "unsigned": {}
            }))
            .await?;
    }

    Ok(())
}
