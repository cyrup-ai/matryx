use axum::Router;
use std::sync::Arc;
use surrealdb::{Surreal, engine::local::Mem};
use matryx_server::{create_app, AppState};

pub async fn create_test_app() -> Router {
    // Create in-memory database for testing
    let db = Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("test").use_db("matrix").await.unwrap();
    
    // Initialize test schema
    let schema = include_str!("../../../surrealdb/migrations/matryx.surql");
    db.query(schema).await.unwrap();
    
    // Create app state
    let state = AppState {
        db: Arc::new(db),
        // Add other required state fields
    };
    
    // Create app with test state
    create_app(state).await
}