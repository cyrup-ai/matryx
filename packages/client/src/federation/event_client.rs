use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use tracing::{debug, error};

use matryx_entity::types::Event;

#[derive(Debug, Error)]
pub enum FederationClientError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    
    #[error("JSON parsing failed: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("Event not found: {event_id}")]
    EventNotFound { event_id: String },
    
    #[error("Access denied to event: {event_id}")]
    AccessDenied { event_id: String },
    
    #[error("Server error: {code} - {message}")]
    ServerError { code: u16, message: String },
}

#[derive(Debug, Deserialize)]
pub struct EventResponse {
    pub origin: String,
    pub origin_server_ts: i64,
    pub pdus: Vec<Event>,
}

#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub errcode: String,
    pub error: String,
}

pub struct FederationEventClient {
    client: Client,
    server_name: String,
}

impl FederationEventClient {
    pub fn new(server_name: String) -> Self {
        Self {
            client: Client::new(),
            server_name,
        }
    }

    pub async fn get_event(
        &self,
        target_server: &str,
        event_id: &str,
    ) -> Result<Event, FederationClientError> {
        let url = format!(
            "https://{}/_matrix/federation/v1/event/{}",
            target_server, event_id
        );

        debug!("Fetching event {} from {}", event_id, target_server);

        let response = self
            .client
            .get(&url)
            .header("User-Agent", "matryx-federation-client/1.0")
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await?;

        match status.as_u16() {
            200 => {
                let event_response: EventResponse = serde_json::from_str(&body)?;
                event_response
                    .pdus
                    .into_iter()
                    .next()
                    .ok_or(FederationClientError::EventNotFound {
                        event_id: event_id.to_string(),
                    })
            }
            404 => {
                let error_response: ErrorResponse = serde_json::from_str(&body)?;
                if error_response.errcode == "M_NOT_FOUND" {
                    Err(FederationClientError::EventNotFound {
                        event_id: event_id.to_string(),
                    })
                } else {
                    Err(FederationClientError::ServerError {
                        code: status.as_u16(),
                        message: error_response.error,
                    })
                }
            }
            403 => {
                let error_response: ErrorResponse = serde_json::from_str(&body)?;
                Err(FederationClientError::AccessDenied {
                    event_id: event_id.to_string(),
                })
            }
            _ => {
                error!("Unexpected response from {}: {} - {}", target_server, status, body);
                let error_response: Result<ErrorResponse, _> = serde_json::from_str(&body);
                match error_response {
                    Ok(err) => Err(FederationClientError::ServerError {
                        code: status.as_u16(),
                        message: err.error,
                    }),
                    Err(_) => Err(FederationClientError::ServerError {
                        code: status.as_u16(),
                        message: body,
                    }),
                }
            }
        }
    }

    pub async fn test_connectivity(&self, target_server: &str) -> Result<bool, FederationClientError> {
        let url = format!("https://{}/_matrix/federation/v1/version", target_server);
        
        debug!("Testing connectivity to {}", target_server);

        match self.client.get(&url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(e) => {
                debug!("Connectivity test failed for {}: {}", target_server, e);
                Ok(false)
            }
        }
    }
}
