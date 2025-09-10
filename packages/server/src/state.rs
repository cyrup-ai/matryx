use crate::auth::MatrixSessionService;
use std::sync::Arc;
use surrealdb::{Surreal, engine::any::Any};

#[derive(Clone)]
pub struct AppState {
    pub db: Surreal<Any>,
    pub session_service: Arc<MatrixSessionService>,
    pub homeserver_name: String,
}

impl AppState {
    pub fn new(
        db: Surreal<Any>,
        session_service: Arc<MatrixSessionService>,
        homeserver_name: String,
    ) -> Self {
        Self { db, session_service, homeserver_name }
    }
}
