use axum::{Router, routing::get};
use matryx_server::{AppState, MatrixSessionService, ServerConfig};
use std::sync::Arc;
use surrealdb::{
    Surreal,
    engine::any::{self, Any},
};

pub mod integration;

/// Creates a test application router for basic integration testing
/// This is used by integration test modules that need a minimal test app
pub async fn create_test_app() -> Router {
    // Create test database with file storage
    let db = any::connect("surrealkv://test_data/common_test.db")
        .await
        .expect("Failed to connect to test database");
    db.use_ns("test")
        .use_db("matrix")
        .await
        .expect("Failed to select test namespace");

    // Create test configuration
    use matryx_server::config::{EmailConfig, PushCacheConfig, SmsConfig};
    use matryx_server::middleware::TransactionConfig;
    let config = ServerConfig {
        homeserver_name: "test.localhost".to_string(),
        federation_port: 8448,
        media_base_url: "https://test.localhost".to_string(),
        admin_email: "admin@test.localhost".to_string(),
        environment: "test".to_string(),
        server_implementation_name: "matryx".to_string(),
        server_implementation_version: "0.1.0".to_string(),
        email_config: EmailConfig {
            smtp_server: "localhost".to_string(),
            smtp_port: 587,
            smtp_username: "".to_string(),
            smtp_password: "".to_string(),
            from_address: "noreply@test.localhost".to_string(),
            enabled: false,
        },
        sms_config: SmsConfig {
            provider: "twilio".to_string(),
            api_key: "".to_string(),
            api_secret: "".to_string(),
            from_number: "".to_string(),
            enabled: false,
        },
        push_cache_config: PushCacheConfig::default(),
        transaction_config: TransactionConfig::from_env(),
    };

    // Create session service
    let jwt_secret = "test_secret".to_string().into_bytes();
    let session_repo = matryx_surrealdb::repository::session::SessionRepository::new(db.clone());
    let key_server_repo = matryx_surrealdb::repository::key_server::KeyServerRepository::new(db.clone());
    let session_service =
        Arc::new(MatrixSessionService::new(jwt_secret, config.homeserver_name.clone(), session_repo, key_server_repo));

    // Create HTTP client
    let http_client = Arc::new(reqwest::Client::new());

    // Create DNS resolver for event signer
    let well_known_client = Arc::new(matryx_server::federation::well_known_client::WellKnownClient::new(http_client.clone()));
    let dns_resolver = Arc::new(matryx_server::federation::dns_resolver::MatrixDnsResolver::new(well_known_client).expect("Failed to create DNS resolver"));

    // Create event signer
    let event_signer = Arc::new(matryx_server::federation::event_signer::EventSigner::new(
        session_service.clone(),
        db.clone(),
        dns_resolver,
        config.homeserver_name.clone(),
        "ed25519:auto".to_string(),
    ).expect("Failed to create test event signer"));

    // Create application state
    let schema = include_str!("../../../surrealdb/migrations/matryx.surql");
    db.query(schema).await.expect("Failed to execute schema");

    // Create app state with all required fields
    let config_static: &'static ServerConfig = Box::leak(Box::new(config.clone()));
    let state = AppState::new(
        db,
        session_service,
        config.homeserver_name.clone(),
        config_static,
        http_client,
        event_signer,
    );

    // Create a simple test router
    Router::new().route("/test", get(|| async { "test" })).with_state(state)
}

pub async fn create_test_app_with_db(db: Surreal<Any>) -> Router {
    // Create test configuration
    use matryx_server::config::{EmailConfig, PushCacheConfig, SmsConfig};
    use matryx_server::middleware::TransactionConfig;
    let config = ServerConfig {
        homeserver_name: "test.localhost".to_string(),
        federation_port: 8448,
        media_base_url: "https://test.localhost".to_string(),
        admin_email: "admin@test.localhost".to_string(),
        environment: "test".to_string(),
        server_implementation_name: "matryx".to_string(),
        server_implementation_version: "0.1.0".to_string(),
        email_config: EmailConfig {
            smtp_server: "localhost".to_string(),
            smtp_port: 587,
            smtp_username: "".to_string(),
            smtp_password: "".to_string(),
            from_address: "noreply@test.localhost".to_string(),
            enabled: false,
        },
        sms_config: SmsConfig {
            provider: "twilio".to_string(),
            api_key: "".to_string(),
            api_secret: "".to_string(),
            from_number: "".to_string(),
            enabled: false,
        },
        push_cache_config: PushCacheConfig::default(),
        transaction_config: TransactionConfig::from_env(),
    };

    // Create session service
    let jwt_secret = "test_secret".to_string().into_bytes();
    let session_repo = matryx_surrealdb::repository::session::SessionRepository::new(db.clone());
    let key_server_repo = matryx_surrealdb::repository::key_server::KeyServerRepository::new(db.clone());
    let session_service =
        Arc::new(MatrixSessionService::new(jwt_secret, config.homeserver_name.clone(), session_repo, key_server_repo));

    // Create HTTP client
    let http_client = Arc::new(reqwest::Client::new());

    // Create DNS resolver for event signer
    let well_known_client = Arc::new(matryx_server::federation::well_known_client::WellKnownClient::new(http_client.clone()));
    let dns_resolver = Arc::new(matryx_server::federation::dns_resolver::MatrixDnsResolver::new(well_known_client).expect("Failed to create DNS resolver"));

    // Create event signer
    let event_signer = Arc::new(matryx_server::federation::event_signer::EventSigner::new(
        session_service.clone(),
        db.clone(),
        dns_resolver,
        config.homeserver_name.clone(),
        "ed25519:auto".to_string(),
    ).expect("Failed to create test event signer"));

    // We need to make config static for AppState - use Box::leak for tests
    let static_config: &'static ServerConfig = Box::leak(Box::new(config));

    let state = AppState::new(
        db,
        session_service,
        static_config.homeserver_name.clone(),
        static_config,
        http_client,
        event_signer,
    );

    // Create a simple test router
    Router::new().route("/test", get(|| async { "test" })).with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_test_app() {
        // Test the create_test_app function
        let app = create_test_app().await;
        
        // Verify that the router was created successfully
        // We can't easily test the actual routes without starting a server,
        // but we can at least ensure the function executes without panicking
        drop(app); // Explicitly use the app variable
    }

    #[tokio::test]
    async fn test_create_test_app_with_db() {
        // Create a test database
        let db = any::connect("surrealkv://test_data/common_with_db_test.db")
            .await
            .expect("Failed to connect to test database");
        db.use_ns("test")
            .use_db("matrix")
            .await
            .expect("Failed to select test namespace");
        
        // Test the create_test_app_with_db function
        let app = create_test_app_with_db(db).await;
        
        // Verify that the router was created successfully
        drop(app); // Explicitly use the app variable
    }
}