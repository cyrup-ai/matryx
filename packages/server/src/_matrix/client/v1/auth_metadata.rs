use crate::error::MatrixError;
use axum::Json;
use serde_json::Value;

/// GET /_matrix/client/v1/auth_metadata
///
/// Discovery endpoint for OAuth 2.0 API support.
/// Returns 404 with M_UNRECOGNIZED to indicate OAuth 2.0 is not (publicly) supported.
///
/// Clients receiving this error will fall back to the password login API (/_matrix/client/v3/login),
/// which is fully implemented and functional in Matryx.
///
/// Matrix Specification: https://spec.matrix.org/v1.15/client-server-api/#get_matrixclientv1auth_metadata
/// MSC Reference: MSC2965
///
/// Note: OAuth 2.0 infrastructure exists in Matryx but is not yet publicly exposed.
/// See packages/server/src/auth/oauth2.rs for implementation details.
pub async fn get() -> Result<Json<Value>, MatrixError> {
    // Return M_UNRECOGNIZED to signal OAuth 2.0 API not supported
    // MatrixError::Unrecognized automatically formats as:
    // {
    //   "errcode": "M_UNRECOGNIZED",
    //   "error": "Unrecognized request"
    // }
    // with HTTP 404 status
    Err(MatrixError::Unrecognized)
}
