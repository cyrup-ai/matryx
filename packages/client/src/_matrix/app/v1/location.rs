//! Application service location endpoints
//!
//! Room-based location queries for application services
//! Reference: packages/server/src/_matrix/app/v1/location.rs

use crate::http_client::{HttpClientError, MatrixHttpClient};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Response from GET /_matrix/app/v1/location/{roomId}
#[derive(Debug, Deserialize)]
pub struct LocationResponse {
    /// Location data (nullable)
    pub location: Option<Value>,
}

/// Request for PUT /_matrix/app/v1/location/{roomId}
#[derive(Debug, Serialize)]
pub struct LocationUpdate {
    /// Location data to set
    #[serde(flatten)]
    pub data: Value,
}

/// Empty response for PUT operations
#[derive(Debug, Deserialize)]
pub struct EmptyResponse {}

/// Client for application service location operations
#[derive(Clone)]
pub struct AppLocationClient {
    http_client: MatrixHttpClient,
}

impl AppLocationClient {
    pub fn new(http_client: MatrixHttpClient) -> Self {
        Self { http_client }
    }

    /// Get location for a room
    ///
    /// GET /_matrix/app/v1/location/{roomId}
    ///
    /// # Arguments
    /// * `room_id` - The Matrix room ID
    pub async fn get_location(&self, room_id: &str) -> Result<LocationResponse, HttpClientError> {
        let path = format!("/_matrix/app/v1/location/{}", room_id);
        self.http_client
            .request(Method::GET, &path, None::<&()>)
            .await
    }

    /// Update location for a room
    ///
    /// PUT /_matrix/app/v1/location/{roomId}
    ///
    /// # Arguments
    /// * `room_id` - The Matrix room ID
    /// * `location` - Location data to set
    pub async fn set_location(
        &self,
        room_id: &str,
        location: Value,
    ) -> Result<EmptyResponse, HttpClientError> {
        let path = format!("/_matrix/app/v1/location/{}", room_id);
        let body = LocationUpdate { data: location };
        self.http_client
            .request(Method::PUT, &path, Some(&body))
            .await
    }
}
