//! Centralized error handling for Matrix API compliance

pub mod matrix_errors;

pub use matrix_errors::MatrixError;

/// Helper function to create standardized Matrix error responses
pub fn matrix_error_response(error: MatrixError) -> impl axum::response::IntoResponse {
    error
}
