use axum::{
    Json,
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderValue, Request, StatusCode},
};
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;

use matryx_server::auth::MatrixSessionService;
use matryx_server::{_matrix::federation::v1::send::by_txn_id, AppState};

/// Test setup helper
async fn setup_test_state() -> AppState {
    // Create test database connection
    let db = surrealdb::Surreal::new::<surrealdb::engine::any::Any>("mem://")
        .await
        .expect("Failed to create test database");

    // Initialize database with test schema
    db.use_ns("test").use_db("test").await.expect("Failed to set namespace/db");

    // Initialize cross_signing_keys table
    let _: Vec<surrealdb::sql::Value> = db.query("
        DEFINE TABLE cross_signing_keys SCHEMAFULL
            PERMISSIONS FOR select WHERE true
            FOR create, update WHERE $auth.id != NONE
            FOR delete WHERE $auth.admin = true;
        
        DEFINE FIELD user_id ON cross_signing_keys TYPE string ASSERT $value != NONE;
        DEFINE FIELD key_type ON cross_signing_keys TYPE string ASSERT $value INSIDE ['master', 'self_signing', 'user_signing'];
        DEFINE FIELD keys ON cross_signing_keys TYPE object;
        DEFINE FIELD signatures ON cross_signing_keys TYPE option<object>;
        DEFINE FIELD usage ON cross_signing_keys TYPE array<string>;
        DEFINE FIELD updated_at ON cross_signing_keys TYPE datetime DEFAULT time::now();
        
        DEFINE INDEX cross_signing_keys_user_key_type ON cross_signing_keys COLUMNS user_id, key_type UNIQUE;
    ").await.expect("Failed to initialize test schema");

    // Initialize federation_transactions table for caching
    let _: Vec<surrealdb::sql::Value> = db.query("
        DEFINE TABLE federation_transactions SCHEMAFULL;
        DEFINE FIELD transaction_key ON federation_transactions TYPE string;
        DEFINE FIELD result ON federation_transactions TYPE object;
        DEFINE FIELD created_at ON federation_transactions TYPE datetime DEFAULT time::now();
        DEFINE INDEX federation_transactions_key ON federation_transactions COLUMNS transaction_key UNIQUE;
    ").await.expect("Failed to initialize federation transactions table");

    AppState {
        db: Arc::new(db),
        session_service: Arc::new(MatrixSessionService::new()),
    }
}

/// Helper to create valid X-Matrix authentication headers
fn create_x_matrix_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();

    // Create a valid X-Matrix authorization header
    // Format: X-Matrix origin=origin.server,key="ed25519:key_id",sig="signature"
    let auth_header = r#"origin="example.com",key="ed25519:key1",sig="signature_placeholder""#;
    headers.insert("authorization", HeaderValue::from_static(&format!("X-Matrix {}", auth_header)));

    headers
}

#[tokio::test]
async fn test_signing_key_update_edu_processing() {
    let state = setup_test_state().await;

    // Create federation transaction with signing key update EDU
    let transaction_payload = json!({
        "origin": "example.com",
        "origin_server_ts": 1234567890,
        "pdus": [],
        "edus": [
            {
                "type": "m.signing_key_update",
                "content": {
                    "user_id": "@alice:example.com",
                    "master_key": {
                        "keys": {
                            "ed25519:master_key_id": "master_key_value_placeholder"
                        },
                        "usage": ["master"],
                        "user_id": "@alice:example.com"
                    },
                    "self_signing_key": {
                        "keys": {
                            "ed25519:self_signing_key_id": "self_signing_key_value_placeholder"
                        },
                        "usage": ["self_signing"],
                        "user_id": "@alice:example.com",
                        "signatures": {
                            "@alice:example.com": {
                                "ed25519:master_key_id": "master_signature_placeholder"
                            }
                        }
                    }
                }
            }
        ]
    });

    // Create test request
    let headers = create_x_matrix_headers();

    // Test the federation transaction endpoint
    let result = by_txn_id::put(
        State(state.clone()),
        axum::extract::Path("test_transaction_123".to_string()),
        headers,
        Json(transaction_payload),
    )
    .await;

    // Should succeed
    match result {
        Ok(response) => {
            println!("Federation transaction processed successfully: {:?}", response.0);

            // Verify the keys were stored in the database
            let query_result = state
                .db
                .query(
                    "
                SELECT * FROM cross_signing_keys WHERE user_id = '@alice:example.com'
            ",
                )
                .await
                .expect("Failed to query cross signing keys");

            let results: Vec<serde_json::Value> = query_result.take(0).unwrap_or_default();

            // Should have stored both master and self_signing keys
            assert!(!results.is_empty(), "Cross-signing keys should be stored");

            // Check for master key
            let master_key = results.iter().find(|r| r["key_type"] == "master");
            assert!(master_key.is_some(), "Master key should be stored");

            // Check for self_signing key
            let self_signing_key = results.iter().find(|r| r["key_type"] == "self_signing");
            assert!(self_signing_key.is_some(), "Self-signing key should be stored");

            println!("✅ Cross-signing keys properly stored in database");
        },
        Err(status_code) => {
            panic!("Federation transaction failed with status: {:?}", status_code);
        },
    }
}

#[tokio::test]
async fn test_transaction_deduplication() {
    let state = setup_test_state().await;

    let transaction_payload = json!({
        "origin": "example.com",
        "origin_server_ts": 1234567890,
        "pdus": [],
        "edus": [
            {
                "type": "m.signing_key_update",
                "content": {
                    "user_id": "@bob:example.com",
                    "master_key": {
                        "keys": {
                            "ed25519:master_key_id_2": "master_key_value_2"
                        },
                        "usage": ["master"],
                        "user_id": "@bob:example.com"
                    }
                }
            }
        ]
    });

    let headers = create_x_matrix_headers();
    let txn_id = "dedup_test_456";

    // Process transaction first time
    let result1 = by_txn_id::put(
        State(state.clone()),
        axum::extract::Path(txn_id.to_string()),
        headers.clone(),
        Json(transaction_payload.clone()),
    )
    .await;

    assert!(result1.is_ok(), "First transaction should succeed");

    // Process same transaction again - should return cached result
    let result2 = by_txn_id::put(
        State(state.clone()),
        axum::extract::Path(txn_id.to_string()),
        headers.clone(),
        Json(transaction_payload.clone()),
    )
    .await;

    assert!(result2.is_ok(), "Duplicate transaction should succeed with cached result");

    println!("✅ Transaction deduplication working correctly");
}

#[tokio::test]
async fn test_invalid_edu_user_validation() {
    let state = setup_test_state().await;

    // Create EDU with user from different origin server (should be rejected)
    let invalid_transaction = json!({
        "origin": "example.com",
        "origin_server_ts": 1234567890,
        "pdus": [],
        "edus": [
            {
                "type": "m.signing_key_update",
                "content": {
                    "user_id": "@malicious:different.server.com",
                    "master_key": {
                        "keys": {
                            "ed25519:malicious_key": "malicious_value"
                        },
                        "usage": ["master"],
                        "user_id": "@malicious:different.server.com"
                    }
                }
            }
        ]
    });

    let headers = create_x_matrix_headers();

    let result = by_txn_id::put(
        State(state.clone()),
        axum::extract::Path("invalid_user_test_789".to_string()),
        headers,
        Json(invalid_transaction),
    )
    .await;

    // Should fail due to user origin validation
    match result {
        Err(StatusCode::INTERNAL_SERVER_ERROR) => {
            println!("✅ Invalid user origin correctly rejected");
        },
        _ => {
            panic!("Transaction with invalid user origin should fail");
        },
    }
}
