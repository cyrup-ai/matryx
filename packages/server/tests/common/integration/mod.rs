use axum::{Router, routing::get};
use axum_test::{TestResponse, TestServer};
use matryx_server::state::AppState;
use serde_json::{Value, json};
use std::collections::HashMap;
use surrealdb::engine::any::{self};
use wiremock::MockServer;
use tokio::sync::mpsc;
use matryx_server::federation::outbound_queue::OutboundEvent;

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
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let app = create_test_app().await?;
        let server = TestServer::new(app)?;
        let mock_server = MockServer::start().await;

        // Create test app state
        let db_any = any::connect("surrealkv://test_data/integration_test.db")
            .await?;
        db_any
            .use_ns("test")
            .use_db("matrix")
            .await?;

        // Create required components for AppState
        use matryx_server::config::{EmailConfig, PushCacheConfig, SmsConfig};
        use matryx_server::middleware::TransactionConfig;
        use matryx_server::{MatrixSessionService, ServerConfig};
        use std::sync::Arc;

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
            tls_config: matryx_server::config::TlsConfig::default(),
            rate_limiting: matryx_server::config::RateLimitConfig::default(),
            captcha: matryx_server::auth::captcha::CaptchaConfig::from_env(),
        };

        let jwt_secret = "test_secret".to_string().into_bytes();
        let session_repo =
            matryx_surrealdb::repository::session::SessionRepository::new(db_any.clone());
        let key_server_repo =
            matryx_surrealdb::repository::key_server::KeyServerRepository::new(db_any.clone());
        let session_service = Arc::new(MatrixSessionService::new(
            &jwt_secret,
            &jwt_secret, // Using same secret for public key in tests
            config.homeserver_name.clone(),
            session_repo,
            key_server_repo,
        ));

        let http_client = Arc::new(reqwest::Client::new());

        // Create DNS resolver for event signer
        let well_known_client = Arc::new(
            matryx_server::federation::well_known_client::WellKnownClient::new(http_client.clone()),
        );
        let dns_resolver = Arc::new(
            matryx_server::federation::dns_resolver::MatrixDnsResolver::new(well_known_client)?,
        );

        let event_signer = Arc::new(
            matryx_server::federation::event_signer::EventSigner::new(
                session_service.clone(),
                db_any.clone(),
                dns_resolver.clone(),
                config.homeserver_name.clone(),
                "ed25519:auto".to_string(),
            )?,
        );

        let config_static: &'static ServerConfig = Box::leak(Box::new(config.clone()));
        
        // Create outbound channel for federation queue (tests don't spawn background task)
        let (outbound_tx, _outbound_rx) = mpsc::unbounded_channel();
        
        let app_state = AppState::new(
            db_any,
            session_service,
            config.homeserver_name.clone(),
            config_static,
            http_client,
            event_signer,
            dns_resolver,
            outbound_tx,
        )?;

        Ok(Self {
            base_url: server.server_address()?
                .to_string(),
            server,
            mock_server,
            app_state,
        })
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

    /// Set up federation mock for testing server-to-server interactions
    pub async fn setup_federation_mock(&self, homeserver_name: &str) {
        use wiremock::{Mock, ResponseTemplate};

        // Mock well-known server discovery
        Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/.well-known/matrix/server"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "m.server": format!("{}:8448", homeserver_name)
            })))
            .mount(&self.mock_server)
            .await;

        // Mock server version endpoint
        Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/_matrix/federation/v1/version"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "server": {
                    "name": "matryx",
                    "version": "0.1.0"
                }
            })))
            .mount(&self.mock_server)
            .await;
    }

    /// Get direct access to the app state for advanced testing scenarios
    pub fn get_app_state(&self) -> &AppState {
        &self.app_state
    }

    /// Test federation endpoint by making request to mock server
    pub async fn test_federation_request(&self, path: &str) -> TestResponse {
        let url = format!("{}{}", self.mock_server.uri(), path);
        self.server.get(&url).await
    }

    /// Access mock server for custom federation testing scenarios
    pub fn get_mock_server(&self) -> &MockServer {
        &self.mock_server
    }
}

/// Create test application with all routes
async fn create_test_app() -> Result<Router, Box<dyn std::error::Error>> {
    // Initialize test database
    let db_any = any::connect("memory").await?;
    db_any
        .use_ns("test")
        .use_db("matrix")
        .await?;

    // Run test schema
    let schema = include_str!("../../../../surrealdb/migrations/matryx.surql");
    db_any.query(schema).await?;

    // Create required components for AppState
    use matryx_server::config::{EmailConfig, PushCacheConfig, SmsConfig};
    use matryx_server::middleware::TransactionConfig;
    use matryx_server::{MatrixSessionService, ServerConfig};
    use std::sync::Arc;

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
        tls_config: matryx_server::config::TlsConfig::default(),
        rate_limiting: matryx_server::config::RateLimitConfig::default(),
        captcha: matryx_server::auth::captcha::CaptchaConfig::from_env(),
    };

    let jwt_secret = "test_secret".to_string().into_bytes();
    let session_repo =
        matryx_surrealdb::repository::session::SessionRepository::new(db_any.clone());
    let key_server_repo =
        matryx_surrealdb::repository::key_server::KeyServerRepository::new(db_any.clone());
    let session_service = Arc::new(MatrixSessionService::new(
        &jwt_secret,
        &jwt_secret, // Using same secret for public key in tests
        config.homeserver_name.clone(),
        session_repo,
        key_server_repo,
    ));

    let http_client = Arc::new(reqwest::Client::new());

    // Create DNS resolver for event signer
    let well_known_client = Arc::new(
        matryx_server::federation::well_known_client::WellKnownClient::new(http_client.clone()),
    );
    let dns_resolver = Arc::new(
        matryx_server::federation::dns_resolver::MatrixDnsResolver::new(well_known_client)
            .expect("Failed to create DNS resolver"),
    );

    let event_signer = Arc::new(
        matryx_server::federation::event_signer::EventSigner::new(
            session_service.clone(),
            db_any.clone(),
            dns_resolver.clone(),
            config.homeserver_name.clone(),
            "ed25519:auto".to_string(),
        )
        .expect("Failed to create test event signer"),
    );

    let config_static: &'static ServerConfig = Box::leak(Box::new(config.clone()));
    
    // Create outbound channel for federation queue (tests don't spawn background task)
    let (outbound_tx, _outbound_rx) = mpsc::unbounded_channel();
    
    let app_state = AppState::new(
        db_any,
        session_service,
        config.homeserver_name.clone(),
        config_static,
        http_client,
        event_signer,
        dns_resolver,
        outbound_tx,
    )?;

    // Create a simple test router for now
    Ok(Router::new()
        .route("/test", get(|| async { "test" }))
        .with_state(app_state))
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
        let user_id = body["user_id"].as_str()
            .ok_or("Test assertion: registration response must contain user_id field")?
            .to_string();
        let access_token = body["access_token"].as_str()
            .ok_or("Test assertion: registration response must contain access_token field")?
            .to_string();
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
        let room_id = body["room_id"].as_str()
            .ok_or("Test assertion: createRoom response must contain room_id field")?
            .to_string();
        Ok(room_id)
    } else {
        Err(format!("Failed to create test room: {}", response.status_code()).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_matrix_test_server_functionality() -> Result<(), Box<dyn std::error::Error>> {
        let server = MatrixTestServer::new().await?;

        // Test accessing app state
        let _app_state = server.get_app_state();

        // Test accessing mock server
        let _mock_server = server.get_mock_server();

        // Set up federation mock
        server.setup_federation_mock("test.localhost").await;

        // Test federation request
        let _response = server.test_federation_request("/_matrix/federation/v1/version").await;
        Ok(())
    }

    #[tokio::test]
    async fn test_helper_functions() -> Result<(), Box<dyn std::error::Error>> {
        let _config = test_database_config();
        let _app = create_test_app().await?;
        Ok(())
    }
}
