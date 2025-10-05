//! Module contains intentional library code not yet fully integrated
#![allow(dead_code)]

//! Federation client for Matrix server-to-server communication
//!
//! Handles HTTP requests to remote Matrix servers for federation queries
//! including user membership validation for restricted room authorization.

use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::federation::event_signer::EventSigner;
use matryx_entity::types::{Transaction, TransactionResponse};

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
            .query(&[("event_type", "m.room.member"), ("state_key", user_id)])
            .timeout(self.request_timeout);

        // Sign the federation request with X-Matrix authentication
        // Include our homeserver name in the signing process for proper federation authorization
        let uri = format!(
            "/_matrix/federation/v1/make_join/{}/{}?event_type=m.room.member&state_key={}",
            urlencoding::encode(room_id),
            urlencoding::encode(user_id),
            urlencoding::encode(user_id)
        );
        let signed_request = self
            .event_signer
            .sign_federation_request(request_builder, "GET", &uri, server_name, None)
            .await
            .map_err(|_e| FederationClientError::InvalidResponse)?;

        // Execute the HTTP request
        let response = signed_request.send().await?;

        // Handle HTTP status codes
        if !response.status().is_success() {
            warn!(
                "Federation membership query failed: {} - {}",
                response.status(),
                response.status().canonical_reason().unwrap_or("Unknown error")
            );

            return Err(FederationClientError::ServerError {
                status_code: response.status().as_u16(),
                message: response
                    .status()
                    .canonical_reason()
                    .unwrap_or("Unknown error")
                    .to_string(),
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

    /// Send a federation transaction to a remote homeserver
    ///
    /// Implements Matrix spec: PUT /_matrix/federation/v1/send/{txnId}
    /// See: /Volumes/samsung_t9/maxtryx/spec/server/05-transactions.md
    pub async fn send_transaction(
        &self,
        destination: &str,
        txn_id: &str,
        transaction: &Transaction,
    ) -> Result<TransactionResponse, FederationClientError> {
        debug!(
            destination = %destination,
            txn_id = %txn_id,
            pdu_count = transaction.pdus.len(),
            edu_count = transaction.edus.len(),
            "Sending federation transaction"
        );

        // Construct federation API URL
        let url = format!(
            "https://{}/_matrix/federation/v1/send/{}",
            destination,
            urlencoding::encode(txn_id)
        );

        // Serialize transaction to JSON
        let transaction_json =
            serde_json::to_value(transaction).map_err(FederationClientError::JsonError)?;

        // Create HTTP PUT request
        let request_builder =
            self.http_client.put(&url).json(&transaction).timeout(self.request_timeout);

        // Sign request with X-Matrix authentication
        let uri = format!("/_matrix/federation/v1/send/{}", urlencoding::encode(txn_id));
        let signed_request = self
            .event_signer
            .sign_federation_request(
                request_builder,
                "PUT",
                &uri,
                destination,
                Some(transaction_json),
            )
            .await
            .map_err(|_| FederationClientError::InvalidResponse)?;

        // Execute HTTP request
        let response = signed_request.send().await?;

        // Handle HTTP errors
        if !response.status().is_success() {
            warn!(
                destination = %destination,
                txn_id = %txn_id,
                status = %response.status(),
                "Transaction send failed"
            );
            return Err(FederationClientError::ServerError {
                status_code: response.status().as_u16(),
                message: response.status().canonical_reason().unwrap_or("Unknown").to_string(),
            });
        }

        // Parse response
        let transaction_response: TransactionResponse = response.json().await?;

        info!(
            destination = %destination,
            txn_id = %txn_id,
            "Transaction sent successfully"
        );

        Ok(transaction_response)
    }

    /// Query user devices from a remote homeserver
    ///
    /// Implements Matrix spec: GET /_matrix/federation/v1/user/devices/{userId}
    pub async fn query_user_devices(
        &self,
        server_name: &str,
        user_id: &str,
    ) -> Result<DevicesResponse, FederationClientError> {
        debug!(
            "Querying devices for user {} on server {}",
            user_id, server_name
        );

        // Prevent federation requests to ourselves
        if server_name == self.homeserver_name {
            return Err(FederationClientError::InvalidResponse);
        }

        // Construct federation API URL
        let url = format!(
            "https://{}/_matrix/federation/v1/user/devices/{}",
            server_name,
            urlencoding::encode(user_id)
        );

        // Create HTTP GET request
        let request_builder = self.http_client.get(&url).timeout(self.request_timeout);

        // Sign request with X-Matrix authentication
        let uri = format!(
            "/_matrix/federation/v1/user/devices/{}",
            urlencoding::encode(user_id)
        );
        let signed_request = self
            .event_signer
            .sign_federation_request(request_builder, "GET", &uri, server_name, None)
            .await
            .map_err(|_| FederationClientError::InvalidResponse)?;

        // Execute HTTP request
        let response = signed_request.send().await?;

        // Handle HTTP errors
        if !response.status().is_success() {
            warn!(
                "Federation devices query failed: {} - {}",
                response.status(),
                response.status().canonical_reason().unwrap_or("Unknown error")
            );
            return Err(FederationClientError::ServerError {
                status_code: response.status().as_u16(),
                message: response.status().canonical_reason().unwrap_or("Unknown").to_string(),
            });
        }

        // Parse response
        let devices_response: DevicesResponse = response.json().await?;

        info!(
            "Successfully queried {} devices for user {} from server {}",
            devices_response.devices.len(),
            user_id,
            server_name
        );

        Ok(devices_response)
    }
}

/// Response structure for user devices queries
#[derive(Debug, Deserialize, Serialize)]
pub struct DevicesResponse {
    pub user_id: String,
    pub stream_id: i64,
    pub devices: Vec<matryx_entity::types::Device>,
    #[serde(default)]
    pub signatures: std::collections::HashMap<String, std::collections::HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unsigned: Option<serde_json::Value>,
}
