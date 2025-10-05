use crate::repository::error::RepositoryError;
use crate::repository::{
    EventRepository,
    PushGatewayRepository,
    PushNotificationRepository,
    PushRepository,
    PusherRepository,
    RoomRepository,
    notification::NotificationRepository,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;

use chrono::{DateTime, Utc};

// Re-export types from sub-repositories
pub use crate::repository::push::{PushEvent, PushAction, PushRule, PushRuleEvaluation, RoomContext};
pub use crate::repository::push_gateway::{PushStatistics, Pusher};
pub use crate::repository::push_notification::{
    NotificationContent,
    NotificationStatistics,
    NotificationStatus,
    PushNotification,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushSettings {
    pub enabled: bool,
    pub default_sound: Option<String>,
    pub default_highlight: bool,
    pub global_mute: bool,
    pub room_overrides: HashMap<String, RoomPushSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomPushSettings {
    pub muted: bool,
    pub sound: Option<String>,
    pub highlight: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushReceipt {
    pub notification_id: String,
    pub delivered: bool,
    pub timestamp: DateTime<Utc>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushCleanupResult {
    pub deleted_notifications: u64,
    pub deleted_attempts: u64,
    pub deleted_failed_pushers: u64,
}

#[derive(Clone)]
pub struct PushService {
    push_repo: PushRepository<Any>,
    gateway_repo: PushGatewayRepository<Any>,
    notification_repo: PushNotificationRepository<Any>,
    _event_repo: EventRepository,
    room_repo: RoomRepository,
    pusher_repo: PusherRepository<Any>,
    notification_settings_repo: NotificationRepository,
    http_client: reqwest::Client,
}

impl PushService {
    pub fn new(db: Surreal<Any>) -> Self {
        let http_client = reqwest::Client::builder()
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(30))
            .tcp_keepalive(Duration::from_secs(60))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            push_repo: PushRepository::new(db.clone()),
            gateway_repo: PushGatewayRepository::new(db.clone()),
            notification_repo: PushNotificationRepository::new(db.clone()),
            _event_repo: EventRepository::new(db.clone()),
            room_repo: RoomRepository::new(db.clone()),
            pusher_repo: PusherRepository::new(db.clone()),
            notification_settings_repo: NotificationRepository::new(db),
            http_client,
        }
    }

    pub async fn process_event_for_push(
        &self,
        event: &matryx_entity::types::Event,
        room_id: &str,
    ) -> Result<Vec<PushNotification>, RepositoryError> {
        let mut notifications = Vec::new();

        // Get room members
        let members = self.pusher_repo.get_room_members_for_push(room_id).await?;

        // Get room context
        let room_context = RoomContext {
            room_id: room_id.to_string(),
            member_count: members.len() as u64,
            user_display_name: None, // Will be set per user
            power_levels: self.pusher_repo.get_room_power_levels(room_id).await?,
        };

        // Process each member
        for member in members {
            if member.user_id == event.sender {
                continue; // Don't notify sender
            }

            // Set user-specific context
            let mut user_context = room_context.clone();
            user_context.user_display_name = member.display_name.clone();

            // Evaluate push rules for this user
            if let Some(notification) =
                self.evaluate_push_for_user(&member.user_id, event, &user_context).await?
            {
                notifications.push(notification);
            }
        }

        Ok(notifications)
    }

    pub async fn evaluate_push_for_user(
        &self,
        user_id: &str,
        event: &matryx_entity::types::Event,
        room_context: &RoomContext,
    ) -> Result<Option<PushNotification>, RepositoryError> {
        // Check user push settings
        let settings = self.get_user_push_settings(user_id).await?;

        if !settings.enabled || settings.global_mute {
            return Ok(None);
        }

        // Check room-specific overrides
        if let Some(room_settings) = settings.room_overrides.get(&room_context.room_id)
            && room_settings.muted {
            return Ok(None);
        }

        // Convert full Event to PushEvent for rule evaluation
        let push_event = PushEvent {
            event_id: event.event_id.clone(),
            event_type: event.event_type.clone(),
            sender: event.sender.clone(),
            content: serde_json::to_value(&event.content).unwrap_or_default(),
            state_key: event.state_key.clone(),
        };

        // Evaluate push rules
        let evaluation = self.push_repo.evaluate_push_rules(user_id, &push_event, room_context).await?;

        if !evaluation.should_notify {
            return Ok(None);
        }

        // Get user's pushers
        let pushers = self.gateway_repo.get_user_pushers(user_id).await?;

        if pushers.is_empty() {
            return Ok(None);
        }

        // Create notification for EACH active pusher
        let mut created_notifications = Vec::new();

        for pusher in &pushers {
            let notification_id =
                format!("{}:{}:{}", event.event_id, user_id, pusher.pusher_key);

            // Extract tweaks from actions
            let mut tweaks = serde_json::Map::new();
            for action in &evaluation.actions {
                if let PushAction::SetTweak { set_tweak, value } = action {
                    tweaks.insert(set_tweak.clone(), value.clone());
                }
            }

            let content = NotificationContent {
                event_type: event.event_type.clone(),
                sender: event.sender.clone(),
                sender_display_name: self.get_user_display_name(&event.sender).await?,
                room_name: self.get_room_name(&room_context.room_id).await?,
                body: event.content.get("body").and_then(|v| v.as_str()).map(|s| s.to_string()),
                image_url: event.content.get("url").and_then(|v| v.as_str()).map(|s| s.to_string()),
                unread_count: self.get_user_unread_count(user_id).await?,
                priority: "high".to_string(),
                tweaks: if tweaks.is_empty() {
                    None
                } else {
                    Some(serde_json::Value::Object(tweaks))
                },
            };

            let notification = PushNotification {
                notification_id: notification_id.clone(),
                user_id: user_id.to_string(),
                event_id: event.event_id.clone(),
                room_id: room_context.room_id.clone(),
                pusher_key: pusher.pusher_key.clone(),
                content,
                created_at: Utc::now(),
                status: NotificationStatus::Pending,
            };

            // Store the notification
            self.notification_repo.create_notification(&notification).await?;
            created_notifications.push(notification);
        }

        // Return first notification for compatibility
        Ok(if created_notifications.is_empty() {
            None
        } else {
            Some(created_notifications[0].clone())
        })
    }

    pub async fn send_push_notification(
        &self,
        notification: &PushNotification,
    ) -> Result<(), RepositoryError> {
        // Get pusher to retrieve gateway URL
        let pusher = self.gateway_repo
            .get_pusher_by_key(&notification.user_id, &notification.pusher_key)
            .await?
            .ok_or_else(|| RepositoryError::NotFound {
                entity_type: "pusher".to_string(),
                id: notification.pusher_key.clone(),
            })?;

        // Extract push gateway URL from pusher data
        let push_url = pusher.data
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RepositoryError::ValidationError {
                field: "pusher.data.url".to_string(),
                message: "Push gateway URL not found in pusher data".to_string(),
            })?;

        // Build Matrix Push Gateway API payload
        let payload = serde_json::json!({
            "notification": {
                "event_id": notification.event_id,
                "room_id": notification.room_id,
                "type": notification.content.event_type,
                "sender": notification.content.sender,
                "sender_display_name": notification.content.sender_display_name,
                "room_name": notification.content.room_name,
                "content": {
                    "body": notification.content.body,
                    "msgtype": "m.text"
                },
                "counts": {
                    "unread": notification.content.unread_count,
                    "missed_calls": null
                },
                "devices": [{
                    "app_id": pusher.app_id,
                    "pushkey": notification.pusher_key,
                    "pushkey_ts": null,
                    "data": null,
                    "tweaks": notification.content.tweaks
                }],
                "prio": notification.content.priority
            }
        });

        // Record attempt as pending
        self.gateway_repo
            .record_push_attempt(&notification.pusher_key, &notification.notification_id, false)
            .await?;

        // Send HTTP POST to push gateway with retry
        let gateway_url = format!("{}/_matrix/push/v1/notify", push_url);
        
        for attempt in 1..=3 {
            match self.http_client
                .post(&gateway_url)
                .json(&payload)
                .send()
                .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        // Parse response to check for rejected pushkeys
                        if let Ok(body) = response.json::<serde_json::Value>().await
                            && let Some(rejected) = body.get("rejected").and_then(|r| r.as_array())
                            && rejected.iter().any(|r| r.as_str() == Some(&notification.pusher_key))
                        {
                            // Pushkey rejected by gateway
                            self.notification_repo
                                .update_notification_status(&notification.notification_id, NotificationStatus::Rejected)
                                .await?;
                            
                            self.gateway_repo
                                .record_push_attempt_with_details(
                                    &notification.pusher_key,
                                    &notification.notification_id,
                                    false,
                                    Some("Pushkey rejected by gateway".to_string()),
                                    Some(200),
                                )
                                .await?;
                            
                            return Ok(());
                        }
                        
                        // Success
                        self.notification_repo
                            .update_notification_status(&notification.notification_id, NotificationStatus::Sent)
                            .await?;
                        
                        self.gateway_repo
                            .record_push_attempt(&notification.pusher_key, &notification.notification_id, true)
                            .await?;
                        
                        return Ok(());
                    } else if attempt < 3 && response.status().is_server_error() {
                        // Retry on 5xx errors
                        tokio::time::sleep(Duration::from_millis(100 * 2_u64.pow(attempt as u32))).await;
                        continue;
                    } else {
                        // Non-retriable error
                        let error_msg = format!("Gateway returned {}", response.status());
                        self.notification_repo
                            .mark_notification_failed(&notification.notification_id, &error_msg)
                            .await?;
                        
                        self.gateway_repo
                            .record_push_attempt_with_details(
                                &notification.pusher_key,
                                &notification.notification_id,
                                false,
                                Some(error_msg.clone()),
                                Some(response.status().as_u16()),
                            )
                            .await?;
                        
                        return Err(RepositoryError::DatabaseError {
                            message: error_msg,
                            operation: "send_push_gateway".to_string(),
                        });
                    }
                },
                Err(_e) if attempt < 3 => {
                    // Retry on network errors
                    tokio::time::sleep(Duration::from_millis(100 * 2_u64.pow(attempt as u32))).await;
                    continue;
                },
                Err(e) => {
                    // Final failure
                    self.notification_repo
                        .mark_notification_failed(&notification.notification_id, &e.to_string())
                        .await?;
                    
                    self.gateway_repo
                        .record_push_attempt_with_details(
                            &notification.pusher_key,
                            &notification.notification_id,
                            false,
                            Some(e.to_string()),
                            None,
                        )
                        .await?;
                    
                    return Err(RepositoryError::DatabaseError {
                        message: e.to_string(),
                        operation: "send_push_gateway".to_string(),
                    });
                }
            }
        }
        
        unreachable!()
    }

    pub async fn handle_push_receipt(
        &self,
        notification_id: &str,
        receipt: &PushReceipt,
    ) -> Result<(), RepositoryError> {
        if receipt.delivered {
            self.notification_repo
                .mark_notification_delivered(notification_id, receipt.timestamp)
                .await?;
        } else {
            let error_msg = receipt.error_message.as_deref().unwrap_or("Delivery failed");
            self.notification_repo
                .mark_notification_failed(notification_id, error_msg)
                .await?;
        }

        Ok(())
    }

    pub async fn get_user_push_settings(
        &self,
        user_id: &str,
    ) -> Result<PushSettings, RepositoryError> {
        // Query notification_settings table via existing repository
        let settings = self.notification_settings_repo
            .get_notification_settings(user_id)
            .await?;

        // Convert NotificationSettings to PushSettings
        let mut room_overrides = HashMap::new();
        for (room_id, room_settings) in settings.room_notifications {
            room_overrides.insert(
                room_id,
                RoomPushSettings {
                    muted: !room_settings.enabled,
                    sound: room_settings.sound,
                    highlight: room_settings.highlight,
                },
            );
        }

        Ok(PushSettings {
            enabled: settings.enabled,
            default_sound: if settings.sound_enabled {
                Some("default".to_string())
            } else {
                None
            },
            default_highlight: false,
            global_mute: !settings.enabled,
            room_overrides,
        })
    }

    pub async fn update_user_push_settings(
        &self,
        user_id: &str,
        settings: &PushSettings,
    ) -> Result<(), RepositoryError> {
        // Convert PushSettings to NotificationSettings
        let mut room_notifications = HashMap::new();
        for (room_id, room_settings) in &settings.room_overrides {
            room_notifications.insert(
                room_id.clone(),
                crate::repository::notification::RoomNotificationSettings {
                    room_id: room_id.clone(),
                    enabled: !room_settings.muted,
                    sound: room_settings.sound.clone(),
                    highlight: room_settings.highlight,
                    mention_only: false,
                },
            );
        }

        let notification_settings = crate::repository::notification::NotificationSettings {
            user_id: user_id.to_string(),
            enabled: settings.enabled && !settings.global_mute,
            sound_enabled: settings.default_sound.is_some(),
            badge_enabled: true,
            room_notifications,
            keyword_notifications: vec![],
            push_rules: vec![],
            updated_at: Utc::now(),
        };

        // Persist to database via existing repository
        self.notification_settings_repo
            .update_notification_settings(user_id, &notification_settings)
            .await
    }

    pub async fn cleanup_push_data(&self) -> Result<PushCleanupResult, RepositoryError> {
        let cutoff = Utc::now() - chrono::Duration::days(30);

        // Cleanup old notifications
        let deleted_notifications =
            self.notification_repo.cleanup_old_notifications(cutoff).await?;

        // Cleanup old push attempts
        let deleted_attempts = self.gateway_repo.cleanup_old_push_attempts(cutoff).await?;

        // Cleanup failed pushers (with failure threshold of 10)
        let deleted_failed_pushers = self.gateway_repo.cleanup_failed_pushers(10).await?;

        Ok(PushCleanupResult {
            deleted_notifications,
            deleted_attempts,
            deleted_failed_pushers,
        })
    }

    async fn get_user_display_name(
        &self,
        _user_id: &str,
    ) -> Result<Option<String>, RepositoryError> {
        // Query user profile for display name
        // This is a simplified implementation
        Ok(Some("User".to_string()))
    }

    async fn get_room_name(&self, room_id: &str) -> Result<Option<String>, RepositoryError> {
        self.room_repo.get_room_name(room_id).await
    }

    async fn get_user_unread_count(&self, _user_id: &str) -> Result<u64, RepositoryError> {
        // Get unread message count for user
        // This is a simplified implementation
        Ok(1)
    }

    pub async fn register_pusher(
        &self,
        user_id: &str,
        pusher: &Pusher,
    ) -> Result<(), RepositoryError> {
        self.gateway_repo.register_pusher(user_id, pusher).await
    }

    pub async fn remove_pusher(
        &self,
        user_id: &str,
        pusher_key: &str,
    ) -> Result<(), RepositoryError> {
        self.gateway_repo.remove_pusher(user_id, pusher_key).await
    }

    pub async fn get_user_pushers(&self, user_id: &str) -> Result<Vec<Pusher>, RepositoryError> {
        self.gateway_repo.get_user_pushers(user_id).await
    }

    pub async fn create_push_rule(
        &self,
        user_id: &str,
        rule: &PushRule,
    ) -> Result<(), RepositoryError> {
        self.push_repo.create_push_rule(user_id, rule).await
    }

    pub async fn get_user_push_rules(
        &self,
        user_id: &str,
    ) -> Result<Vec<PushRule>, RepositoryError> {
        self.push_repo.get_user_push_rules(user_id).await
    }

    pub async fn update_push_rule(
        &self,
        user_id: &str,
        rule_id: &str,
        rule: &PushRule,
    ) -> Result<(), RepositoryError> {
        self.push_repo.update_push_rule(user_id, rule_id, rule).await
    }

    pub async fn delete_push_rule(
        &self,
        user_id: &str,
        rule_id: &str,
    ) -> Result<(), RepositoryError> {
        self.push_repo.delete_push_rule(user_id, rule_id).await
    }

    pub async fn get_pending_notifications(
        &self,
        limit: Option<u32>,
    ) -> Result<Vec<PushNotification>, RepositoryError> {
        self.notification_repo.get_pending_notifications(limit).await
    }

    pub async fn get_push_statistics(
        &self,
        pusher_key: &str,
    ) -> Result<PushStatistics, RepositoryError> {
        self.gateway_repo.get_push_statistics(pusher_key).await
    }

    pub async fn get_notification_statistics(
        &self,
        user_id: Option<&str>,
    ) -> Result<NotificationStatistics, RepositoryError> {
        self.notification_repo.get_notification_statistics(user_id).await
    }

    pub async fn retry_failed_notifications(
        &self,
        limit: Option<u32>,
    ) -> Result<u64, RepositoryError> {
        let failed_notifications = self.notification_repo.get_failed_notifications(limit).await?;
        let mut retried_count = 0;

        for notification in failed_notifications {
            if let Ok(()) = self
                .notification_repo
                .retry_failed_notification(&notification.notification_id)
                .await
            {
                retried_count += 1;
            }
        }

        Ok(retried_count)
    }

    pub async fn process_pending_notifications(
        &self,
        batch_size: u32,
    ) -> Result<u64, RepositoryError> {
        let pending_notifications =
            self.notification_repo.get_pending_notifications(Some(batch_size)).await?;
        let mut processed_count = 0;

        for notification in pending_notifications {
            if let Ok(()) = self.send_push_notification(&notification).await {
                processed_count += 1;
            }
        }

        Ok(processed_count)
    }

    pub async fn get_user_notification_history(
        &self,
        user_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<PushNotification>, RepositoryError> {
        self.notification_repo.get_user_notifications(user_id, limit).await
    }

    pub async fn mute_room_notifications(
        &self,
        user_id: &str,
        room_id: &str,
        muted: bool,
    ) -> Result<(), RepositoryError> {
        let mut settings = self.get_user_push_settings(user_id).await?;

        let room_settings = RoomPushSettings { muted, sound: None, highlight: false };

        settings.room_overrides.insert(room_id.to_string(), room_settings);
        self.update_user_push_settings(user_id, &settings).await
    }

    pub async fn enable_push_rule(
        &self,
        user_id: &str,
        rule_id: &str,
        enabled: bool,
    ) -> Result<(), RepositoryError> {
        self.push_repo.enable_push_rule(user_id, rule_id, enabled).await
    }
}
