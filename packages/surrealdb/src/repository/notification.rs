use base64::{Engine as _, engine::general_purpose};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use surrealdb::{Surreal, engine::any::Any};

use crate::repository::RepositoryError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationType {
    Message,
    Invite,
    Mention,
    Reaction,
    Call,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub notification_id: String,
    pub user_id: String,
    pub event_id: String,
    pub room_id: String,
    pub notification_type: NotificationType,
    pub content: Value,
    pub created_at: DateTime<Utc>,
    pub read: bool,
    pub delivered: bool,
    pub actions: Vec<NotificationAction>,
    pub priority: NotificationPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationAction {
    pub action_id: String,
    pub action_type: String,
    pub label: String,
    pub url: Option<String>,
    pub method: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationPriority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationResponse {
    pub notifications: Vec<Notification>,
    pub next_token: Option<String>,
    pub prev_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSettings {
    pub user_id: String,
    pub enabled: bool,
    pub sound_enabled: bool,
    pub badge_enabled: bool,
    pub room_notifications: HashMap<String, RoomNotificationSettings>,
    pub keyword_notifications: Vec<String>,
    pub push_rules: Vec<PushRule>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomNotificationSettings {
    pub room_id: String,
    pub enabled: bool,
    pub sound: Option<String>,
    pub highlight: bool,
    pub mention_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushRule {
    pub rule_id: String,
    pub priority_class: i32,
    pub conditions: Vec<PushCondition>,
    pub actions: Vec<PushAction>,
    pub enabled: bool,
    pub default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushCondition {
    pub kind: String,
    pub key: Option<String>,
    pub pattern: Option<String>,
    pub is: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushAction {
    pub action_type: String,
    pub set_tweak: Option<String>,
    pub value: Option<Value>,
}

#[derive(Clone)]
pub struct NotificationRepository {
    db: Surreal<Any>,
}

impl NotificationRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    pub async fn create_notification(
        &self,
        user_id: &str,
        event_id: &str,
        room_id: &str,
        notification_type: NotificationType,
    ) -> Result<String, RepositoryError> {
        let notification_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();

        let create_query = r#"
            CREATE notifications CONTENT {
                notification_id: $notification_id,
                user_id: $user_id,
                event_id: $event_id,
                room_id: $room_id,
                notification_type: $notification_type,
                content: $content,
                created_at: $created_at,
                read: false,
                delivered: false,
                actions: $actions,
                priority: $priority
            }
        "#;

        // Get event content for notification
        let event_content = self.get_event_content(event_id).await?;

        let actions = self.get_default_actions_for_type(&notification_type);
        let priority = self.get_priority_for_type(&notification_type);

        self.db
            .query(create_query)
            .bind(("notification_id", notification_id.clone()))
            .bind(("user_id", user_id.to_string()))
            .bind(("event_id", event_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .bind(("notification_type", notification_type.clone()))
            .bind(("content", event_content))
            .bind(("created_at", now))
            .bind(("actions", actions))
            .bind(("priority", priority))
            .await
            .map_err(|e| {
                RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "create_notification".to_string(),
                }
            })?;

        Ok(notification_id)
    }

    pub async fn get_user_notifications(
        &self,
        user_id: &str,
        from: Option<&str>,
        limit: Option<u32>,
        only: Option<&str>,
    ) -> Result<NotificationResponse, RepositoryError> {
        let mut query = String::from("SELECT * FROM notifications WHERE user_id = $user_id");

        let mut db_query = self.db.query(&query).bind(("user_id", user_id.to_string()));

        if let Some(from_token) = from {
            query.push_str(" AND created_at < $from_time");
            // Parse from token to get timestamp
            if let Ok(from_time) = self.parse_notification_token(from_token) {
                db_query = db_query.bind(("from_time", from_time));
            }
        }

        // Filter by notification type if 'only' parameter is provided
        match only {
            Some("highlight") => {
                query.push_str(" AND highlight = true");
            },
            Some(unknown_filter) => {
                // Log unknown filter types for debugging but ignore them per Matrix spec
                // Unknown conditions MUST NOT match any events (effectively disabling the filter)
                tracing::warn!(
                    "Unknown notification filter type '{}' ignored for user {} (forward compatibility)",
                    unknown_filter,
                    user_id
                );
                // Don't add any filter - unknown filters are ignored per spec
            },
            None => {
                // No filter specified
            },
        }

        query.push_str(" ORDER BY created_at DESC");

        let limit_value = limit.unwrap_or(50);
        query.push_str(" LIMIT $limit");
        db_query = db_query.bind(("limit", limit_value));

        let mut response = db_query.await.map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_user_notifications".to_string(),
            }
        })?;

        let notifications_data: Vec<Value> = response.take(0).map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_user_notifications_parse".to_string(),
            }
        })?;

        let notifications: Result<Vec<Notification>, _> =
            notifications_data.into_iter().map(serde_json::from_value).collect();

        let notifications = notifications.map_err(|e| {
            RepositoryError::SerializationError {
                message: format!("Failed to deserialize notifications: {}", e),
            }
        })?;

        // Generate pagination tokens
        let next_token = if notifications.len() == limit_value as usize {
            notifications.last().map(|n| self.create_notification_token(&n.created_at))
        } else {
            None
        };

        let prev_token = notifications
            .first()
            .map(|n| self.create_notification_token(&n.created_at));

        Ok(NotificationResponse { notifications, next_token, prev_token })
    }

    pub async fn mark_notification_read(
        &self,
        user_id: &str,
        notification_id: &str,
    ) -> Result<(), RepositoryError> {
        let update_query = r#"
            UPDATE notifications 
            SET read = true, read_at = time::now()
            WHERE notification_id = $notification_id AND user_id = $user_id
        "#;

        self.db
            .query(update_query)
            .bind(("notification_id", notification_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await
            .map_err(|e| {
                RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "mark_notification_read".to_string(),
                }
            })?;

        Ok(())
    }

    pub async fn mark_all_notifications_read(
        &self,
        user_id: &str,
        room_id: Option<&str>,
    ) -> Result<u64, RepositoryError> {
        let mut query = String::from(
            "UPDATE notifications SET read = true, read_at = time::now() WHERE user_id = $user_id",
        );

        let mut db_query = self.db.query(&query).bind(("user_id", user_id.to_string()));

        if let Some(room_id) = room_id {
            query.push_str(" AND room_id = $room_id");
            db_query = db_query.bind(("room_id", room_id.to_string()));
        }

        db_query.await.map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "mark_all_notifications_read".to_string(),
            }
        })?;

        // Note: SurrealDB doesn't return count of updated records by default
        // This is a simplified implementation
        Ok(0)
    }

    pub async fn get_notification_count(
        &self,
        user_id: &str,
        room_id: Option<&str>,
    ) -> Result<u64, RepositoryError> {
        let mut query = String::from(
            "SELECT count() FROM notifications WHERE user_id = $user_id AND read = false",
        );

        let mut db_query = self.db.query(&query).bind(("user_id", user_id.to_string()));

        if let Some(room_id) = room_id {
            query.push_str(" AND room_id = $room_id");
            db_query = db_query.bind(("room_id", room_id.to_string()));
        }

        let mut response = db_query.await.map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_notification_count".to_string(),
            }
        })?;

        let count: Vec<(u64,)> = response.take(0).map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_notification_count_parse".to_string(),
            }
        })?;

        Ok(count.first().map(|(c,)| *c).unwrap_or(0))
    }

    pub async fn delete_notification(
        &self,
        user_id: &str,
        notification_id: &str,
    ) -> Result<(), RepositoryError> {
        let delete_query = r#"
            DELETE notifications 
            WHERE notification_id = $notification_id AND user_id = $user_id
        "#;

        self.db
            .query(delete_query)
            .bind(("notification_id", notification_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await
            .map_err(|e| {
                RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "delete_notification".to_string(),
                }
            })?;

        Ok(())
    }

    pub async fn get_notification_settings(
        &self,
        user_id: &str,
    ) -> Result<NotificationSettings, RepositoryError> {
        let settings_query = "SELECT * FROM notification_settings WHERE user_id = $user_id";

        let mut response = self
            .db
            .query(settings_query)
            .bind(("user_id", user_id.to_string()))
            .await
            .map_err(|e| {
                RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "get_notification_settings".to_string(),
                }
            })?;

        let settings_data: Vec<Value> = response.take(0).map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_notification_settings_parse".to_string(),
            }
        })?;

        if let Some(settings_value) = settings_data.into_iter().next() {
            serde_json::from_value(settings_value).map_err(|e| {
                RepositoryError::SerializationError {
                    message: format!("Failed to deserialize notification settings: {}", e),
                }
            })
        } else {
            // Return default settings if none exist
            Ok(NotificationSettings {
                user_id: user_id.to_string(),
                enabled: true,
                sound_enabled: true,
                badge_enabled: true,
                room_notifications: HashMap::new(),
                keyword_notifications: vec![],
                push_rules: vec![],
                updated_at: Utc::now(),
            })
        }
    }

    pub async fn update_notification_settings(
        &self,
        user_id: &str,
        settings: &NotificationSettings,
    ) -> Result<(), RepositoryError> {
        let upsert_query = r#"
            UPSERT notification_settings:$user_id CONTENT $settings
        "#;

        self.db
            .query(upsert_query)
            .bind(("user_id", user_id.to_string()))
            .bind(("settings", settings.clone()))
            .await
            .map_err(|e| {
                RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "update_notification_settings".to_string(),
                }
            })?;

        Ok(())
    }

    pub async fn cleanup_old_notifications(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<u64, RepositoryError> {
        let cleanup_query = "DELETE notifications WHERE created_at < $cutoff AND read = true";

        self.db.query(cleanup_query).bind(("cutoff", cutoff)).await.map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "cleanup_old_notifications".to_string(),
            }
        })?;

        // Note: SurrealDB doesn't return count of deleted records by default
        // This is a simplified implementation
        Ok(0)
    }

    // Helper methods

    async fn get_event_content(&self, event_id: &str) -> Result<Value, RepositoryError> {
        let event_query = "SELECT content FROM event WHERE event_id = $event_id";

        let mut response = self
            .db
            .query(event_query)
            .bind(("event_id", event_id.to_string()))
            .await
            .map_err(|e| {
                RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "get_event_content".to_string(),
                }
            })?;

        let events: Vec<(Value,)> = response.take(0).map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_event_content_parse".to_string(),
            }
        })?;

        Ok(events
            .into_iter()
            .next()
            .map(|(content,)| content)
            .unwrap_or_else(|| serde_json::Value::Null))
    }

    fn get_default_actions_for_type(
        &self,
        notification_type: &NotificationType,
    ) -> Vec<NotificationAction> {
        match notification_type {
            NotificationType::Message => {
                vec![
                    NotificationAction {
                        action_id: "view".to_string(),
                        action_type: "view".to_string(),
                        label: "View".to_string(),
                        url: None,
                        method: Some("GET".to_string()),
                    },
                    NotificationAction {
                        action_id: "reply".to_string(),
                        action_type: "reply".to_string(),
                        label: "Reply".to_string(),
                        url: None,
                        method: Some("POST".to_string()),
                    },
                ]
            },
            NotificationType::Invite => {
                vec![
                    NotificationAction {
                        action_id: "accept".to_string(),
                        action_type: "accept".to_string(),
                        label: "Accept".to_string(),
                        url: None,
                        method: Some("POST".to_string()),
                    },
                    NotificationAction {
                        action_id: "decline".to_string(),
                        action_type: "decline".to_string(),
                        label: "Decline".to_string(),
                        url: None,
                        method: Some("POST".to_string()),
                    },
                ]
            },
            _ => vec![],
        }
    }

    fn get_priority_for_type(&self, notification_type: &NotificationType) -> NotificationPriority {
        match notification_type {
            NotificationType::Call => NotificationPriority::Critical,
            NotificationType::Mention => NotificationPriority::High,
            NotificationType::Invite => NotificationPriority::High,
            NotificationType::Message => NotificationPriority::Normal,
            NotificationType::Reaction => NotificationPriority::Low,
            NotificationType::Custom(_) => NotificationPriority::Normal,
        }
    }

    fn create_notification_token(&self, timestamp: &DateTime<Utc>) -> String {
        // Simple token format: base64 encoded timestamp
        general_purpose::STANDARD.encode(timestamp.timestamp().to_string())
    }

    fn parse_notification_token(&self, token: &str) -> Result<DateTime<Utc>, RepositoryError> {
        let decoded = general_purpose::STANDARD.decode(token).map_err(|e| {
            RepositoryError::ValidationError {
                field: "notification_token".to_string(),
                message: format!("Invalid token format: {}", e),
            }
        })?;

        let timestamp_str = String::from_utf8(decoded).map_err(|e| {
            RepositoryError::ValidationError {
                field: "notification_token".to_string(),
                message: format!("Invalid token encoding: {}", e),
            }
        })?;

        let timestamp: i64 = timestamp_str.parse().map_err(|e| {
            RepositoryError::ValidationError {
                field: "notification_token".to_string(),
                message: format!("Invalid timestamp in token: {}", e),
            }
        })?;

        DateTime::from_timestamp(timestamp, 0).ok_or_else(|| {
            RepositoryError::ValidationError {
                field: "notification_token".to_string(),
                message: "Invalid timestamp value".to_string(),
            }
        })
    }
}
