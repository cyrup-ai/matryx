//! Module contains intentional library code not yet fully integrated
#![allow(dead_code)]

//! Standardized response types for Matrix API compliance

use crate::error::MatrixError;
use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;

/// Standard Matrix API response wrapper
#[derive(Debug, Serialize)]
pub struct MatrixResponse<T> {
    #[serde(flatten)]
    pub data: T,
}

impl<T: Serialize> MatrixResponse<T> {
    pub fn new(data: T) -> Self {
        Self { data }
    }
}

impl<T: Serialize> IntoResponse for MatrixResponse<T> {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::OK, Json(self.data)).into_response()
    }
}

/// Helper function for creating successful Matrix responses
pub fn matrix_ok<T: Serialize>(data: T) -> impl IntoResponse {
    MatrixResponse::new(data)
}

/// Helper function for creating Matrix error responses
pub fn matrix_error(error: MatrixError) -> impl IntoResponse {
    error.into_response()
}

/// Standard Matrix API result type
pub type MatrixResult<T> = Result<MatrixResponse<T>, MatrixError>;
