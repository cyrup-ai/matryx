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
use matryx_server::{_matrix::federation::v1::user::keys::claim, AppState};

/// Test setup helper
async fn setup_test_state() -> AppState {
    // Create test database connection
    let db = surrealdb::Surreal::new::<surrealdb::engine::any::Any>("mem://")
        .await
        .expect("Failed to create test database");

    // Initialize database with test schema
    db.use_ns("test").use_db("test").await.expect("Failed to set namespace/db");

    // Create device table
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
            },
            one_time_keys = {
                'signed_curve25519:AAAAHQ': {
                    'key': 'test_one_time_key',
                    'signatures': {
                        '@alice:example.com': {
                            'ed25519:TESTDEVICE001': 'test_otk_signature'
                        }
                    }
                }
            },
            fallback_keys = {
                'signed_curve25519:fallback': {
                    'key': 'test_fallback_key',
                    'signatures': {
                        '@alice:example.com': {
                            'ed25519:TESTDEVICE001': 'test_fallback_signature'
                        }
                    }
                }
            }
    ",
        )
        .await
        .expect("Failed to create test device");

    let session_service = Arc::new(MatrixSessionService::new(
        "test_secret".to_string(),
        "test.example.com".to_string(),
    ));

    AppState::new(db, session_service, "test.example.com".to_string())
}
#[tokio::test]
async fn test_claim_one_time_key_success() {
    let state = setup_test_state().await;

    let request_payload = json!({
        "one_time_keys": {
            "@alice:example.com": {
                "TESTDEVICE001": "signed_curve25519"
            }
        }
    });

    let result = claim::post(State(state), Json(request_payload))
        .await
        .expect("Handler should succeed");

    let response = result.0;

    // Verify response structure
    assert!(response.get("one_time_keys").is_some());

    let otks = response["one_time_keys"].as_object().expect("Should be object");
    assert!(otks.contains_key("@alice:example.com"));

    let alice_keys = otks["@alice:example.com"].as_object().expect("Should be object");
    assert!(alice_keys.contains_key("TESTDEVICE001"));

    let device_keys = alice_keys["TESTDEVICE001"].as_object().expect("Should be object");
    assert!(!device_keys.is_empty(), "Should have claimed a key");
}

#[tokio::test]
async fn test_claim_nonexistent_user() {
    let state = setup_test_state().await;

    let request_payload = json!({
        "one_time_keys": {
            "@nonexistent:example.com": {
                "DEVICE123": "signed_curve25519"
            }
        }
    });

    let result = claim::post(State(state), Json(request_payload))
        .await
        .expect("Handler should succeed");

    let response = result.0;

    // Should return empty response for nonexistent user
    let otks = response["one_time_keys"].as_object().expect("Should be object");
    assert!(
        !otks.contains_key("@nonexistent:example.com"),
        "Should not have keys for nonexistent user"
    );
}
#[tokio::test]
async fn test_claim_fallback_key_when_no_one_time_keys() {
    let state = setup_test_state().await;

    // Remove one-time keys from device
    let _: Vec<surrealdb::sql::Value> = state
        .db
        .query(
            "
        UPDATE device SET one_time_keys = {} WHERE device_id = 'TESTDEVICE001'
    ",
        )
        .await
        .expect("Failed to clear one-time keys");

    let request_payload = json!({
        "one_time_keys": {
            "@alice:example.com": {
                "TESTDEVICE001": "signed_curve25519"
            }
        }
    });

    let result = claim::post(State(state), Json(request_payload))
        .await
        .expect("Handler should succeed");

    let response = result.0;

    // Should return fallback key
    let otks = response["one_time_keys"].as_object().expect("Should be object");
    assert!(otks.contains_key("@alice:example.com"));

    let alice_keys = otks["@alice:example.com"].as_object().expect("Should be object");
    assert!(alice_keys.contains_key("TESTDEVICE001"));

    let device_keys = alice_keys["TESTDEVICE001"].as_object().expect("Should be object");
    assert!(!device_keys.is_empty(), "Should have claimed fallback key");

    // Verify it's the fallback key (contains "fallback" in key id)
    let has_fallback = device_keys.keys().any(|k| k.contains("fallback"));
    assert!(has_fallback, "Should contain fallback key");
}

#[tokio::test]
async fn test_empty_request() {
    let state = setup_test_state().await;

    let request_payload = json!({
        "one_time_keys": {}
    });

    let result = claim::post(State(state), Json(request_payload))
        .await
        .expect("Handler should succeed");

    let response = result.0;

    // Should return empty response
    let otks = response["one_time_keys"].as_object().expect("Should be object");
    assert!(otks.is_empty(), "Should be empty response");
}
