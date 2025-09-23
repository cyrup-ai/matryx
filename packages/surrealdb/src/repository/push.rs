use crate::repository::error::RepositoryError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use surrealdb::{Connection, Surreal};

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
pub struct Event {
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
        event: &Event,
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
        event: &Event,
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
        event: &Event,
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
        }
    }

    fn evaluate_event_match(&self, key: &str, pattern: &str, event: &Event) -> bool {
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

    fn evaluate_contains_display_name(&self, event: &Event, context: &RoomContext) -> bool {
        if let Some(display_name) = &context.user_display_name {
            if let Some(body) = event.content.get("body") {
                if let Some(body_str) = body.as_str() {
                    return body_str.contains(display_name);
                }
            }
        }
        false
    }

    fn evaluate_room_member_count(&self, is_condition: &str, context: &RoomContext) -> bool {
        if let Some(num_str) = is_condition.strip_prefix("==") {
            if let Ok(num) = num_str.parse::<u64>() {
                return context.member_count == num;
            }
        } else if let Some(num_str) = is_condition.strip_prefix(">") {
            if let Ok(num) = num_str.parse::<u64>() {
                return context.member_count > num;
            }
        } else if let Some(num_str) = is_condition.strip_prefix("<") {
            if let Ok(num) = num_str.parse::<u64>() {
                return context.member_count < num;
            }
        }
        false
    }

    fn evaluate_sender_notification_permission(
        &self,
        key: &str,
        event: &Event,
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
}
