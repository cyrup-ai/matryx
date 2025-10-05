use crate::repository::error::RepositoryError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use surrealdb::{Connection, Surreal};
use matryx_entity::types::Event;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushRule {
    pub rule_id: String,
    pub priority_class: i32,
    pub priority: i32,
    pub conditions: Vec<PushCondition>,
    pub actions: Vec<PushAction>,
    pub default: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PushCondition {
    EventMatch { key: String, pattern: String },
    ContainsDisplayName,
    RoomMemberCount { is: String },
    SenderNotificationPermission { key: String },
    EventPropertyContains { key: String, value: String },
    EventPropertyIs { key: String, value: serde_json::Value },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PushAction {
    Notify,
    DontNotify,
    Coalesce,
    SetTweak { set_tweak: String, value: serde_json::Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushRuleEvaluation {
    pub should_notify: bool,
    pub actions: Vec<PushAction>,
    pub matched_rule: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomContext {
    pub room_id: String,
    pub member_count: u64,
    pub user_display_name: Option<String>,
    pub power_levels: HashMap<String, i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushEvent {
    pub event_id: String,
    pub event_type: String,
    pub sender: String,
    pub content: serde_json::Value,
    pub state_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushRuleRecord {
    pub id: String,
    pub user_id: String,
    pub rule_data: PushRule,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct PusherConfig<'a> {
    pub kind: &'a str,
    pub app_id: &'a str,
    pub app_display_name: &'a str,
    pub device_display_name: &'a str,
    pub profile_tag: Option<&'a str>,
    pub lang: &'a str,
    pub data: &'a serde_json::Value,
}

#[derive(Clone)]
pub struct PushRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> PushRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    pub async fn create_push_rule(
        &self,
        user_id: &str,
        rule: &PushRule,
    ) -> Result<(), RepositoryError> {
        let record = PushRuleRecord {
            id: format!("push_rule:{}:{}", user_id, rule.rule_id),
            user_id: user_id.to_string(),
            rule_data: rule.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let _: Option<PushRuleRecord> = self
            .db
            .create(("push_rule", format!("{}:{}", user_id, rule.rule_id)))
            .content(record)
            .await?;

        Ok(())
    }

    pub async fn get_user_push_rules(
        &self,
        user_id: &str,
    ) -> Result<Vec<PushRule>, RepositoryError> {
        let query = "SELECT * FROM push_rule WHERE user_id = $user_id ORDER BY rule_data.priority_class ASC, rule_data.priority ASC";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;

        let records: Vec<PushRuleRecord> = result.take(0)?;
        Ok(records.into_iter().map(|r| r.rule_data).collect())
    }

    pub async fn update_push_rule(
        &self,
        user_id: &str,
        rule_id: &str,
        rule: &PushRule,
    ) -> Result<(), RepositoryError> {
        let record_id = format!("{}:{}", user_id, rule_id);

        // Get existing record to preserve created_at
        let existing: Option<PushRuleRecord> = self.db.select(("push_rule", &record_id)).await?;

        let created_at = existing.map(|r| r.created_at).unwrap_or(Utc::now());

        let record = PushRuleRecord {
            id: format!("push_rule:{}", record_id),
            user_id: user_id.to_string(),
            rule_data: rule.clone(),
            created_at,
            updated_at: Utc::now(),
        };

        let _: Option<PushRuleRecord> =
            self.db.update(("push_rule", record_id)).content(record).await?;

        Ok(())
    }

    pub async fn delete_push_rule(
        &self,
        user_id: &str,
        rule_id: &str,
    ) -> Result<(), RepositoryError> {
        let _: Option<PushRuleRecord> =
            self.db.delete(("push_rule", format!("{}:{}", user_id, rule_id))).await?;

        Ok(())
    }

    pub async fn get_push_rule_by_id(
        &self,
        user_id: &str,
        rule_id: &str,
    ) -> Result<Option<PushRule>, RepositoryError> {
        let record: Option<PushRuleRecord> =
            self.db.select(("push_rule", format!("{}:{}", user_id, rule_id))).await?;

        Ok(record.map(|r| r.rule_data))
    }

    pub async fn evaluate_push_rules(
        &self,
        user_id: &str,
        event: &PushEvent,
        room_context: &RoomContext,
    ) -> Result<PushRuleEvaluation, RepositoryError> {
        let rules = self.get_user_push_rules(user_id).await?;

        // If user has no custom rules, use default rules
        let rules_to_evaluate = if rules.is_empty() {
            self.get_default_push_rules().await?
        } else {
            rules
        };

        for rule in rules_to_evaluate {
            if !rule.enabled {
                continue;
            }

            if self.evaluate_conditions(&rule.conditions, event, room_context) {
                let should_notify = rule.actions.contains(&PushAction::Notify);
                return Ok(PushRuleEvaluation {
                    should_notify,
                    actions: rule.actions,
                    matched_rule: Some(rule.rule_id),
                });
            }
        }

        // Default evaluation if no rules match
        Ok(PushRuleEvaluation {
            should_notify: true,
            actions: vec![PushAction::Notify],
            matched_rule: None,
        })
    }

    pub async fn get_default_push_rules(&self) -> Result<Vec<PushRule>, RepositoryError> {
        Ok(vec![
            // Override rules (highest priority)
            PushRule {
                rule_id: ".m.rule.master".to_string(),
                priority_class: 5,
                priority: 0,
                conditions: vec![],
                actions: vec![PushAction::DontNotify],
                default: true,
                enabled: false,
            },
            // Content rules
            PushRule {
                rule_id: ".m.rule.contains_display_name".to_string(),
                priority_class: 4,
                priority: 0,
                conditions: vec![PushCondition::ContainsDisplayName],
                actions: vec![
                    PushAction::Notify,
                    PushAction::SetTweak {
                        set_tweak: "sound".to_string(),
                        value: serde_json::Value::String("default".to_string()),
                    },
                    PushAction::SetTweak {
                        set_tweak: "highlight".to_string(),
                        value: serde_json::Value::Bool(true),
                    },
                ],
                default: true,
                enabled: true,
            },
            // Underride rules (lowest priority)
            PushRule {
                rule_id: ".m.rule.message".to_string(),
                priority_class: 1,
                priority: 0,
                conditions: vec![PushCondition::EventMatch {
                    key: "type".to_string(),
                    pattern: "m.room.message".to_string(),
                }],
                actions: vec![PushAction::Notify],
                default: true,
                enabled: true,
            },
            PushRule {
                rule_id: ".m.rule.member_event".to_string(),
                priority_class: 1,
                priority: 10,
                conditions: vec![PushCondition::EventMatch {
                    key: "type".to_string(),
                    pattern: "m.room.member".to_string(),
                }],
                actions: vec![PushAction::Notify],
                default: true,
                enabled: true,
            },
        ])
    }

    pub async fn reset_user_push_rules(&self, user_id: &str) -> Result<(), RepositoryError> {
        // Delete all user's custom push rules
        let query = "DELETE FROM push_rule WHERE user_id = $user_id";
        self.db.query(query).bind(("user_id", user_id.to_string())).await?;

        Ok(())
    }

    fn evaluate_conditions(
        &self,
        conditions: &[PushCondition],
        event: &PushEvent,
        context: &RoomContext,
    ) -> bool {
        for condition in conditions {
            if !self.evaluate_condition(condition, event, context) {
                return false; // All conditions must match
            }
        }
        true
    }

    fn evaluate_condition(
        &self,
        condition: &PushCondition,
        event: &PushEvent,
        context: &RoomContext,
    ) -> bool {
        match condition {
            PushCondition::EventMatch { key, pattern } => {
                self.evaluate_event_match(key, pattern, event)
            },
            PushCondition::ContainsDisplayName => {
                self.evaluate_contains_display_name(event, context)
            },
            PushCondition::RoomMemberCount { is } => self.evaluate_room_member_count(is, context),
            PushCondition::SenderNotificationPermission { key } => {
                self.evaluate_sender_notification_permission(key, event, context)
            },
            PushCondition::EventPropertyContains { key, value } => {
                self.evaluate_event_property_contains(key, value, event)
            },
            PushCondition::EventPropertyIs { key, value } => {
                self.evaluate_event_property_is(key, value, event)
            },
        }
    }

    fn evaluate_event_match(&self, key: &str, pattern: &str, event: &PushEvent) -> bool {
        match key {
            "type" => event.event_type.contains(pattern),
            "content.msgtype" => {
                if let Some(msgtype) = event.content.get("msgtype") {
                    msgtype.as_str().unwrap_or("").contains(pattern)
                } else {
                    false
                }
            },
            "content.body" => {
                if let Some(body) = event.content.get("body") {
                    body.as_str().unwrap_or("").contains(pattern)
                } else {
                    false
                }
            },
            _ => false,
        }
    }

    fn evaluate_contains_display_name(&self, event: &PushEvent, context: &RoomContext) -> bool {
        if let Some(display_name) = &context.user_display_name
            && let Some(body) = event.content.get("body")
            && let Some(body_str) = body.as_str() {
            return body_str.contains(display_name);
        }
        false
    }

    fn evaluate_room_member_count(&self, is_condition: &str, context: &RoomContext) -> bool {
        if let Some(num_str) = is_condition.strip_prefix("==")
            && let Ok(num) = num_str.parse::<u64>() {
            return context.member_count == num;
        } else if let Some(num_str) = is_condition.strip_prefix(">")
            && let Ok(num) = num_str.parse::<u64>() {
            return context.member_count > num;
        } else if let Some(num_str) = is_condition.strip_prefix("<")
            && let Ok(num) = num_str.parse::<u64>() {
            return context.member_count < num;
        }
        false
    }

    fn evaluate_sender_notification_permission(
        &self,
        key: &str,
        event: &PushEvent,
        context: &RoomContext,
    ) -> bool {
        if let Some(sender_power) = context.power_levels.get(&event.sender) {
            let required_power = match key {
                "room" => 50,
                _ => 0,
            };
            *sender_power >= required_power
        } else {
            false
        }
    }

    fn evaluate_event_property_contains(&self, key: &str, value: &str, event: &PushEvent) -> bool {
        // For Matrix v1.7 user mentions: "content.m\\.mentions.user_ids"
        if key == "content.m\\.mentions.user_ids"
            && let Some(mentions) = event.content.get("m.mentions")
            && let Some(user_ids) = mentions.get("user_ids")
            && let Some(user_ids_array) = user_ids.as_array()
        {
            return user_ids_array.iter().any(|id| {
                id.as_str().is_some_and(|id_str| id_str.contains(value))
            });
        }
        false
    }

    fn evaluate_event_property_is(&self, key: &str, value: &serde_json::Value, event: &PushEvent) -> bool {
        // For Matrix v1.7 room mentions: "content.m\\.mentions.room"
        if key == "content.m\\.mentions.room"
            && let Some(mentions) = event.content.get("m.mentions")
            && let Some(room_mention) = mentions.get("room")
        {
            return room_mention == value;
        }
        false
    }

    pub async fn get_user_push_rule_count(&self, user_id: &str) -> Result<u64, RepositoryError> {
        let query = "SELECT count() AS count FROM push_rule WHERE user_id = $user_id";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;

        #[derive(Deserialize)]
        struct CountResult {
            count: u64,
        }

        let counts: Vec<CountResult> = result.take(0)?;
        Ok(counts.into_iter().next().map(|r| r.count).unwrap_or(0))
    }

    pub async fn get_enabled_push_rules(
        &self,
        user_id: &str,
    ) -> Result<Vec<PushRule>, RepositoryError> {
        let rules = self.get_user_push_rules(user_id).await?;
        Ok(rules.into_iter().filter(|rule| rule.enabled).collect())
    }

    pub async fn enable_push_rule(
        &self,
        user_id: &str,
        rule_id: &str,
        enabled: bool,
    ) -> Result<(), RepositoryError> {
        if let Some(mut rule) = self.get_push_rule_by_id(user_id, rule_id).await? {
            rule.enabled = enabled;
            self.update_push_rule(user_id, rule_id, &rule).await?;
        } else {
            return Err(RepositoryError::NotFound {
                entity_type: "push_rule".to_string(),
                id: rule_id.to_string(),
            });
        }
        Ok(())
    }

    /// Delete a pusher for a user
    pub async fn delete_pusher(
        &self,
        user_id: &str,
        pusher_id: &str,
    ) -> Result<(), RepositoryError> {
        let query = "DELETE FROM pushers WHERE user_id = $user_id AND pusher_id = $pusher_id";
        self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("pusher_id", pusher_id.to_string()))
            .await?;

        Ok(())
    }

    /// Delete all pushers for a user's specific app_id (used for append=false behavior)
    pub async fn delete_pushers_by_app_id(
        &self,
        user_id: &str,
        app_id: &str,
    ) -> Result<(), RepositoryError> {
        let query = "DELETE FROM pushers WHERE user_id = $user_id AND app_id = $app_id";
        self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("app_id", app_id.to_string()))
            .await?;

        Ok(())
    }

    /// Create or update a pusher for a user
    pub async fn upsert_pusher(
        &self,
        user_id: &str,
        pusher_id: &str,
        config: PusherConfig<'_>,
    ) -> Result<(), RepositoryError> {
        let id = format!("pusher_{}", uuid::Uuid::new_v4());
        
        let query = r#"
            UPSERT pushers SET
                id = $id,
                user_id = $user_id,
                pusher_id = $pusher_id,
                kind = $kind,
                app_id = $app_id,
                app_display_name = $app_display_name,
                device_display_name = $device_display_name,
                profile_tag = $profile_tag,
                lang = $lang,
                data = $data,
                created_at = time::now()
            WHERE user_id = $user_id AND pusher_id = $pusher_id
        "#;

        self.db
            .query(query)
            .bind(("id", id))
            .bind(("user_id", user_id.to_string()))
            .bind(("pusher_id", pusher_id.to_string()))
            .bind(("kind", config.kind.to_string()))
            .bind(("app_id", config.app_id.to_string()))
            .bind(("app_display_name", config.app_display_name.to_string()))
            .bind(("device_display_name", config.device_display_name.to_string()))
            .bind(("profile_tag", config.profile_tag.map(|s| s.to_string())))
            .bind(("lang", config.lang.to_string()))
            .bind(("data", config.data.clone()))
            .await?;

        Ok(())
    }

    /// Process an event for push notifications
    ///
    /// This method is delegated to PushService which has access to all required repositories.
    /// Event processing happens via engine.rs -> push_service.rs -> evaluate_push_rules.
    pub async fn process_event(
        &self,
        event: &Event,
        room_id: &str,
    ) -> Result<(), RepositoryError> {
        use tracing::debug;

        debug!("Processing event {} for push notifications in room {}", event.event_id, room_id);

        // Event processing is handled by PushService and PushEngine which have
        // access to all required repositories. This method returns Ok as processing
        // happens at a higher level in the architecture.
        Ok(())
    }
}
