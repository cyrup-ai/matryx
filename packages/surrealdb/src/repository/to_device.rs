use crate::repository::error::RepositoryError;
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use surrealdb::{Surreal, engine::any::Any};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToDeviceMessage {
    pub message_id: String,
    pub sender_id: String,
    pub recipient_id: String,
    pub device_id: String,
    #[serde(rename = "message_type")]
    pub event_type: String,
    pub content: Value,
    pub txn_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub is_delivered: bool,
}

#[derive(Clone)]
pub struct ToDeviceRepository {
    db: Surreal<Any>,
}

impl ToDeviceRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    /// Send to-device messages to multiple users and devices
    pub async fn send_to_device(
        &self,
        sender_id: &str,
        event_type: &str,
        messages: &HashMap<String, HashMap<String, Value>>,
    ) -> Result<(), RepositoryError> {
        // Validate event type
        if event_type.is_empty() {
            return Err(RepositoryError::Validation {
                field: "event_type".to_string(),
                message: "Event type cannot be empty".to_string(),
            });
        }

        // Validate that sender exists and is not deactivated
        let sender_check_query = "
            SELECT user_id FROM user_profiles
            WHERE user_id = $sender_id AND deactivated_at IS NULL
            LIMIT 1
        ";

        let mut result = self
            .db
            .query(sender_check_query)
            .bind(("sender_id", sender_id.to_string()))
            .await?;

        let sender_exists: Vec<Value> = result.take(0)?;
        if sender_exists.is_empty() {
            return Err(RepositoryError::NotFound {
                entity_type: "User".to_string(),
                id: sender_id.to_string(),
            });
        }

        let created_at = Utc::now();

        // Process messages for each user
        for (user_id, device_messages) in messages {
            // Validate recipient exists
            let recipient_check_query = "
                SELECT user_id FROM user_profiles
                WHERE user_id = $user_id AND deactivated_at IS NULL
                LIMIT 1
            ";

            let mut result = self
                .db
                .query(recipient_check_query)
                .bind(("user_id", user_id.to_string()))
                .await?;

            let recipient_exists: Vec<Value> = result.take(0)?;
            if recipient_exists.is_empty() {
                continue; // Skip invalid recipients
            }

            // Send message to each device
            for (device_id, content) in device_messages {
                // Skip if device_id is "*" and no devices are found
                let target_devices = if device_id == "*" {
                    self.get_user_device_ids(user_id).await?
                } else {
                    // Verify specific device exists
                    if self.device_exists(user_id, device_id).await? {
                        vec![device_id.clone()]
                    } else {
                        continue; // Skip invalid devices
                    }
                };

                // Send to each target device
                for target_device_id in target_devices {
                    let message_id = format!("todevice_{}", Uuid::new_v4());

                    let insert_query = "
                        CREATE to_device_messages SET
                            message_id = $message_id,
                            sender_id = $sender_id,
                            recipient_id = $recipient_id,
                            device_id = $device_id,
                            event_type = $event_type,
                            content = $content,
                            is_delivered = false,
                            created_at = $created_at
                    ";

                    self.db
                        .query(insert_query)
                        .bind(("message_id", message_id))
                        .bind(("sender_id", sender_id.to_string()))
                        .bind(("recipient_id", user_id.to_string()))
                        .bind(("device_id", target_device_id))
                        .bind(("event_type", event_type.to_string()))
                        .bind(("content", content.clone()))
                        .bind(("created_at", created_at.to_rfc3339()))
                        .await?;
                }
            }
        }

        Ok(())
    }

    /// Get to-device messages for a user and device since a given token
    pub async fn get_to_device_messages(
        &self,
        user_id: &str,
        device_id: &str,
        since: Option<&str>,
    ) -> Result<Vec<ToDeviceMessage>, RepositoryError> {
        let mut query = "
            SELECT * FROM to_device_messages
            WHERE recipient_id = $user_id AND device_id = $device_id
            AND is_delivered = false
        "
        .to_string();

        let mut query_builder = self
            .db
            .query(&query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()));

        // Add since filter if provided
        if let Some(since_token) = since {
            // Parse since token as timestamp or message ID
            if let Ok(since_timestamp) = since_token.parse::<DateTime<Utc>>() {
                query.push_str(" AND created_at > $since_timestamp");
                query_builder =
                    query_builder.bind(("since_timestamp", since_timestamp.to_rfc3339()));
            } else {
                // Treat as message ID
                query.push_str(" AND message_id > $since_message_id");
                query_builder = query_builder.bind(("since_message_id", since_token.to_string()));
            }
        }

        query.push_str(" ORDER BY created_at ASC LIMIT 100"); // Cap at 100 messages

        let mut result = query_builder.await?;
        let messages_data: Vec<Value> = result.take(0)?;

        let mut messages = Vec::new();
        for message_data in messages_data {
            if let Some(message) = self.value_to_to_device_message(message_data)? {
                messages.push(message);
            }
        }

        Ok(messages)
    }

    /// Mark to-device messages as delivered
    pub async fn mark_to_device_messages_delivered(
        &self,
        user_id: &str,
        device_id: &str,
        message_ids: &[String],
    ) -> Result<(), RepositoryError> {
        if message_ids.is_empty() {
            return Ok(());
        }

        let delivered_at = Utc::now();

        // Update messages in batches to avoid query size limits
        for chunk in message_ids.chunks(50) {
            let update_query = format!(
                "UPDATE to_device_messages SET
                    is_delivered = true,
                    delivered_at = $delivered_at
                WHERE recipient_id = $user_id 
                AND device_id = $device_id 
                AND message_id IN [{}]",
                chunk
                    .iter()
                    .map(|id| format!("'{}'", id.replace('\'', "''")))
                    .collect::<Vec<_>>()
                    .join(", ")
            );

            self.db
                .query(&update_query)
                .bind(("user_id", user_id.to_string()))
                .bind(("device_id", device_id.to_string()))
                .bind(("delivered_at", delivered_at.to_rfc3339()))
                .await?;
        }

        Ok(())
    }

    /// Clean up delivered messages older than the cutoff time
    pub async fn cleanup_delivered_messages(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<u64, RepositoryError> {
        let delete_query = "
            DELETE FROM to_device_messages
            WHERE is_delivered = true AND delivered_at < $cutoff
        ";

        let mut result = self.db.query(delete_query).bind(("cutoff", cutoff.to_rfc3339())).await?;

        let deleted: Vec<Value> = result.take(0)?;
        Ok(deleted.len() as u64)
    }

    /// Validate permissions for sending to-device messages
    pub async fn validate_to_device_permissions(
        &self,
        sender_id: &str,
        recipient_id: &str,
    ) -> Result<bool, RepositoryError> {
        // Check if sender exists and is not deactivated
        let sender_query = "
            SELECT user_id FROM user_profiles
            WHERE user_id = $sender_id AND deactivated_at IS NULL
            LIMIT 1
        ";

        let mut result = self
            .db
            .query(sender_query)
            .bind(("sender_id", sender_id.to_string()))
            .await?;

        let sender_exists: Vec<Value> = result.take(0)?;
        if sender_exists.is_empty() {
            return Ok(false);
        }

        // Check if recipient exists and is not deactivated
        let recipient_query = "
            SELECT user_id FROM user_profiles
            WHERE user_id = $recipient_id AND deactivated_at IS NULL
            LIMIT 1
        ";

        let mut result = self
            .db
            .query(recipient_query)
            .bind(("recipient_id", recipient_id.to_string()))
            .await?;

        let recipient_exists: Vec<Value> = result.take(0)?;
        if recipient_exists.is_empty() {
            return Ok(false);
        }

        // Check if recipient has blocked the sender
        let block_query = "
            SELECT id FROM user_blocks
            WHERE blocker_id = $recipient_id AND blocked_id = $sender_id
            LIMIT 1
        ";

        let mut result = self
            .db
            .query(block_query)
            .bind(("recipient_id", recipient_id.to_string()))
            .bind(("sender_id", sender_id.to_string()))
            .await?;

        let blocks: Vec<Value> = result.take(0)?;
        if !blocks.is_empty() {
            return Ok(false);
        }

        // Additional validation: check if users share any rooms (for privacy)
        let shared_rooms_query = "
            SELECT COUNT(*) as count FROM (
                SELECT DISTINCT m1.room_id FROM membership m1
                JOIN membership m2 ON m1.room_id = m2.room_id
                WHERE m1.user_id = $sender_id AND m1.membership = 'join'
                AND m2.user_id = $recipient_id AND m2.membership = 'join'
            ) GROUP ALL
        ";

        let mut result = self
            .db
            .query(shared_rooms_query)
            .bind(("sender_id", sender_id.to_string()))
            .bind(("recipient_id", recipient_id.to_string()))
            .await?;

        let counts: Vec<Value> = result.take(0)?;
        let shared_rooms_count = counts
            .first()
            .and_then(|v| v.get("count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        // Allow if users share at least one room
        Ok(shared_rooms_count > 0)
    }

    /// Get device IDs for a user
    async fn get_user_device_ids(&self, user_id: &str) -> Result<Vec<String>, RepositoryError> {
        let query = "
            SELECT device_id FROM device_keys
            WHERE user_id = $user_id
        ";

        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;

        let devices_data: Vec<Value> = result.take(0)?;

        let mut device_ids = Vec::new();
        for device_data in devices_data {
            if let Some(device_id) = device_data.get("device_id").and_then(|v| v.as_str()) {
                device_ids.push(device_id.to_string());
            }
        }

        Ok(device_ids)
    }

    /// Check if a specific device exists for a user
    async fn device_exists(&self, user_id: &str, device_id: &str) -> Result<bool, RepositoryError> {
        let query = "
            SELECT device_id FROM device_keys
            WHERE user_id = $user_id AND device_id = $device_id
            LIMIT 1
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .await?;

        let devices: Vec<Value> = result.take(0)?;
        Ok(!devices.is_empty())
    }

    /// Subscribe to to-device messages using SurrealDB LIVE query
    /// Returns a stream of notifications for to-device messages for the specified user
    pub async fn subscribe_to_device_messages(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<
        impl futures_util::Stream<Item = Result<ToDeviceMessage, RepositoryError>>,
        RepositoryError,
    > {
        // Create SurrealDB LiveQuery for to-device messages
        let mut stream = self
            .db
            .query("LIVE SELECT * FROM to_device_messages WHERE recipient_id = $user_id AND device_id = $device_id AND is_delivered = false")
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .await
            .map_err(RepositoryError::Database)?;

        // Transform SurrealDB notification stream with proper error handling
        let message_stream = stream
            .stream::<surrealdb::Notification<serde_json::Value>>(0)
            .map_err(RepositoryError::Database)?
            .map(|notification_result| -> Result<ToDeviceMessage, RepositoryError> {
                let notification = notification_result.map_err(RepositoryError::Database)?;

                match notification.action {
                    surrealdb::Action::Create | surrealdb::Action::Update => {
                        // Convert to ToDeviceMessage or return error
                        Self::convert_notification_to_message_result(notification.data)
                    },
                    surrealdb::Action::Delete => {
                        // Return the deleted message data for proper handling
                        Self::convert_notification_to_message_result(notification.data)
                    },
                    _ => {
                        // Handle unexpected actions with error
                        Err(RepositoryError::Database(surrealdb::Error::msg(format!(
                            "Unexpected action in to-device message notification: {:?}",
                            notification.action
                        ))))
                    },
                }
            });

        Ok(message_stream)
    }

    /// Convert notification data to ToDeviceMessage with proper error handling
    fn convert_notification_to_message_result(value: Value) -> Result<ToDeviceMessage, RepositoryError> {
        let message_id = value.get("message_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RepositoryError::Validation {
                field: "message_id".to_string(),
                message: "Missing or invalid message_id".to_string(),
            })?;
        
        let sender_id = value.get("sender_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RepositoryError::Validation {
                field: "sender_id".to_string(),
                message: "Missing or invalid sender_id".to_string(),
            })?;
        
        let recipient_id = value.get("recipient_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RepositoryError::Validation {
                field: "recipient_id".to_string(),
                message: "Missing or invalid recipient_id".to_string(),
            })?;
        
        let device_id = value.get("device_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RepositoryError::Validation {
                field: "device_id".to_string(),
                message: "Missing or invalid device_id".to_string(),
            })?;
        
        let event_type = value.get("event_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RepositoryError::Validation {
                field: "event_type".to_string(),
                message: "Missing or invalid event_type".to_string(),
            })?;
        
        let content = value.get("content")
            .ok_or_else(|| RepositoryError::Validation {
                field: "content".to_string(),
                message: "Missing content field".to_string(),
            })?;
        
        let created_at_str = value.get("created_at")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RepositoryError::Validation {
                field: "created_at".to_string(),
                message: "Missing or invalid created_at".to_string(),
            })?;
        
        let is_delivered = value.get("is_delivered")
            .and_then(|v| v.as_bool())
            .ok_or_else(|| RepositoryError::Validation {
                field: "is_delivered".to_string(),
                message: "Missing or invalid is_delivered".to_string(),
            })?;

        let created_at = created_at_str.parse::<DateTime<Utc>>()
            .map_err(|e| RepositoryError::Validation {
                field: "created_at".to_string(),
                message: format!("Invalid datetime format: {}", e),
            })?;

        let delivered_at = if is_delivered {
            value
                .get("delivered_at")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<DateTime<Utc>>().ok())
        } else {
            None
        };

        let txn_id = value.get("txn_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(ToDeviceMessage {
            message_id: message_id.to_string(),
            sender_id: sender_id.to_string(),
            recipient_id: recipient_id.to_string(),
            device_id: device_id.to_string(),
            event_type: event_type.to_string(),
            content: content.clone(),
            txn_id,
            created_at,
            delivered_at,
            is_delivered,
        })
    }

    /// Convert database value to ToDeviceMessage
    fn value_to_to_device_message(
        &self,
        value: Value,
    ) -> Result<Option<ToDeviceMessage>, RepositoryError> {
        if let (
            Some(message_id),
            Some(sender_id),
            Some(recipient_id),
            Some(device_id),
            Some(event_type),
            Some(content),
            Some(created_at_str),
            Some(is_delivered),
        ) = (
            value.get("message_id").and_then(|v| v.as_str()),
            value.get("sender_id").and_then(|v| v.as_str()),
            value.get("recipient_id").and_then(|v| v.as_str()),
            value.get("device_id").and_then(|v| v.as_str()),
            value.get("event_type").and_then(|v| v.as_str()),
            value.get("content"),
            value.get("created_at").and_then(|v| v.as_str()),
            value.get("is_delivered").and_then(|v| v.as_bool()),
        ) {
            let created_at = created_at_str.parse::<DateTime<Utc>>().unwrap_or_else(|_| Utc::now());

            let delivered_at = if is_delivered {
                value
                    .get("delivered_at")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<DateTime<Utc>>().ok())
            } else {
                None
            };

            let txn_id = value.get("txn_id").and_then(|v| v.as_str()).map(|s| s.to_string());

            Ok(Some(ToDeviceMessage {
                message_id: message_id.to_string(),
                sender_id: sender_id.to_string(),
                recipient_id: recipient_id.to_string(),
                device_id: device_id.to_string(),
                event_type: event_type.to_string(),
                content: content.clone(),
                txn_id,
                created_at,
                delivered_at,
                is_delivered,
            }))
        } else {
            Ok(None)
        }
    }
}
