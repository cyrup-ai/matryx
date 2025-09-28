//! Matrix Federation Request Signing
//!
//! Implements the complete Matrix JSON signing algorithm for server-to-server
//! HTTP request authentication, following the Matrix Server-Server API specification
//! for X-Matrix authorization headers.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use axum::http::HeaderMap;

use crate::federation::event_signing::{EventSigningEngine, EventSigningError};
use crate::auth::x_matrix_parser::{parse_x_matrix_header, XMatrixAuth};
use crate::error::MatrixError;

/// JSON structure for signing federation requests per Matrix Server-Server API specification
#[derive(Debug, Serialize, Deserialize)]
pub struct FederationRequestSigningData {
    pub method: String,
    pub uri: String,
    pub origin: String,
    pub destination: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Value>,
}

impl FederationRequestSigningData {
    pub fn new(
        method: &str,
        uri: &str,
        origin: &str,
        destination: &str,
        content: Option<Value>,
    ) -> Self {
        Self {
            method: method.to_string(),
            uri: uri.to_string(),
            origin: origin.to_string(),
            destination: destination.to_string(),
            content,
        }
    }
}

/// Federation request signer for Matrix X-Matrix authorization headers
pub struct FederationRequestSigner {
    event_signing_engine: EventSigningEngine,
    homeserver_name: String,
}

impl FederationRequestSigner {
    pub fn new(event_signing_engine: EventSigningEngine, homeserver_name: String) -> Self {
        Self {
            event_signing_engine,
            homeserver_name,
        }
    }

    /// Extract JSON content from a reqwest::Request
    ///
    /// Extracts and parses the request body as JSON if present and content-type is application/json.
    /// Returns None for requests without bodies or non-JSON content.
    fn extract_request_content(request: &reqwest::Request) -> Result<Option<Value>, EventSigningError> {
        // Check if request has a body
        let body_bytes = match request.body() {
            Some(body) => {
                // Extract bytes from the request body
                match body.as_bytes() {
                    Some(bytes) => bytes,
                    None => return Ok(None), // Body exists but not accessible as bytes
                }
            }
            None => return Ok(None), // No body
        };

        // Check content-type header
        let headers = request.headers();
        let content_type = headers.get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        // Only parse as JSON if content-type indicates JSON
        if !content_type.starts_with("application/json") {
            return Ok(None);
        }

        // Parse body as JSON
        match serde_json::from_slice::<Value>(body_bytes) {
            Ok(json_value) => Ok(Some(json_value)),
            Err(e) => Err(EventSigningError::JsonError(e)),
        }
    }

    /// Create X-Matrix authorization header for federation requests
    ///
    /// Generates a properly formatted X-Matrix authorization header following
    /// RFC 9110 format and Matrix Server-Server API specification.
    ///
    /// # Arguments
    /// * `method` - HTTP method (GET, POST, PUT, etc.)
    /// * `uri` - Request URI including path and query parameters, starting with /_matrix/...
    /// * `destination` - Destination server name (pre-delegation)
    /// * `content` - Optional JSON request body
    ///
    /// # Returns
    /// * `Ok(String)` - Properly formatted X-Matrix authorization header
    /// * `Err(EventSigningError)` - If signing fails
    pub async fn create_authorization_header(
        &self,
        method: &str,
        uri: &str,
        destination: &str,
        content: Option<Value>,
    ) -> Result<String, EventSigningError> {
        // Create signing data structure per Matrix specification
        let signing_data = FederationRequestSigningData::new(
            method,
            uri,
            &self.homeserver_name,
            destination,
            content,
        );

        // Convert to JSON and sign using existing infrastructure
        let signing_json = serde_json::to_value(signing_data)?;
        let signed_json = self.event_signing_engine.sign_json(&signing_json, None).await?;

        // Extract signature from signed JSON
        let signatures = signed_json["signatures"]
            .as_object()
            .ok_or_else(|| EventSigningError::InvalidSignature("Missing signatures in signed JSON".to_string()))?;

        let origin_signatures = signatures.get(&self.homeserver_name)
            .and_then(|v| v.as_object())
            .ok_or_else(|| EventSigningError::InvalidSignature("Missing origin signatures".to_string()))?;

        let (key_id, signature_value) = origin_signatures
            .iter()
            .next()
            .ok_or_else(|| EventSigningError::InvalidSignature("No signatures found".to_string()))?;

        let signature = signature_value
            .as_str()
            .ok_or_else(|| EventSigningError::InvalidSignature("Signature is not a string".to_string()))?;

        // Build X-Matrix authorization header per RFC 9110
        Ok(format!(
            "X-Matrix origin=\"{}\",destination=\"{}\",key=\"{}\",sig=\"{}\"",
            self.homeserver_name, destination, key_id, signature
        ))
    }

    /// Sign a reqwest::RequestBuilder with X-Matrix authorization
    ///
    /// Complete implementation that extracts request details, parses body content,
    /// and generates Matrix-compliant X-Matrix authorization header.
    ///
    /// # Arguments
    /// * `request_builder` - The reqwest RequestBuilder to sign
    /// * `destination` - Destination server name (pre-delegation)
    ///
    /// # Returns
    /// * `Ok(RequestBuilder)` - The signed request builder with X-Matrix header
    /// * `Err(EventSigningError)` - If any step fails
    pub async fn sign_request_builder(
        &self,
        request_builder: reqwest::RequestBuilder,
        destination: &str,
    ) -> Result<reqwest::RequestBuilder, EventSigningError> {
        // Build the request to extract details
        let request = request_builder.try_clone()
            .ok_or_else(|| EventSigningError::InvalidRequest("Cannot clone request builder".to_string()))?
            .build()
            .map_err(|e| EventSigningError::HttpError(e))?;

        let method = request.method().to_string();
        let url = request.url();

        // Extract URI path + query per Matrix specification
        // Must include full path starting with /_matrix/... and query parameters
        let uri = format!("{}{}",
            url.path(),
            url.query().map(|q| format!("?{}", q)).unwrap_or_default()
        );

        // Extract request body content if present
        let content = Self::extract_request_content(&request)?;

        // Generate X-Matrix authorization header
        let auth_header = self.create_authorization_header(
            &method,
            &uri,
            destination,
            content,
        ).await?;

        // Add authorization header to original request builder
        Ok(request_builder.header("Authorization", auth_header))
    }
}

/// Verify X-Matrix authorization header for federation requests
///
/// Parses and validates the X-Matrix authorization header according to Matrix
/// Server-Server API specification with complete signature verification.
///
/// # Arguments
/// * `headers` - HTTP headers from the incoming request
/// * `homeserver_name` - Expected destination server name
/// * `method` - HTTP method of the request
/// * `uri` - Request URI path and query
/// * `content` - Optional JSON request body
/// * `event_signing_engine` - Engine for cryptographic operations
///
/// # Returns
/// * `Ok(XMatrixAuth)` - Successfully verified authentication data
/// * `Err(MatrixError)` - Matrix-compliant structured error
pub async fn verify_x_matrix_auth(
    headers: &HeaderMap,
    homeserver_name: &str,
    method: &str,
    uri: &str,
    content: Option<Value>,
    event_signing_engine: &EventSigningEngine,
) -> Result<XMatrixAuth, MatrixError> {
    // Extract Authorization header
    let auth_header = headers
        .get("authorization")
        .ok_or(MatrixError::MissingToken)?
        .to_str()
        .map_err(|_| MatrixError::Unauthorized)?;

    // Parse X-Matrix header
    let x_matrix_auth = parse_x_matrix_header(auth_header)
        .map_err(|_| MatrixError::Unauthorized)?;

    // Validate destination matches our homeserver
    if let Some(ref destination) = x_matrix_auth.destination {
        if destination != homeserver_name {
            return Err(MatrixError::Unauthorized);
        }
    }

    // Validate key ID format
    if x_matrix_auth.key_id.is_empty() {
        return Err(MatrixError::Unauthorized);
    }

    if x_matrix_auth.signature.is_empty() {
        return Err(MatrixError::InvalidSignature);
    }

    // Reconstruct the signing data exactly as it was signed
    let signing_data = FederationRequestSigningData::new(
        method,
        uri,
        &x_matrix_auth.origin,
        homeserver_name,
        content,
    );

    // Convert to canonical JSON for verification
    let signing_json = serde_json::to_value(signing_data)
        .map_err(|_| MatrixError::Unknown)?;

    let canonical_json = crate::utils::canonical_json::to_canonical_json(&signing_json)
        .map_err(|_| MatrixError::Unknown)?;

    // Fetch the remote server's public key
    let public_key = event_signing_engine
        .fetch_remote_server_key(&x_matrix_auth.origin, &x_matrix_auth.key_id)
        .await
        .map_err(|_| MatrixError::Unknown)?;

    // Verify the signature using the event signing engine
    event_signing_engine.session_service
        .verify_ed25519_signature(&x_matrix_auth.signature, &canonical_json, &public_key)
        .map_err(|_| MatrixError::InvalidSignature)?;

    Ok(x_matrix_auth)
}