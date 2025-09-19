use axum::{Router, response::Json};
use axum_test::{TestServer, TestResponse};
use wiremock::{MockServer, Mock, ResponseTemplate};
use serde_json::{json, Value};
use std::collections::HashMap;
use tokio::sync::OnceCell;
use matryx_server::state::AppState;
use surrealdb::{Surreal, engine::local::Mem};

pub mod client_compatibility;
pub mod database;
pub mod compliance;
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
        let db = Surreal::new::<Mem>(()).await.unwrap();
        db.use_ns("test").use_db("matrix").await.unwrap();
        
        let app_state = AppState {
            db: db.clone(),
            homeserver_name: "test.localhost".to_string(),
            server_name: "test.localhost".to_string(),
            signing_key: ed25519_dalek::SigningKey::generate(&mut rand::thread_rng()),
        };
        
        Self {
            base_url: server.server_address().unwrap(),
            server,
            mock_server,
            app_state,
        }
    }
    
    pub async fn test_endpoint(&self, method: &str, path: &str, body: Option<Value>) -> TestResponse {
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
    
    pub async fn test_authenticated_endpoint(&self, method: &str, path: &str, access_token: &str, body: Option<Value>) -> TestResponse {
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
    let db = Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("test").use_db("matrix").await.unwrap();
    
    // Run test schema
    let schema = include_str!("../../../surrealdb/migrations/matryx.surql");
    db.query(schema).await.unwrap();
    
    let app_state = AppState {
        db: db.clone(),
        homeserver_name: "test.localhost".to_string(),
        server_name: "test.localhost".to_string(),
        signing_key: ed25519_dalek::SigningKey::generate(&mut rand::thread_rng()),
    };
    
    // Create the main application router
    matryx_server::create_app(app_state).await
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
pub async fn create_test_user(server: &MatrixTestServer, username: &str, password: &str) -> Result<(String, String), Box<dyn std::error::Error>> {
    let register_body = json!({
        "username": username,
        "password": password,
        "auth": {
            "type": "m.login.dummy"
        }
    });
    
    let response = server.test_endpoint("POST", "/_matrix/client/v3/register", Some(register_body)).await;
    
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
pub async fn create_test_room(server: &MatrixTestServer, access_token: &str, room_name: &str) -> Result<String, Box<dyn std::error::Error>> {
    let room_body = json!({
        "name": room_name,
        "preset": "public_chat",
        "room_version": "10"
    });
    
    let response = server.test_authenticated_endpoint("POST", "/_matrix/client/v3/createRoom", access_token, Some(room_body)).await;
    
    if response.status_code() == 200 {
        let body: Value = response.json();
        let room_id = body["room_id"].as_str().unwrap().to_string();
        Ok(room_id)
    } else {
        Err(format!("Failed to create test room: {}", response.status_code()).into())
    }
}