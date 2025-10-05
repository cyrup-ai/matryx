//! Module contains intentional library code not yet fully integrated
#![allow(dead_code)]

use matryx_surrealdb::repository::PushGatewayRepository;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use surrealdb::{Connection, Surreal};
use tracing::{error, info, warn};

#[derive(Debug, Serialize)]
pub struct PushNotification {
    pub notification: NotificationData,
}

#[derive(Debug, Serialize)]
pub struct NotificationData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<serde_json::Value>,
    pub counts: NotificationCounts,
    pub devices: Vec<PushDeviceInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    pub prio: String, // "high" | "low"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender_display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "type")]
    pub type_: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_is_target: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct NotificationCounts {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unread: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missed_calls: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct PushDeviceInfo {
    pub app_id: String,
    pub pushkey: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pushkey_ts: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tweaks: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct PushResponse {
    pub rejected: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum PushError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("Gateway returned error status: {0}")]
    GatewayError(reqwest::StatusCode),
    #[error("Invalid gateway URL: {0}")]
    InvalidUrl(String),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("Database error: {0}")]
    DatabaseError(#[from] surrealdb::Error),
    #[error("Repository error: {0}")]
    RepositoryError(#[from] matryx_surrealdb::repository::RepositoryError),
    #[error("Timeout error")]
    Timeout,
}

pub struct PushGateway {
    client: Client,
    gateway_url: String,
}

pub struct PushGatewayWithRepository<C: Connection> {
    gateway: PushGateway,
    repository: PushGatewayRepository<C>,
}

impl PushGateway {
    pub fn new(gateway_url: String) -> Result<Self, PushError> {
        // Create client with connection pooling for standalone usage
        let client = Client::builder()
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(30))
            .tcp_keepalive(Duration::from_secs(60))
            .build()
            .map_err(PushError::HttpError)?;

        Self::with_client(gateway_url, client)
    }

    pub fn with_client(gateway_url: String, client: Client) -> Result<Self, PushError> {
        // Validate URL format
        if !gateway_url.starts_with("http://") && !gateway_url.starts_with("https://") {
            return Err(PushError::InvalidUrl(format!(
                "URL must start with http:// or https://, got: {}",
                gateway_url
            )));
        }

        Ok(Self {
            client, // Reuse shared client with connection pooling
            gateway_url,
        })
    }

    pub async fn send_notification(
        &self,
        notification: PushNotification,
    ) -> Result<PushResponse, PushError> {
        let url = format!("{}/_matrix/push/v1/notify", self.gateway_url);

        info!("Sending push notification to gateway: {}", url);

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&notification)
            .send()
            .await
            .map_err(PushError::HttpError)?;

        let status = response.status();

        if status.is_success() {
            let push_response: PushResponse =
                response.json().await.map_err(PushError::HttpError)?;

            if !push_response.rejected.is_empty() {
                warn!("Push gateway rejected some pushkeys: {:?}", push_response.rejected);
            }

            info!("Push notification sent successfully");
            Ok(push_response)
        } else {
            error!("Push gateway returned error status: {}", status);

            // Try to get error details from response body
            if let Ok(error_body) = response.text().await {
                error!("Push gateway error details: {}", error_body);
            }

            Err(PushError::GatewayError(status))
        }
    }

    pub async fn send_notification_with_retry(
        &self,
        notification: PushNotification,
        max_retries: u32,
    ) -> Result<PushResponse, PushError> {
        let mut last_error = None;

        for attempt in 0..=max_retries {
            match self.send_notification(notification.clone()).await {
                Ok(response) => return Ok(response),
                Err(error) => {
                    last_error = Some(error);

                    if attempt < max_retries {
                        let delay = Duration::from_millis(1000 * (2_u64.pow(attempt)));
                        warn!(
                            "Push notification attempt {} failed, retrying in {:?}",
                            attempt + 1,
                            delay
                        );
                        tokio::time::sleep(delay).await;
                    }
                },
            }
        }

        Err(last_error.unwrap_or(PushError::Timeout))
    }
}

impl<C: Connection> PushGatewayWithRepository<C> {
    pub fn new(gateway_url: String, db: Surreal<C>) -> Result<Self, PushError> {
        let gateway = PushGateway::new(gateway_url)?;
        let repository = PushGatewayRepository::new(db);

        Ok(Self { gateway, repository })
    }

    pub fn with_client(
        gateway_url: String,
        client: Client,
        db: Surreal<C>,
    ) -> Result<Self, PushError> {
        let gateway = PushGateway::with_client(gateway_url, client)?;
        let repository = PushGatewayRepository::new(db);

        Ok(Self { gateway, repository })
    }

    /// Send notification with automatic attempt recording
    pub async fn send_notification_with_tracking(
        &self,
        notification: PushNotification,
        pusher_key: &str,
        notification_id: &str,
    ) -> Result<PushResponse, PushError> {
        match self.gateway.send_notification(notification).await {
            Ok(response) => {
                // Record successful attempt
                self.repository
                    .record_push_attempt(pusher_key, notification_id, true)
                    .await
                    .map_err(PushError::RepositoryError)?;
                Ok(response)
            },
            Err(error) => {
                // Record failed attempt with details
                let error_message = Some(error.to_string());
                let response_code = match &error {
                    PushError::GatewayError(status) => Some(status.as_u16()),
                    _ => None,
                };

                self.repository
                    .record_push_attempt_with_details(
                        pusher_key,
                        notification_id,
                        false,
                        error_message,
                        response_code,
                    )
                    .await
                    .map_err(PushError::RepositoryError)?;

                Err(error)
            },
        }
    }

    /// Send notification with retry and automatic tracking
    pub async fn send_notification_with_retry_and_tracking(
        &self,
        notification: PushNotification,
        pusher_key: &str,
        notification_id: &str,
        max_retries: u32,
    ) -> Result<PushResponse, PushError> {
        let mut last_error = None;

        for attempt in 0..=max_retries {
            match self
                .send_notification_with_tracking(notification.clone(), pusher_key, notification_id)
                .await
            {
                Ok(response) => return Ok(response),
                Err(error) => {
                    last_error = Some(error);

                    if attempt < max_retries {
                        let delay = Duration::from_millis(1000 * (2_u64.pow(attempt)));
                        warn!(
                            "Push notification attempt {} failed, retrying in {:?}",
                            attempt + 1,
                            delay
                        );
                        tokio::time::sleep(delay).await;
                    }
                },
            }
        }

        Err(last_error.unwrap_or(PushError::Timeout))
    }

    /// Register a pusher using the repository
    pub async fn register_pusher(
        &self,
        user_id: &str,
        pusher: &matryx_surrealdb::repository::push_gateway::Pusher,
    ) -> Result<(), PushError> {
        self.repository
            .register_pusher(user_id, pusher)
            .await
            .map_err(PushError::RepositoryError)
    }

    /// Remove a pusher using the repository
    pub async fn remove_pusher(&self, user_id: &str, pusher_key: &str) -> Result<(), PushError> {
        self.repository
            .remove_pusher(user_id, pusher_key)
            .await
            .map_err(PushError::RepositoryError)
    }

    /// Get user's pushers using the repository
    pub async fn get_user_pushers(
        &self,
        user_id: &str,
    ) -> Result<Vec<matryx_surrealdb::repository::push_gateway::Pusher>, PushError> {
        self.repository
            .get_user_pushers(user_id)
            .await
            .map_err(PushError::RepositoryError)
    }

    /// Get push statistics using the repository
    pub async fn get_push_statistics(
        &self,
        pusher_key: &str,
    ) -> Result<matryx_surrealdb::repository::push_gateway::PushStatistics, PushError> {
        self.repository
            .get_push_statistics(pusher_key)
            .await
            .map_err(PushError::RepositoryError)
    }

    /// Cleanup failed pushers using the repository
    pub async fn cleanup_failed_pushers(&self, failure_threshold: u32) -> Result<u64, PushError> {
        self.repository
            .cleanup_failed_pushers(failure_threshold)
            .await
            .map_err(PushError::RepositoryError)
    }

    /// Get active pushers using the repository
    pub async fn get_active_pushers(
        &self,
        days: u32,
    ) -> Result<Vec<matryx_surrealdb::repository::push_gateway::Pusher>, PushError> {
        self.repository
            .get_active_pushers(days)
            .await
            .map_err(PushError::RepositoryError)
    }

    /// Get failed pushers using the repository
    pub async fn get_failed_pushers(
        &self,
        failure_threshold: u32,
    ) -> Result<Vec<matryx_surrealdb::repository::push_gateway::Pusher>, PushError> {
        self.repository
            .get_failed_pushers(failure_threshold)
            .await
            .map_err(PushError::RepositoryError)
    }
}

impl Clone for PushNotification {
    fn clone(&self) -> Self {
        Self {
            notification: NotificationData {
                content: self.notification.content.clone(),
                counts: NotificationCounts {
                    unread: self.notification.counts.unread,
                    missed_calls: self.notification.counts.missed_calls,
                },
                devices: self
                    .notification
                    .devices
                    .iter()
                    .map(|d| PushDeviceInfo {
                        app_id: d.app_id.clone(),
                        pushkey: d.pushkey.clone(),
                        pushkey_ts: d.pushkey_ts,
                        data: d.data.clone(),
                        tweaks: d.tweaks.clone(),
                    })
                    .collect(),
                event_id: self.notification.event_id.clone(),
                prio: self.notification.prio.clone(),
                room_id: self.notification.room_id.clone(),
                room_name: self.notification.room_name.clone(),
                sender: self.notification.sender.clone(),
                sender_display_name: self.notification.sender_display_name.clone(),
                type_: self.notification.type_.clone(),
                user_is_target: self.notification.user_is_target,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_gateway_url_validation() {
        // Valid URLs
        assert!(PushGateway::new("https://push.example.com".to_string()).is_ok());
        assert!(PushGateway::new("http://localhost:8080".to_string()).is_ok());

        // Invalid URLs
        assert!(PushGateway::new("ftp://example.com".to_string()).is_err());
        assert!(PushGateway::new("example.com".to_string()).is_err());
    }

    #[test]
    fn test_notification_serialization() {
        let notification = PushNotification {
            notification: NotificationData {
                content: Some(serde_json::json!({
                    "msgtype": "m.text",
                    "body": "Hello world"
                })),
                counts: NotificationCounts { unread: Some(5), missed_calls: None },
                devices: vec![PushDeviceInfo {
                    app_id: "com.example.app".to_string(),
                    pushkey: "test_pushkey".to_string(),
                    pushkey_ts: Some(1234567890),
                    data: None,
                    tweaks: Some(serde_json::json!({"sound": "default"})),
                }],
                event_id: Some("$event123".to_string()),
                prio: "high".to_string(),
                room_id: Some("!room123:example.com".to_string()),
                room_name: Some("Test Room".to_string()),
                sender: Some("@user:example.com".to_string()),
                sender_display_name: Some("Test User".to_string()),
                type_: Some("m.room.message".to_string()),
                user_is_target: Some(false),
            },
        };

        let json = serde_json::to_string(&notification).unwrap();
        assert!(json.contains("\"prio\":\"high\""));
        assert!(json.contains("\"unread\":5"));
    }
}
