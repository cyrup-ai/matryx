//! Generic endpoint utilities for Matrix client
//!
//! This module provides utilities for endpoint discovery and metadata.
//! Currently serves as a placeholder for future endpoint-related functionality.

use crate::http_client::MatrixHttpClient;

/// Client for generic endpoint operations
#[derive(Clone)]
pub struct EndpointClient {
    /// HTTP client reserved for future endpoint discovery methods
    _http_client: MatrixHttpClient,
}

impl EndpointClient {
    pub fn new(http_client: MatrixHttpClient) -> Self {
        Self { _http_client: http_client }
    }
    
    // Future endpoint discovery methods will be added here
    // as the Matrix specification evolves
}
