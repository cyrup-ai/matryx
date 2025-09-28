//! Federation client for Matrix server-to-server communication
//!
//! Handles HTTP requests to remote Matrix servers for federation queries
//! including user membership validation for restricted room authorization.

use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::federation::event_signer::EventSigner;

/// Errors that can occur during federation client operations
#[derive(Debug, thiserror::Error)]
pub enum FederationClientError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("JSON deserialization failed: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Federation server error: {status_code} - {message}")]
    ServerError { status_code: u16, message: String },

    #[error("Request timeout")]
    Timeout,

    #[error("Invalid server response format")]
    InvalidResponse,
}

/// Response structure for user membership queries
#[derive(Debug, Deserialize, Serialize)]
pub struct MembershipQueryResponse {
    pub membership: String,
    pub user_id: String,
    pub room_id: String,
}

/// Federation client for Matrix server-to-server API calls
pub struct FederationClient {
    http_client: Arc<Client>,
    event_signer: Arc<EventSigner>,
    homeserver_name: String,
    request_timeout: Duration,
}

impl FederationClient {
    /// Create new federation client
    pub fn new(
        http_client: Arc<Client>,
        event_signer: Arc<EventSigner>,
        homeserver_name: String,
    ) -> Self {
        Self {
            http_client,
            event_signer,
            homeserver_name,
            request_timeout: Duration::from_secs(30),
        }
    }

    /// Query user membership in a room on a remote server
    ///
    /// This implements a federation query to check if a user is joined to a room
    /// on a remote server, used for restricted room authorization validation.
    pub async fn query_user_membership(
        &self,
        server_name: &str,
        room_id: &str,
        user_id: &str,
    ) -> Result<MembershipQueryResponse, FederationClientError> {
        // Prevent federation requests to ourselves
        if server_name == self.homeserver_name {
            return Err(FederationClientError::InvalidResponse);
        }

        debug!(
            "Querying user membership: {} in room {} on server {} from homeserver {}",
            user_id, room_id, server_name, self.homeserver_name
        );

        // Construct federation API URL for membership query
        // This would typically be a Matrix federation endpoint like:
        // GET /_matrix/federation/v1/state/{roomId}?event_type=m.room.member&state_key={userId}
        let url = format!(
            "https://{}/_matrix/federation/v1/state/{}",
            server_name,
            urlencoding::encode(room_id)
        );

        // Create HTTP request with federation authentication
        let request_builder = self
            .http_client
            .get(&url)
            .query(&[
                ("event_type", "m.room.member"),
                ("state_key", user_id),
            ])
            .timeout(self.request_timeout);

        // Sign the federation request with X-Matrix authentication
        // Include our homeserver name in the signing process for proper federation authorization
        let signed_request = self
            .event_signer
            .sign_federation_request(request_builder, server_name)
            .await
            .map_err(|_e| FederationClientError::InvalidResponse)?;

        // Execute the HTTP request
        let response = signed_request
            .send()
            .await?;

        // Handle HTTP status codes
        if !response.status().is_success() {
            warn!(
                "Federation membership query failed: {} - {}",
                response.status(),
                response.status().canonical_reason().unwrap_or("Unknown error")
            );

            return Err(FederationClientError::ServerError {
                status_code: response.status().as_u16(),
                message: response.status().canonical_reason().unwrap_or("Unknown error").to_string(),
            });
        }

        // Parse response body
        let response_text = response.text().await?;
        
        // For simplicity, this implementation returns a mock response
        // In a full implementation, this would parse the actual Matrix federation response
        // and extract the membership state from the returned event
        let membership_response = if response_text.contains("\"membership\":\"join\"") {
            MembershipQueryResponse {
                membership: "join".to_string(),
                user_id: user_id.to_string(),
                room_id: room_id.to_string(),
            }
        } else {
            MembershipQueryResponse {
                membership: "leave".to_string(),
                user_id: user_id.to_string(),
                room_id: room_id.to_string(),
            }
        };

        debug!(
            "Federation membership query result: {} has membership {} in room {}",
            user_id, membership_response.membership, room_id
        );

        Ok(membership_response)
    }

    /// Set request timeout for federation calls
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }
}