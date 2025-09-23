use crate::repository::error::RepositoryError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::{Connection, Surreal};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushNotification {
    pub notification_id: String,
    pub user_id: String,
    pub event_id: String,
    pub room_id: String,
    pub pusher_key: String,
    pub content: NotificationContent,
    pub created_at: DateTime<Utc>,
    pub status: NotificationStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationContent {
    pub event_type: String,
    pub sender: String,
    pub sender_display_name: Option<String>,
    pub room_name: Option<String>,
    pub body: Option<String>,
    pub image_url: Option<String>,
    pub unread_count: u64,
    pub priority: String,
    pub tweaks: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationStatus {
    Pending,
    Sent,
    Delivered,
    Failed,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationStatistics {
    pub total_notifications: u64,
    pub pending_notifications: u64,
    pub sent_notifications: u64,
    pub delivered_notifications: u64,
    pub failed_notifications: u64,
    pub delivery_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushNotificationRecord {
    pub id: String,
    pub notification_data: PushNotification,
    pub attempts: u32,
    pub last_attempt: Option<DateTime<Utc>>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
}

#[derive(Clone)]
pub struct PushNotificationRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> PushNotificationRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    pub async fn create_notification(
        &self,
        notification: &PushNotification,
    ) -> Result<String, RepositoryError> {
        let record = PushNotificationRecord {
            id: format!("push_notification:{}", notification.notification_id),
            notification_data: notification.clone(),
            attempts: 0,
            last_attempt: None,
            delivered_at: None,
            error_message: None,
        };

        let _: Option<PushNotificationRecord> = self
            .db
            .create(("push_notification", &notification.notification_id))
            .content(record)
            .await?;

        Ok(notification.notification_id.clone())
    }

    pub async fn get_notification(
        &self,
        notification_id: &str,
    ) -> Result<Option<PushNotification>, RepositoryError> {
        let record: Option<PushNotificationRecord> =
            self.db.select(("push_notification", notification_id)).await?;

        Ok(record.map(|r| r.notification_data))
    }

    pub async fn update_notification_status(
        &self,
        notification_id: &str,
        status: NotificationStatus,
    ) -> Result<(), RepositoryError> {
        // Get existing record
        let existing: Option<PushNotificationRecord> =
            self.db.select(("push_notification", notification_id)).await?;

        if let Some(mut record) = existing {
            record.notification_data.status = status;
            record.attempts += 1;
            record.last_attempt = Some(Utc::now());

            let _: Option<PushNotificationRecord> = self
                .db
                .update(("push_notification", notification_id))
                .content(record)
                .await?;

            Ok(())
        } else {
            Err(RepositoryError::NotFound {
                entity_type: "push_notification".to_string(),
                id: notification_id.to_string(),
            })
        }
    }

    pub async fn get_pending_notifications(
        &self,
        limit: Option<u32>,
    ) -> Result<Vec<PushNotification>, RepositoryError> {
        let query = if let Some(limit) = limit {
            format!(
                "SELECT * FROM push_notification WHERE notification_data.status = 'Pending' ORDER BY notification_data.created_at ASC LIMIT {}",
                limit
            )
        } else {
            "SELECT * FROM push_notification WHERE notification_data.status = 'Pending' ORDER BY notification_data.created_at ASC".to_string()
        };

        let mut result = self.db.query(&query).await?;
        let records: Vec<PushNotificationRecord> = result.take(0)?;
        Ok(records.into_iter().map(|r| r.notification_data).collect())
    }

    pub async fn get_user_notifications(
        &self,
        user_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<PushNotification>, RepositoryError> {
        let query = if let Some(limit) = limit {
            format!(
                "SELECT * FROM push_notification WHERE notification_data.user_id = $user_id ORDER BY notification_data.created_at DESC LIMIT {}",
                limit
            )
        } else {
            "SELECT * FROM push_notification WHERE notification_data.user_id = $user_id ORDER BY notification_data.created_at DESC".to_string()
        };

        let mut result = self.db.query(&query).bind(("user_id", user_id.to_string())).await?;

        let records: Vec<PushNotificationRecord> = result.take(0)?;
        Ok(records.into_iter().map(|r| r.notification_data).collect())
    }

    pub async fn mark_notification_delivered(
        &self,
        notification_id: &str,
        delivered_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        // Get existing record
        let existing: Option<PushNotificationRecord> =
            self.db.select(("push_notification", notification_id)).await?;

        if let Some(mut record) = existing {
            record.notification_data.status = NotificationStatus::Delivered;
            record.delivered_at = Some(delivered_at);

            let _: Option<PushNotificationRecord> = self
                .db
                .update(("push_notification", notification_id))
                .content(record)
                .await?;

            Ok(())
        } else {
            Err(RepositoryError::NotFound {
                entity_type: "push_notification".to_string(),
                id: notification_id.to_string(),
            })
        }
    }

    pub async fn cleanup_old_notifications(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<u64, RepositoryError> {
        let query = "DELETE FROM push_notification WHERE notification_data.created_at < $cutoff";
        let mut result = self.db.query(query).bind(("cutoff", cutoff)).await?;

        let deleted: Option<Vec<serde_json::Value>> = result.take(0).ok();
        Ok(deleted.map(|v| v.len()).unwrap_or(0) as u64)
    }

    pub async fn get_notification_statistics(
        &self,
        user_id: Option<&str>,
    ) -> Result<NotificationStatistics, RepositoryError> {
        let base_query = if let Some(user_id) = user_id {
            format!("FROM push_notification WHERE notification_data.user_id = '{}'", user_id)
        } else {
            "FROM push_notification".to_string()
        };

        // Get total notifications
        let total_query = format!("SELECT count() AS total {}", base_query);
        let mut total_result = self.db.query(&total_query).await?;

        #[derive(Deserialize)]
        struct TotalCountResult {
            total: u64,
        }

        let total_counts: Vec<TotalCountResult> = total_result.take(0)?;
        let total_notifications = total_counts.into_iter().next().map(|r| r.total).unwrap_or(0);

        // Get status counts
        let pending_query = format!(
            "SELECT count() AS pending {} AND notification_data.status = 'Pending'",
            base_query
        );
        let mut pending_result = self.db.query(&pending_query).await?;

        #[derive(Deserialize)]
        struct PendingCountResult {
            pending: u64,
        }

        let pending_counts: Vec<PendingCountResult> = pending_result.take(0)?;
        let pending_notifications =
            pending_counts.into_iter().next().map(|r| r.pending).unwrap_or(0);

        let sent_query =
            format!("SELECT count() AS sent {} AND notification_data.status = 'Sent'", base_query);
        let mut sent_result = self.db.query(&sent_query).await?;

        #[derive(Deserialize)]
        struct SentCountResult {
            sent: u64,
        }

        let sent_counts: Vec<SentCountResult> = sent_result.take(0)?;
        let sent_notifications = sent_counts.into_iter().next().map(|r| r.sent).unwrap_or(0);

        let delivered_query = format!(
            "SELECT count() AS delivered {} AND notification_data.status = 'Delivered'",
            base_query
        );
        let mut delivered_result = self.db.query(&delivered_query).await?;

        #[derive(Deserialize)]
        struct DeliveredCountResult {
            delivered: u64,
        }

        let delivered_counts: Vec<DeliveredCountResult> = delivered_result.take(0)?;
        let delivered_notifications =
            delivered_counts.into_iter().next().map(|r| r.delivered).unwrap_or(0);

        let failed_query = format!(
            "SELECT count() AS failed {} AND notification_data.status = 'Failed'",
            base_query
        );
        let mut failed_result = self.db.query(&failed_query).await?;

        #[derive(Deserialize)]
        struct FailedCountResult {
            failed: u64,
        }

        let failed_counts: Vec<FailedCountResult> = failed_result.take(0)?;
        let failed_notifications = failed_counts.into_iter().next().map(|r| r.failed).unwrap_or(0);

        let delivery_rate = if total_notifications > 0 {
            delivered_notifications as f64 / total_notifications as f64
        } else {
            0.0
        };

        Ok(NotificationStatistics {
            total_notifications,
            pending_notifications,
            sent_notifications,
            delivered_notifications,
            failed_notifications,
            delivery_rate,
        })
    }

    pub async fn get_failed_notifications(
        &self,
        limit: Option<u32>,
    ) -> Result<Vec<PushNotification>, RepositoryError> {
        let query = if let Some(limit) = limit {
            format!(
                "SELECT * FROM push_notification WHERE notification_data.status = 'Failed' ORDER BY notification_data.created_at DESC LIMIT {}",
                limit
            )
        } else {
            "SELECT * FROM push_notification WHERE notification_data.status = 'Failed' ORDER BY notification_data.created_at DESC".to_string()
        };

        let mut result = self.db.query(&query).await?;
        let records: Vec<PushNotificationRecord> = result.take(0)?;
        Ok(records.into_iter().map(|r| r.notification_data).collect())
    }

    pub async fn retry_failed_notification(
        &self,
        notification_id: &str,
    ) -> Result<(), RepositoryError> {
        self.update_notification_status(notification_id, NotificationStatus::Pending)
            .await
    }

    pub async fn mark_notification_failed(
        &self,
        notification_id: &str,
        error_message: &str,
    ) -> Result<(), RepositoryError> {
        // Get existing record
        let existing: Option<PushNotificationRecord> =
            self.db.select(("push_notification", notification_id)).await?;

        if let Some(mut record) = existing {
            record.notification_data.status = NotificationStatus::Failed;
            record.error_message = Some(error_message.to_string());
            record.attempts += 1;
            record.last_attempt = Some(Utc::now());

            let _: Option<PushNotificationRecord> = self
                .db
                .update(("push_notification", notification_id))
                .content(record)
                .await?;

            Ok(())
        } else {
            Err(RepositoryError::NotFound {
                entity_type: "push_notification".to_string(),
                id: notification_id.to_string(),
            })
        }
    }

    pub async fn get_notifications_by_event(
        &self,
        event_id: &str,
    ) -> Result<Vec<PushNotification>, RepositoryError> {
        let query = "SELECT * FROM push_notification WHERE notification_data.event_id = $event_id ORDER BY notification_data.created_at DESC";
        let mut result = self.db.query(query).bind(("event_id", event_id.to_string())).await?;

        let records: Vec<PushNotificationRecord> = result.take(0)?;
        Ok(records.into_iter().map(|r| r.notification_data).collect())
    }

    pub async fn get_notifications_by_room(
        &self,
        room_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<PushNotification>, RepositoryError> {
        let query = if let Some(limit) = limit {
            format!(
                "SELECT * FROM push_notification WHERE notification_data.room_id = $room_id ORDER BY notification_data.created_at DESC LIMIT {}",
                limit
            )
        } else {
            "SELECT * FROM push_notification WHERE notification_data.room_id = $room_id ORDER BY notification_data.created_at DESC".to_string()
        };

        let mut result = self.db.query(&query).bind(("room_id", room_id.to_string())).await?;

        let records: Vec<PushNotificationRecord> = result.take(0)?;
        Ok(records.into_iter().map(|r| r.notification_data).collect())
    }

    pub async fn get_notifications_by_pusher(
        &self,
        pusher_key: &str,
        limit: Option<u32>,
    ) -> Result<Vec<PushNotification>, RepositoryError> {
        let query = if let Some(limit) = limit {
            format!(
                "SELECT * FROM push_notification WHERE notification_data.pusher_key = $pusher_key ORDER BY notification_data.created_at DESC LIMIT {}",
                limit
            )
        } else {
            "SELECT * FROM push_notification WHERE notification_data.pusher_key = $pusher_key ORDER BY notification_data.created_at DESC".to_string()
        };

        let mut result = self.db.query(&query).bind(("pusher_key", pusher_key.to_string())).await?;

        let records: Vec<PushNotificationRecord> = result.take(0)?;
        Ok(records.into_iter().map(|r| r.notification_data).collect())
    }

    pub async fn cleanup_delivered_notifications(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<u64, RepositoryError> {
        let query = "DELETE FROM push_notification WHERE notification_data.status = 'Delivered' AND delivered_at < $cutoff";
        let mut result = self.db.query(query).bind(("cutoff", cutoff)).await?;

        let deleted: Option<Vec<serde_json::Value>> = result.take(0).ok();
        Ok(deleted.map(|v| v.len()).unwrap_or(0) as u64)
    }

    pub async fn get_notification_attempts(
        &self,
        notification_id: &str,
    ) -> Result<u32, RepositoryError> {
        let record: Option<PushNotificationRecord> =
            self.db.select(("push_notification", notification_id)).await?;

        Ok(record.map(|r| r.attempts).unwrap_or(0))
    }
}
