use axum::extract::ConnectInfo;
use axum::http::HeaderMap;
use axum::{Json, extract::State, http::StatusCode};
use std::net::SocketAddr;

use crate::_matrix::client::v3::login::{LoginRequest, LoginResponse};
use crate::state::AppState;

/// POST /_matrix/client/login
///
/// Delegates to the comprehensive v3 login implementation which handles
/// password, token, SSO, and application service authentication flows.
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    // Delegate to v3 login implementation
    crate::_matrix::client::v3::login::post(State(state), ConnectInfo(addr), tower_cookies::Cookies::default(), headers, Json(request))
        .await
}
