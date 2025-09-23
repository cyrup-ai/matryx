use axum::{Router, response::Json, routing::get};
use axum_test::{TestResponse, TestServer};
use matryx_server::state::AppState;
use serde_json::{Value, json};
use std::collections::HashMap;
use surrealdb::{
    Surreal,
    engine::any::{self, Any},
};
use tokio::sync::OnceCell;
use wiremock::{Mock, MockServer, ResponseTemplate};

pub mod client_compatibility;
pub mod compliance;
pub mod database;
pub mod federation;
pub mod performance;

/// Matrix Test Server for HTTP API testing
pub struct MatrixTestServer {
    pub server: TestServer,
    pub mock_server: MockServer,
    pub base_url: String,
    pub app_state: AppState,
}

impl MatrixTestServer {
    pub async fn new() -> Self {
        let app = create_test_app().await;
        let server = TestServer::new(app).unwrap();
        let mock_server = MockServer::start().await;

        // Create test app state
        let db_any = any::connect("surrealkv://test_data/integration_test.db").await.expect("Failed to connect to test database");
        db_any
            .use_ns("test")
            .use_db("matrix")
            .await
            .expect("Failed to select test namespace");

        // Create required components for AppState
        use matryx_server::config::{EmailConfig, PushCacheConfig, SmsConfig};
        use matryx_server::{MatrixSessionService, ServerConfig};
        use std::sync::Arc;

        let config = ServerConfig {
            homeserver_name: "test.localhost".to_string(),
            federation_port: 8448,
            media_base_url: "https://test.localhost".to_string(),
            admin_email: "admin@test.localhost".to_string(),
            environment: "test".to_string(),
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
        };

        let jwt_secret = "test_secret".to_string().into_bytes();
        let session_repo = matryx_surrealdb::repository::session::SessionRepository::new(db_any.clone());
        let key_server_repo = matryx_surrealdb::repository::key_server::KeyServerRepository::new(db_any.clone());
        let session_service =
            Arc::new(MatrixSessionService::new(jwt_secret, config.homeserver_name.clone(), session_repo, key_server_repo));

        let http_client = Arc::new(reqwest::Client::new());
        let event_signer = Arc::new(matryx_server::federation::event_signer::EventSigner::new(
            session_service.clone(),
            db_any.clone(),
            config.homeserver_name.clone(),
            "ed25519:auto".to_string(),
        ));

        let config_static: &'static ServerConfig = Box::leak(Box::new(config.clone()));
        let app_state = AppState::new(
            db_any,
            session_service,
            config.homeserver_name.clone(),
            config_static,
            http_client,
            event_signer,
        );

        Self {
            base_url: server.server_address().unwrap().to_string(),
            server,
            mock_server,
            app_state,
        }
    }

    pub async fn test_endpoint(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> TestResponse {
        match method {
            "GET" => self.server.get(path).await,
            "POST" => {
                let mut request = self.server.post(path);
                if let Some(body) = body {
                    request = request.json(&body);
                }
                request.await
            },
            "PUT" => {
                let mut request = self.server.put(path);
                if let Some(body) = body {
                    request = request.json(&body);
                }
                request.await
            },
            "DELETE" => self.server.delete(path).await,
            _ => panic!("Unsupported HTTP method: {}", method),
        }
    }

    pub async fn test_authenticated_endpoint(
        &self,
        method: &str,
        path: &str,
        access_token: &str,
        body: Option<Value>,
    ) -> TestResponse {
        let auth_header = format!("Bearer {}", access_token);

        match method {
            "GET" => self.server.get(path).add_header("Authorization", auth_header).await,
            "POST" => {
                let mut request = self.server.post(path).add_header("Authorization", auth_header);
                if let Some(body) = body {
                    request = request.json(&body);
                }
                request.await
            },
            "PUT" => {
                let mut request = self.server.put(path).add_header("Authorization", auth_header);
                if let Some(body) = body {
                    request = request.json(&body);
                }
                request.await
            },
            "DELETE" => self.server.delete(path).add_header("Authorization", auth_header).await,
            _ => panic!("Unsupported HTTP method: {}", method),
        }
    }
}

/// Create test application with all routes
async fn create_test_app() -> Router {
    // Initialize test database
    let db_any = any::connect("memory").await.expect("Failed to connect to test database");
    db_any
        .use_ns("test")
        .use_db("matrix")
        .await
        .expect("Failed to select test namespace");

    // Run test schema
    let schema = include_str!("../../../../surrealdb/migrations/matryx.surql");
    db_any.query(schema).await.expect("Failed to execute test schema");

    // Create required components for AppState
    use matryx_server::config::{EmailConfig, PushCacheConfig, SmsConfig};
    use matryx_server::{MatrixSessionService, ServerConfig};
    use std::sync::Arc;

    let config = ServerConfig {
        homeserver_name: "test.localhost".to_string(),
        federation_port: 8448,
        media_base_url: "https://test.localhost".to_string(),
        admin_email: "admin@test.localhost".to_string(),
        environment: "test".to_string(),
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
    };

    let jwt_secret = "test_secret".to_string().into_bytes();
    let session_repo = matryx_surrealdb::repository::session::SessionRepository::new(db_any.clone());
    let key_server_repo = matryx_surrealdb::repository::key_server::KeyServerRepository::new(db_any.clone());
    let session_service =
        Arc::new(MatrixSessionService::new(jwt_secret, config.homeserver_name.clone(), session_repo, key_server_repo));

    let http_client = Arc::new(reqwest::Client::new());
    let event_signer = Arc::new(matryx_server::federation::event_signer::EventSigner::new(
        session_service.clone(),
        db_any.clone(),
        config.homeserver_name.clone(),
        "ed25519:auto".to_string(),
    ));

    let config_static: &'static ServerConfig = Box::leak(Box::new(config.clone()));
    let app_state = AppState::new(
        db_any,
        session_service,
        config.homeserver_name.clone(),
        config_static,
        http_client,
        event_signer,
    );

    // Create a simple test router for now
    Router::new()
        .route("/test", get(|| async { "test" }))
        .with_state(app_state)
}

/// Test configuration for isolated test database
pub fn test_database_config() -> HashMap<String, String> {
    let mut config = HashMap::new();
    config.insert("url".to_string(), "memory://".to_string());
    config.insert("namespace".to_string(), "test".to_string());
    config.insert("database".to_string(), "matrix_test".to_string());
    config
}

/// Helper function to create test user
pub async fn create_test_user(
    server: &MatrixTestServer,
    username: &str,
    password: &str,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    let register_body = json!({
        "username": username,
        "password": password,
        "auth": {
            "type": "m.login.dummy"
        }
    });

    let response = server
        .test_endpoint("POST", "/_matrix/client/v3/register", Some(register_body))
        .await;

    if response.status_code() == 200 {
        let body: Value = response.json();
        let user_id = body["user_id"].as_str().unwrap().to_string();
        let access_token = body["access_token"].as_str().unwrap().to_string();
        Ok((user_id, access_token))
    } else {
        Err(format!("Failed to create test user: {}", response.status_code()).into())
    }
}

/// Helper function to create test room
pub async fn create_test_room(
    server: &MatrixTestServer,
    access_token: &str,
    room_name: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let room_body = json!({
        "name": room_name,
        "preset": "public_chat",
        "room_version": "10"
    });

    let response = server
        .test_authenticated_endpoint(
            "POST",
            "/_matrix/client/v3/createRoom",
            access_token,
            Some(room_body),
        )
        .await;

    if response.status_code() == 200 {
        let body: Value = response.json();
        let room_id = body["room_id"].as_str().unwrap().to_string();
        Ok(room_id)
    } else {
        Err(format!("Failed to create test room: {}", response.status_code()).into())
    }
}
