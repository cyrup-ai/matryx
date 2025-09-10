use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    response::Response,
    routing::get,
    Router,
};
use chrono::Utc;
use serde_json::Value;
use surrealdb::{engine::local::Mem, Surreal};
use tower::ServiceExt;

use matryx_entity::types::Event;
use matryx_server::_matrix::federation::v1::event::by_event_id;
use matryx_surrealdb::repository::event::EventRepository;

async fn setup_test_db() -> Surreal<surrealdb::engine::local::Db> {
    let db = Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("test").use_db("matrix").await.unwrap();
    
    // Create event table
    db.query("DEFINE TABLE event SCHEMAFULL").await.unwrap();
    db.query("DEFINE FIELD event_id ON TABLE event TYPE string").await.unwrap();
    db.query("DEFINE FIELD room_id ON TABLE event TYPE string").await.unwrap();
    db.query("DEFINE FIELD sender ON TABLE event TYPE string").await.unwrap();
    db.query("DEFINE FIELD event_type ON TABLE event TYPE string").await.unwrap();
    db.query("DEFINE FIELD content ON TABLE event TYPE object DEFAULT {}").await.unwrap();
    db.query("DEFINE FIELD state_key ON TABLE event TYPE option<string>").await.unwrap();
    db.query("DEFINE FIELD origin_server_ts ON TABLE event TYPE datetime DEFAULT time::now()").await.unwrap();
    db.query("DEFINE FIELD unsigned ON TABLE event TYPE option<object>").await.unwrap();
    db.query("DEFINE FIELD prev_events ON TABLE event TYPE array<string> DEFAULT []").await.unwrap();
    db.query("DEFINE FIELD auth_events ON TABLE event TYPE array<string> DEFAULT []").await.unwrap();
    db.query("DEFINE FIELD depth ON TABLE event TYPE int DEFAULT 0").await.unwrap();
    db.query("DEFINE FIELD hashes ON TABLE event TYPE object DEFAULT {}").await.unwrap();
    db.query("DEFINE FIELD signatures ON TABLE event TYPE object DEFAULT {}").await.unwrap();
    db.query("DEFINE FIELD redacts ON TABLE event TYPE option<string>").await.unwrap();
    db.query("DEFINE FIELD outlier ON TABLE event TYPE bool DEFAULT false").await.unwrap();
    db.query("DEFINE FIELD rejected_reason ON TABLE event TYPE option<string>").await.unwrap();
    db.query("DEFINE FIELD soft_failed ON TABLE event TYPE bool DEFAULT false").await.unwrap();
    db.query("DEFINE FIELD received_ts ON TABLE event TYPE datetime DEFAULT time::now()").await.unwrap();
    
    db
}

async fn create_test_event(db: &Surreal<surrealdb::engine::local::Db>) -> Event {
    let event = Event {
        event_id: "$test_event:example.com".to_string(),
        room_id: "!test_room:example.com".to_string(),
        sender: "@test_user:example.com".to_string(),
        event_type: "m.room.message".to_string(),
        content: serde_json::json!({"body": "Hello, world!", "msgtype": "m.text"}),
        state_key: None,
        origin_server_ts: Utc::now(),
        unsigned: None,
        prev_events: vec!["$prev_event:example.com".to_string()],
        auth_events: vec!["$auth_event:example.com".to_string()],
        depth: 1,
        hashes: serde_json::json!({"sha256": "test_hash"}),
        signatures: serde_json::json!({"example.com": {"ed25519:key1": "test_signature"}}),
        redacts: None,
        outlier: false,
        rejected_reason: None,
        soft_failed: false,
        received_ts: Utc::now(),
    };

    let repo = EventRepository::new(db.clone());
    repo.create(&event).await.unwrap()
}

fn create_test_app(db: Surreal<surrealdb::engine::local::Db>) -> Router {
    Router::new()
        .route("/_matrix/federation/v1/event/:event_id", get(by_event_id::get))
        .with_state(db)
}

#[tokio::test]
async fn test_get_event_success() {
    let db = setup_test_db().await;
    let event = create_test_event(&db).await;
    let app = create_test_app(db);

    let request = Request::builder()
        .uri(&format!("/_matrix/federation/v1/event/{}", event.event_id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(response_json["origin"], "matryx.local");
    assert!(response_json["pdus"].is_array());
    assert_eq!(response_json["pdus"].as_array().unwrap().len(), 1);
    
    let returned_event = &response_json["pdus"][0];
    assert_eq!(returned_event["event_id"], event.event_id);
    assert_eq!(returned_event["room_id"], event.room_id);
    assert_eq!(returned_event["sender"], event.sender);
}

#[tokio::test]
async fn test_get_event_not_found() {
    let db = setup_test_db().await;
    let app = create_test_app(db);

    let request = Request::builder()
        .uri("/_matrix/federation/v1/event/$nonexistent:example.com")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(response_json["errcode"], "M_NOT_FOUND");
    assert!(response_json["error"].as_str().unwrap().contains("not found"));
}

#[tokio::test]
async fn test_get_event_invalid_id() {
    let db = setup_test_db().await;
    let app = create_test_app(db);

    let request = Request::builder()
        .uri("/_matrix/federation/v1/event/invalid_event_id")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let response_json: Value = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(response_json["errcode"], "M_INVALID_PARAM");
    assert!(response_json["error"].as_str().unwrap().contains("Invalid event ID"));
}

#[tokio::test]
async fn test_livequery_event_updates() {
    let db = setup_test_db().await;
    let event = create_test_event(&db).await;
    
    let repo = EventRepository::new(db.clone());
    let mut stream = repo.subscribe_event(&event.event_id);
    
    // Update the event to trigger LiveQuery
    let mut updated_event = event.clone();
    updated_event.content = serde_json::json!({"body": "Updated message", "msgtype": "m.text"});
    
    // This would trigger the LiveQuery stream in a real scenario
    // For this test, we're just verifying the stream can be created
    assert!(stream.as_mut().is_some());
}

#[tokio::test]
async fn test_event_repository_operations() {
    let db = setup_test_db().await;
    let repo = EventRepository::new(db);
    
    // Test creating an event
    let event = Event {
        event_id: "$repo_test:example.com".to_string(),
        room_id: "!repo_room:example.com".to_string(),
        sender: "@repo_user:example.com".to_string(),
        event_type: "m.room.message".to_string(),
        content: serde_json::json!({"body": "Repository test", "msgtype": "m.text"}),
        state_key: None,
        origin_server_ts: Utc::now(),
        unsigned: None,
        prev_events: vec![],
        auth_events: vec![],
        depth: 0,
        hashes: serde_json::json!({}),
        signatures: serde_json::json!({}),
        redacts: None,
        outlier: false,
        rejected_reason: None,
        soft_failed: false,
        received_ts: Utc::now(),
    };
    
    let created_event = repo.create(&event).await.unwrap();
    assert_eq!(created_event.event_id, event.event_id);
    
    // Test retrieving the event
    let retrieved_event = repo.get_by_id(&event.event_id).await.unwrap();
    assert!(retrieved_event.is_some());
    assert_eq!(retrieved_event.unwrap().event_id, event.event_id);
    
    // Test retrieving non-existent event
    let non_existent = repo.get_by_id("$does_not_exist:example.com").await.unwrap();
    assert!(non_existent.is_none());
}
