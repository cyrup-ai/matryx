use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use thiserror::Error;

/// Matrix authentication errors following Matrix specification error codes
#[derive(Error, Debug)]
pub enum MatrixAuthError {
    #[error("Missing access token")]
    MissingToken,

    #[error("Unknown or invalid access token")]
    UnknownToken,

    #[error("Access forbidden")]
    Forbidden,

    #[error("Invalid server signature")]
    InvalidSignature,

    #[error("Missing Authorization header")]
    MissingAuthorization,

    #[error("Invalid X-Matrix authorization format")]
    InvalidXMatrixFormat,

    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Session expired")]
    SessionExpired,

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("JWT error: {0}")]
    JwtError(String),
}

impl IntoResponse for MatrixAuthError {
    fn into_response(self) -> Response {
        let (status, error_code, message) = match self {
            MatrixAuthError::MissingToken => {
                (StatusCode::UNAUTHORIZED, "M_MISSING_TOKEN", "Missing access token")
            },
            MatrixAuthError::UnknownToken => {
                (StatusCode::UNAUTHORIZED, "M_UNKNOWN_TOKEN", "Unknown or invalid access token")
            },
            MatrixAuthError::Forbidden => {
                (StatusCode::FORBIDDEN, "M_FORBIDDEN", "Access forbidden")
            },
            MatrixAuthError::InvalidSignature => {
                (StatusCode::UNAUTHORIZED, "M_UNAUTHORIZED", "Invalid server signature")
            },
            MatrixAuthError::MissingAuthorization => {
                (StatusCode::UNAUTHORIZED, "M_UNAUTHORIZED", "Missing Authorization header")
            },
            MatrixAuthError::InvalidXMatrixFormat => (
                StatusCode::UNAUTHORIZED,
                "M_UNAUTHORIZED",
                "Invalid X-Matrix authorization format",
            ),
            MatrixAuthError::InvalidCredentials => {
                (StatusCode::UNAUTHORIZED, "M_FORBIDDEN", "Invalid credentials")
            },
            MatrixAuthError::SessionExpired => {
                (StatusCode::UNAUTHORIZED, "M_UNKNOWN_TOKEN", "Session expired")
            },
            MatrixAuthError::DatabaseError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "M_UNKNOWN", "Internal server error")
            },
            MatrixAuthError::JwtError(_) => {
                (StatusCode::UNAUTHORIZED, "M_UNKNOWN_TOKEN", "Invalid token format")
            },
        };

        let body = Json(json!({
            "errcode": error_code,
            "error": message
        }));

        (status, body).into_response()
    }
}

impl From<MatrixAuthError> for StatusCode {
    fn from(error: MatrixAuthError) -> Self {
        match error {
            MatrixAuthError::MissingToken => StatusCode::UNAUTHORIZED,
            MatrixAuthError::UnknownToken => StatusCode::UNAUTHORIZED,
            MatrixAuthError::Forbidden => StatusCode::FORBIDDEN,
            MatrixAuthError::InvalidSignature => StatusCode::UNAUTHORIZED,
            MatrixAuthError::MissingAuthorization => StatusCode::UNAUTHORIZED,
            MatrixAuthError::InvalidXMatrixFormat => StatusCode::UNAUTHORIZED,
            MatrixAuthError::InvalidCredentials => StatusCode::UNAUTHORIZED,
            MatrixAuthError::SessionExpired => StatusCode::UNAUTHORIZED,
            MatrixAuthError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            MatrixAuthError::JwtError(_) => StatusCode::UNAUTHORIZED,
        }
    }
}
