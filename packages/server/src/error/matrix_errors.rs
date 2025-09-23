use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};
use std::collections::HashMap;
use thiserror::Error;

/// Complete Matrix error code system following Matrix specification
#[derive(Error, Debug)]
pub enum MatrixError {
    // Authentication & Authorization
    #[error("Forbidden access")]
    Forbidden,
    #[error("Unknown or invalid access token")]
    UnknownToken { soft_logout: bool },
    #[error("Missing access token")]
    MissingToken,
    #[error("The request was not correctly authorized")]
    Unauthorized,
    #[error("User account locked")]
    UserLocked,
    #[error("User account suspended")]
    UserSuspended,
    #[error("User account deactivated")]
    UserDeactivated,

    // Request Format
    #[error("Invalid JSON in request")]
    BadJson,
    #[error("Request body is not valid JSON")]
    NotJson,
    #[error("Resource not found")]
    NotFound,
    #[error("Unrecognized request")]
    Unrecognized,
    #[error("Request entity too large")]
    TooLarge,
    #[error("Missing required parameters")]
    MissingParams,
    #[error("Invalid parameter value")]
    InvalidParam,

    // Rate Limiting
    #[error("Rate limit exceeded")]
    LimitExceeded { retry_after_ms: Option<u64> },

    // User/Room Management
    #[error("Username already in use")]
    UserInUse,
    #[error("Invalid username format")]
    InvalidUsername,
    #[error("Room alias already in use")]
    RoomInUse,
    #[error("Invalid room state")]
    InvalidRoomState,
    #[error("Third-party identifier already in use")]
    ThreepidInUse,

    // Federation specific
    #[error("Unable to authorize join")]
    UnableToAuthorizeJoin,
    #[error("Unable to grant join")]
    UnableToGrantJoin,
    #[error("Invalid server signature")]
    InvalidSignature,

    // Session management
    #[error("Session not validated")]
    SessionNotValidated,
    #[error("No valid session")]
    NoValidSession,
    #[error("Session expired")]
    SessionExpired,

    // Generic
    #[error("Unknown server error")]
    Unknown,
}

impl MatrixError {
    /// Convert error to response parts (status, errcode, message, extra_fields)
    pub fn to_response_parts(
        &self,
    ) -> (StatusCode, &'static str, String, Option<HashMap<String, Value>>) {
        match self {
            MatrixError::Forbidden => {
                (StatusCode::FORBIDDEN, "M_FORBIDDEN", self.to_string(), None)
            },
            MatrixError::UnknownToken { soft_logout } => {
                let mut extra = HashMap::new();
                extra.insert("soft_logout".to_string(), Value::Bool(*soft_logout));
                (StatusCode::UNAUTHORIZED, "M_UNKNOWN_TOKEN", self.to_string(), Some(extra))
            },
            MatrixError::MissingToken => {
                (StatusCode::UNAUTHORIZED, "M_MISSING_TOKEN", self.to_string(), None)
            },
            MatrixError::Unauthorized => {
                (StatusCode::UNAUTHORIZED, "M_UNAUTHORIZED", self.to_string(), None)
            },
            MatrixError::UserLocked => {
                (StatusCode::UNAUTHORIZED, "M_USER_LOCKED", self.to_string(), None)
            },
            MatrixError::UserSuspended => {
                (StatusCode::UNAUTHORIZED, "M_USER_SUSPENDED", self.to_string(), None)
            },
            MatrixError::UserDeactivated => {
                (StatusCode::FORBIDDEN, "M_USER_DEACTIVATED", self.to_string(), None)
            },
            MatrixError::BadJson => (StatusCode::BAD_REQUEST, "M_BAD_JSON", self.to_string(), None),
            MatrixError::NotJson => (StatusCode::BAD_REQUEST, "M_NOT_JSON", self.to_string(), None),
            MatrixError::NotFound => (StatusCode::NOT_FOUND, "M_NOT_FOUND", self.to_string(), None),
            MatrixError::Unrecognized => {
                (StatusCode::NOT_FOUND, "M_UNRECOGNIZED", self.to_string(), None)
            },
            MatrixError::TooLarge => {
                (StatusCode::PAYLOAD_TOO_LARGE, "M_TOO_LARGE", self.to_string(), None)
            },
            MatrixError::MissingParams => {
                (StatusCode::BAD_REQUEST, "M_MISSING_PARAMS", self.to_string(), None)
            },
            MatrixError::InvalidParam => {
                (StatusCode::BAD_REQUEST, "M_INVALID_PARAM", self.to_string(), None)
            },
            MatrixError::LimitExceeded { retry_after_ms } => {
                let mut extra = HashMap::new();
                if let Some(retry_ms) = retry_after_ms {
                    extra.insert("retry_after_ms".to_string(), Value::Number((*retry_ms).into()));
                }
                let extra_opt = if extra.is_empty() { None } else { Some(extra) };
                (StatusCode::TOO_MANY_REQUESTS, "M_LIMIT_EXCEEDED", self.to_string(), extra_opt)
            },
            MatrixError::UserInUse => {
                (StatusCode::BAD_REQUEST, "M_USER_IN_USE", self.to_string(), None)
            },
            MatrixError::InvalidUsername => {
                (StatusCode::BAD_REQUEST, "M_INVALID_USERNAME", self.to_string(), None)
            },
            MatrixError::RoomInUse => {
                (StatusCode::BAD_REQUEST, "M_ROOM_IN_USE", self.to_string(), None)
            },
            MatrixError::InvalidRoomState => {
                (StatusCode::BAD_REQUEST, "M_INVALID_ROOM_STATE", self.to_string(), None)
            },
            MatrixError::ThreepidInUse => {
                (StatusCode::BAD_REQUEST, "M_THREEPID_IN_USE", self.to_string(), None)
            },
            MatrixError::UnableToAuthorizeJoin => {
                (StatusCode::BAD_REQUEST, "M_UNABLE_TO_AUTHORISE_JOIN", self.to_string(), None)
            },
            MatrixError::UnableToGrantJoin => {
                (StatusCode::BAD_REQUEST, "M_UNABLE_TO_GRANT_JOIN", self.to_string(), None)
            },
            MatrixError::InvalidSignature => {
                (StatusCode::UNAUTHORIZED, "M_UNAUTHORIZED", self.to_string(), None)
            },
            MatrixError::SessionNotValidated => {
                (StatusCode::BAD_REQUEST, "M_SESSION_NOT_VALIDATED", self.to_string(), None)
            },
            MatrixError::NoValidSession => {
                (StatusCode::BAD_REQUEST, "M_NO_VALID_SESSION", self.to_string(), None)
            },
            MatrixError::SessionExpired => {
                (StatusCode::UNAUTHORIZED, "M_SESSION_EXPIRED", self.to_string(), None)
            },
            MatrixError::Unknown => {
                (StatusCode::INTERNAL_SERVER_ERROR, "M_UNKNOWN", self.to_string(), None)
            },
        }
    }
}

impl IntoResponse for MatrixError {
    fn into_response(self) -> Response {
        let (status, errcode, message, extra) = self.to_response_parts();
        let mut response = json!({
            "errcode": errcode,
            "error": message
        });

        if let Some(extra_fields) = extra {
            if let serde_json::Value::Object(ref mut map) = response {
                for (key, value) in extra_fields {
                    map.insert(key, value);
                }
            }
        }

        (status, Json(response)).into_response()
    }
}

impl From<MatrixError> for StatusCode {
    fn from(error: MatrixError) -> Self {
        let (status, _, _, _) = error.to_response_parts();
        status
    }
}

// Conversion from existing auth errors
impl From<crate::auth::errors::MatrixAuthError> for MatrixError {
    fn from(auth_error: crate::auth::errors::MatrixAuthError) -> Self {
        match auth_error {
            crate::auth::errors::MatrixAuthError::MissingToken => MatrixError::MissingToken,
            crate::auth::errors::MatrixAuthError::UnknownToken => {
                MatrixError::UnknownToken { soft_logout: false }
            },
            crate::auth::errors::MatrixAuthError::Forbidden => MatrixError::Forbidden,
            crate::auth::errors::MatrixAuthError::InvalidSignature => MatrixError::InvalidSignature,
            crate::auth::errors::MatrixAuthError::MissingAuthorization => MatrixError::Unauthorized,
            crate::auth::errors::MatrixAuthError::InvalidXMatrixFormat => MatrixError::Unauthorized,
            crate::auth::errors::MatrixAuthError::SessionExpired => MatrixError::SessionExpired,
            crate::auth::errors::MatrixAuthError::DatabaseError(_) => MatrixError::Unknown,
            crate::auth::errors::MatrixAuthError::JwtError(_) => {
                MatrixError::UnknownToken { soft_logout: false }
            },
        }
    }
}
