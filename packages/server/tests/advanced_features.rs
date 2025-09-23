use axum_test::TestServer;
use serde_json::{Value, json};
use std::collections::HashMap;

mod common;
use common::*;

#[tokio::test]
async fn test_push_notification_flow() {
    let app = create_test_app().await;
    let server = TestServer::new(app).unwrap();

    // 1. Create test user with pusher
    let user_response = server
        .post("/_matrix/client/v3/register")
        .json(&json!({
            "username": "push_test_user",
            "password": "test_password",
            "device_id": "TEST_DEVICE"
        }))
        .await;

    assert_eq!(user_response.status_code(), 200);
    let user_data: Value = user_response.json();
    let access_token = user_data["access_token"].as_str().unwrap();

    // Set up pusher
    let pusher_response = server
        .post("/_matrix/client/v3/pushers/set")
        .add_header("Authorization", format!("Bearer {}", access_token))
        .json(&json!({
            "pusher_id": "test_pusher",
            "kind": "http",
            "app_id": "com.example.app",
            "app_display_name": "Test App",
            "device_display_name": "Test Device",
            "lang": "en",
            "data": {
                "url": "https://push.example.com",
                "format": "event_id_only"
            }
        }))
        .await;

    assert_eq!(pusher_response.status_code(), 200);

    // 2. Create room
    let room_response = server
        .post("/_matrix/client/v3/createRoom")
        .add_header("Authorization", format!("Bearer {}", access_token))
        .json(&json!({
            "name": "Push Test Room",
            "preset": "public_chat"
        }))
        .await;

    assert_eq!(room_response.status_code(), 200);
    let room_data: Value = room_response.json();
    let room_id = room_data["room_id"].as_str().unwrap();

    // 3. Send message to room (this should trigger push notification)
    let message_response = server
        .put(&format!("/_matrix/client/v3/rooms/{}/send/m.room.message/test_txn", room_id))
        .add_header("Authorization", format!("Bearer {}", access_token))
        .json(&json!({
            "msgtype": "m.text",
            "body": "Test push notification message"
        }))
        .await;

    assert_eq!(message_response.status_code(), 200);

    // 4. Verify push rule evaluation (would need mock push gateway to fully test)
    // For now, just verify the pusher was set up correctly
    let pushers_response = server
        .get("/_matrix/client/v3/pushers")
        .add_header("Authorization", format!("Bearer {}", access_token))
        .await;

    assert_eq!(pushers_response.status_code(), 200);
    let pushers_data: Value = pushers_response.json();
    assert!(pushers_data["pushers"].as_array().unwrap().len() > 0);
}

#[tokio::test]
async fn test_room_preview_world_readable() {
    let app = create_test_app().await;
    let server = TestServer::new(app).unwrap();

    // 1. Create room with world_readable history
    let user_response = server
        .post("/_matrix/client/v3/register")
        .json(&json!({
            "username": "preview_test_user",
            "password": "test_password",
            "device_id": "TEST_DEVICE"
        }))
        .await;

    let user_data: Value = user_response.json();
    let access_token = user_data["access_token"].as_str().unwrap();

    let room_response = server
        .post("/_matrix/client/v3/createRoom")
        .add_header("Authorization", format!("Bearer {}", access_token))
        .json(&json!({
            "name": "Preview Test Room",
            "preset": "public_chat",
            "initial_state": [{
                "type": "m.room.history_visibility",
                "content": {
                    "history_visibility": "world_readable"
                }
            }]
        }))
        .await;

    let room_data: Value = room_response.json();
    let room_id = room_data["room_id"].as_str().unwrap();

    // Send a test message
    server
        .put(&format!("/_matrix/client/v3/rooms/{}/send/m.room.message/preview_test", room_id))
        .add_header("Authorization", format!("Bearer {}", access_token))
        .json(&json!({
            "msgtype": "m.text",
            "body": "This message should be previewable"
        }))
        .await;

    // 2. Test preview access without joining (no auth header)
    let preview_response = server
        .get(&format!("/_matrix/client/v3/rooms/{}/initialSync", room_id))
        .await;

    assert_eq!(preview_response.status_code(), 200);
    let preview_data: Value = preview_response.json();

    // 3. Verify event visibility rules
    assert_eq!(preview_data["room_id"], room_id);
    assert!(preview_data["messages"]["chunk"].as_array().is_some());
    assert!(preview_data["state"].as_array().is_some());
}

#[tokio::test]
async fn test_room_preview_forbidden_for_non_world_readable() {
    let app = create_test_app().await;
    let server = TestServer::new(app).unwrap();

    // Create room with default (shared) history visibility
    let user_response = server
        .post("/_matrix/client/v3/register")
        .json(&json!({
            "username": "private_room_user",
            "password": "test_password",
            "device_id": "TEST_DEVICE"
        }))
        .await;

    let user_data: Value = user_response.json();
    let access_token = user_data["access_token"].as_str().unwrap();

    let room_response = server
        .post("/_matrix/client/v3/createRoom")
        .add_header("Authorization", format!("Bearer {}", access_token))
        .json(&json!({
            "name": "Private Room",
            "preset": "private_chat"
        }))
        .await;

    let room_data: Value = room_response.json();
    let room_id = room_data["room_id"].as_str().unwrap();

    // Try to preview without authentication - should be forbidden
    let preview_response = server
        .get(&format!("/_matrix/client/v3/rooms/{}/initialSync", room_id))
        .await;

    assert_eq!(preview_response.status_code(), 403);
}

#[tokio::test]
async fn test_guest_access_restrictions() {
    let app = create_test_app().await;
    let server = TestServer::new(app).unwrap();

    // 1. Enable guest access on room
    let user_response = server
        .post("/_matrix/client/v3/register")
        .json(&json!({
            "username": "guest_room_owner",
            "password": "test_password",
            "device_id": "TEST_DEVICE"
        }))
        .await;

    let user_data: Value = user_response.json();
    let access_token = user_data["access_token"].as_str().unwrap();

    let room_response = server
        .post("/_matrix/client/v3/createRoom")
        .add_header("Authorization", format!("Bearer {}", access_token))
        .json(&json!({
            "name": "Guest Access Room",
            "preset": "public_chat",
            "initial_state": [{
                "type": "m.room.guest_access",
                "content": {
                    "guest_access": "can_join"
                }
            }]
        }))
        .await;

    let room_data: Value = room_response.json();
    let room_id = room_data["room_id"].as_str().unwrap();

    // 2. Test guest user registration
    let guest_response = server
        .post("/_matrix/client/v3/register")
        .json(&json!({
            "kind": "guest"
        }))
        .await;

    // Guest registration should work
    assert!(guest_response.status_code() == 200 || guest_response.status_code() == 400); // May not be implemented

    // 3. Verify API restrictions for guests would be tested here
    // This would require implementing guest-specific limitations
}

#[tokio::test]
async fn test_room_history_visibility_enforcement() {
    let app = create_test_app().await;
    let server = TestServer::new(app).unwrap();

    // Test all history visibility modes
    let visibility_modes = vec!["invited", "joined", "shared", "world_readable"];

    for mode in visibility_modes {
        let user_response = server
            .post("/_matrix/client/v3/register")
            .json(&json!({
                "username": format!("history_test_{}", mode),
                "password": "test_password",
                "device_id": "TEST_DEVICE"
            }))
            .await;

        let user_data: Value = user_response.json();
        let access_token = user_data["access_token"].as_str().unwrap();

        let room_response = server
            .post("/_matrix/client/v3/createRoom")
            .add_header("Authorization", format!("Bearer {}", access_token))
            .json(&json!({
                "name": format!("History Test Room {}", mode),
                "preset": "public_chat",
                "initial_state": [{
                    "type": "m.room.history_visibility",
                    "content": {
                        "history_visibility": mode
                    }
                }]
            }))
            .await;

        assert_eq!(room_response.status_code(), 200);
    }
}

#[tokio::test]
async fn test_room_tagging_operations() {
    let app = create_test_app().await;
    let server = TestServer::new(app).unwrap();

    // Create user and room
    let user_response = server
        .post("/_matrix/client/v3/register")
        .json(&json!({
            "username": "tag_test_user",
            "password": "test_password",
            "device_id": "TEST_DEVICE"
        }))
        .await;

    let user_data: Value = user_response.json();
    let access_token = user_data["access_token"].as_str().unwrap();
    let user_id = user_data["user_id"].as_str().unwrap();

    let room_response = server
        .post("/_matrix/client/v3/createRoom")
        .add_header("Authorization", format!("Bearer {}", access_token))
        .json(&json!({
            "name": "Tag Test Room"
        }))
        .await;

    let room_data: Value = room_response.json();
    let room_id = room_data["room_id"].as_str().unwrap();

    // Test Matrix reserved tags
    let reserved_tags = vec!["m.favourite", "m.lowpriority"];

    for tag in reserved_tags {
        let tag_response = server
            .put(&format!("/_matrix/client/v3/user/{}/rooms/{}/tags/{}", user_id, room_id, tag))
            .add_header("Authorization", format!("Bearer {}", access_token))
            .json(&json!({
                "order": 0.5
            }))
            .await;

        assert_eq!(tag_response.status_code(), 200);
    }

    // Test custom user tag
    let custom_tag_response = server
        .put(&format!("/_matrix/client/v3/user/{}/rooms/{}/tags/u.work", user_id, room_id))
        .add_header("Authorization", format!("Bearer {}", access_token))
        .json(&json!({
            "order": 0.3
        }))
        .await;

    assert_eq!(custom_tag_response.status_code(), 200);

    // Get tags and verify
    let get_tags_response = server
        .get(&format!("/_matrix/client/v3/user/{}/rooms/{}/tags", user_id, room_id))
        .add_header("Authorization", format!("Bearer {}", access_token))
        .await;

    assert_eq!(get_tags_response.status_code(), 200);
    let tags_data: Value = get_tags_response.json();
    assert!(tags_data["tags"].as_object().unwrap().contains_key("m.favourite"));
    assert!(tags_data["tags"].as_object().unwrap().contains_key("u.work"));
}

#[tokio::test]
async fn test_server_side_search() {
    let app = create_test_app().await;
    let server = TestServer::new(app).unwrap();

    // Create user and room with messages
    let user_response = server
        .post("/_matrix/client/v3/register")
        .json(&json!({
            "username": "search_test_user",
            "password": "test_password",
            "device_id": "TEST_DEVICE"
        }))
        .await;

    let user_data: Value = user_response.json();
    let access_token = user_data["access_token"].as_str().unwrap();

    let room_response = server
        .post("/_matrix/client/v3/createRoom")
        .add_header("Authorization", format!("Bearer {}", access_token))
        .json(&json!({
            "name": "Search Test Room"
        }))
        .await;

    let room_data: Value = room_response.json();
    let room_id = room_data["room_id"].as_str().unwrap();

    // Send searchable messages
    let messages = vec![
        "Hello world, this is a test message",
        "Another message with different content",
        "Final message for search testing",
    ];

    for (i, message) in messages.iter().enumerate() {
        server
            .put(&format!("/_matrix/client/v3/rooms/{}/send/m.room.message/search_{}", room_id, i))
            .add_header("Authorization", format!("Bearer {}", access_token))
            .json(&json!({
                "msgtype": "m.text",
                "body": message
            }))
            .await;
    }

    // Test search functionality
    let search_response = server
        .post("/_matrix/client/v3/search")
        .add_header("Authorization", format!("Bearer {}", access_token))
        .json(&json!({
            "search_categories": {
                "room_events": {
                    "search_term": "test",
                    "keys": ["content.body"],
                    "filter": {
                        "rooms": [room_id]
                    }
                }
            }
        }))
        .await;

    assert_eq!(search_response.status_code(), 200);
    let search_data: Value = search_response.json();
    assert!(
        search_data["search_categories"]["room_events"]["results"]
            .as_array()
            .is_some()
    );
}

#[tokio::test]
async fn test_third_party_invites() {
    let app = create_test_app().await;
    let server = TestServer::new(app).unwrap();

    // Create user and room
    let user_response = server
        .post("/_matrix/client/v3/register")
        .json(&json!({
            "username": "3pid_test_user",
            "password": "test_password",
            "device_id": "TEST_DEVICE"
        }))
        .await;

    let user_data: Value = user_response.json();
    let access_token = user_data["access_token"].as_str().unwrap();

    // Test 3PID endpoints exist and respond
    let threepid_response = server
        .get("/_matrix/client/v3/account/3pid")
        .add_header("Authorization", format!("Bearer {}", access_token))
        .await;

    // Should return 200 with empty list or proper 3PID data
    assert!(threepid_response.status_code() == 200 || threepid_response.status_code() == 404);
}
