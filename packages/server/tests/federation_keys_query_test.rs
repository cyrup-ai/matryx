use axum::{
    Json,
    body::Body,
    extract::State,
    http::{Request, StatusCode},
};
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;

use matryx_server::auth::MatrixSessionService;
use matryx_server::{_matrix::federation::v1::user::keys::query, AppState};

/// Test setup helper
async fn setup_test_state() -> AppState {
    // Create test database connection
    let db = surrealdb::Surreal::new::<surrealdb::engine::any::Any>("mem://")
        .await
        .expect("Failed to create test database");

    // Initialize database with test schema
    db.use_ns("test").use_db("test").await.expect("Failed to set namespace/db");

    // Create test device
    let _: Vec<surrealdb::sql::Value> = db
        .query(
            "
        CREATE device SET
            device_id = 'TESTDEVICE001',
            user_id = '@alice:example.com',
            display_name = 'Test Device',
            device_keys = {
                'algorithms': ['m.olm.v1.curve25519-aes-sha2', 'm.megolm.v1.aes-sha2'],
                'device_id': 'TESTDEVICE001',
                'keys': {
                    'curve25519:TESTDEVICE001': 'test_curve25519_key',
                    'ed25519:TESTDEVICE001': 'test_ed25519_key'
                },
                'signatures': {
                    '@alice:example.com': {
                        'ed25519:TESTDEVICE001': 'test_signature'
                    }
                },
                'user_id': '@alice:example.com'
            }
    ",
        )
        .await
        .expect("Failed to create test device"); // Create cross-signing keys
    let _: Vec<surrealdb::sql::Value> = db
        .query(
            "
        CREATE cross_signing_keys SET
            user_id = '@alice:example.com',
            key_type = 'master',
            keys = {
                'ed25519:master_key_id': 'master_public_key'
            },
            signatures = {
                '@alice:example.com': {
                    'ed25519:master_key_id': 'master_signature'
                }
            },
            usage = ['master']
    ",
        )
        .await
        .expect("Failed to create master key");

    let _: Vec<surrealdb::sql::Value> = db
        .query(
            "
        CREATE cross_signing_keys SET
            user_id = '@alice:example.com',
            key_type = 'self_signing',
            keys = {
                'ed25519:self_signing_key_id': 'self_signing_public_key'
            },
            signatures = {
                '@alice:example.com': {
                    'ed25519:master_key_id': 'self_signing_signature'
                }
            },
            usage = ['self_signing']
    ",
        )
        .await
        .expect("Failed to create self-signing key");

    let session_service = Arc::new(MatrixSessionService::new(
        "test_secret".to_string(),
        "test.example.com".to_string(),
    ));

    AppState::new(db, session_service, "test.example.com".to_string())
}
#[tokio::test]
async fn test_query_all_devices_for_user() {
    let state = setup_test_state().await;

    let request_payload = json!({
        "device_keys": {
            "@alice:example.com": []
        }
    });

    let result = query::post(State(state), Json(request_payload))
        .await
        .expect("Handler should succeed");

    let response = result.0;

    // Verify response structure
    assert!(response.get("device_keys").is_some());

    let device_keys = response["device_keys"].as_object().expect("Should be object");
    assert!(device_keys.contains_key("@alice:example.com"));

    let alice_devices = device_keys["@alice:example.com"].as_object().expect("Should be object");
    assert!(alice_devices.contains_key("TESTDEVICE001"));

    let device = alice_devices["TESTDEVICE001"].as_object().expect("Should be object");
    assert_eq!(device["device_id"].as_str().expect("Should be string"), "TESTDEVICE001");
    assert_eq!(device["user_id"].as_str().expect("Should be string"), "@alice:example.com");
    assert!(device.contains_key("algorithms"));
    assert!(device.contains_key("keys"));
    assert!(device.contains_key("signatures"));
}

#[tokio::test]
async fn test_query_specific_device() {
    let state = setup_test_state().await;

    let request_payload = json!({
        "device_keys": {
            "@alice:example.com": ["TESTDEVICE001"]
        }
    });

    let result = query::post(State(state), Json(request_payload))
        .await
        .expect("Handler should succeed");

    let response = result.0;

    // Verify specific device is returned
    let device_keys = response["device_keys"].as_object().expect("Should be object");
    let alice_devices = device_keys["@alice:example.com"].as_object().expect("Should be object");
    assert!(alice_devices.contains_key("TESTDEVICE001"));
    assert_eq!(alice_devices.len(), 1, "Should only return requested device");
}
#[tokio::test]
async fn test_query_cross_signing_keys() {
    let state = setup_test_state().await;

    let request_payload = json!({
        "device_keys": {
            "@alice:example.com": []
        }
    });

    let result = query::post(State(state), Json(request_payload))
        .await
        .expect("Handler should succeed");

    let response = result.0;

    // Verify cross-signing keys are included
    assert!(response.get("master_keys").is_some());
    assert!(response.get("self_signing_keys").is_some());

    if let Some(master_keys) = response["master_keys"].as_object() {
        assert!(master_keys.contains_key("@alice:example.com"));
        let master_key = master_keys["@alice:example.com"].as_object().expect("Should be object");
        assert!(master_key.contains_key("keys"));
        assert!(master_key.contains_key("usage"));
        assert_eq!(master_key["user_id"].as_str().expect("Should be string"), "@alice:example.com");
    }

    if let Some(self_signing_keys) = response["self_signing_keys"].as_object() {
        assert!(self_signing_keys.contains_key("@alice:example.com"));
    }
}

#[tokio::test]
async fn test_query_nonexistent_user() {
    let state = setup_test_state().await;

    let request_payload = json!({
        "device_keys": {
            "@nonexistent:example.com": []
        }
    });

    let result = query::post(State(state), Json(request_payload))
        .await
        .expect("Handler should succeed");

    let response = result.0;

    // Should return empty response for nonexistent user
    let device_keys = response["device_keys"].as_object().expect("Should be object");
    assert!(
        !device_keys.contains_key("@nonexistent:example.com"),
        "Should not have keys for nonexistent user"
    );
}
