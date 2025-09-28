use matryx_entity::PDU;
use matryx_surrealdb::repository::push::{
    PushEvent,
    PushAction,
    PushCondition,
    PushRule,
    PushRuleEvaluation,
    RoomContext,
};
use matryx_surrealdb::repository::{PushRepository, RepositoryError};


use surrealdb::engine::any::Any;
use surrealdb::Surreal;

// Types are now imported from the repository module

pub struct PushRuleEngine {
    push_repo: PushRepository<Any>,
    default_rules: Vec<PushRule>,
}

impl PushRuleEngine {
    pub fn new(db: Surreal<Any>) -> Self {
        let push_repo = PushRepository::new(db);
        let default_rules = Self::get_default_rules();

        Self { push_repo, default_rules }
    }

    pub fn with_rules(db: Surreal<Any>, rules: Vec<PushRule>) -> Self {
        let push_repo = PushRepository::new(db);

        Self { push_repo, default_rules: rules }
    }

    pub fn evaluate_event(&self, event: &PDU, context: &RoomContext) -> Vec<PushAction> {
        // Use default rules for evaluation (this is synchronous)
        let mut actions = Vec::new();

        // Sort rules by priority class and priority
        let mut sorted_rules = self.default_rules.clone();
        sorted_rules.sort_by(|a, b| {
            a.priority_class.cmp(&b.priority_class).then(a.priority.cmp(&b.priority))
        });

        for rule in sorted_rules {
            if !rule.enabled {
                continue;
            }

            if self.evaluate_conditions(&rule.conditions, event, context) {
                actions.extend(rule.actions.clone());
                break; // First matching rule wins
            }
        }

        if actions.is_empty() {
            // Default action if no rules match
            actions.push(PushAction::Notify);
        }

        actions
    }

    /// Evaluate push rules for a user using the repository (async version)
    pub async fn evaluate_event_for_user(
        &self,
        user_id: &str,
        event: &PDU,
        context: &RoomContext,
    ) -> Result<PushRuleEvaluation, RepositoryError> {
        // Convert PDU to Event
        let push_event = PushEvent {
            event_id: event.event_id.clone(),
            event_type: event.event_type.clone(),
            sender: event.sender.clone(),
            content: serde_json::to_value(&event.content)
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
            state_key: event.state_key.clone(),
        };

        self.push_repo.evaluate_push_rules(user_id, &push_event, context).await
    }

    fn evaluate_conditions(
        &self,
        conditions: &[PushCondition],
        event: &PDU,
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
        event: &PDU,
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

    fn evaluate_event_match(&self, key: &str, pattern: &str, event: &PDU) -> bool {
        // Simple pattern matching - in production this would use glob patterns
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

    fn evaluate_contains_display_name(&self, event: &PDU, context: &RoomContext) -> bool {
        if let Some(display_name) = &context.user_display_name
            && let Some(body) = event.content.get("body")
            && let Some(body_str) = body.as_str() {
                return body_str.contains(display_name);
            }
        false
    }

    fn evaluate_room_member_count(&self, is_condition: &str, context: &RoomContext) -> bool {
        // Parse condition like "==2" or ">10"
        if let Some(num_str) = is_condition.strip_prefix("==") {
            if let Ok(num) = num_str.parse::<u64>() {
                return context.member_count == num;
            }
        } else if let Some(num_str) = is_condition.strip_prefix(">") {
            if let Ok(num) = num_str.parse::<u64>() {
                return context.member_count > num;
            }
        } else if let Some(num_str) = is_condition.strip_prefix("<")
            && let Ok(num) = num_str.parse::<u64>() {
                return context.member_count < num;
            }
        false
    }

    fn evaluate_sender_notification_permission(
        &self,
        key: &str,
        event: &PDU,
        context: &RoomContext,
    ) -> bool {
        // Check if sender has required power level for notifications
        if let Some(sender_power) = context.power_levels.get(&event.sender) {
            // Default notification power level is 50
            let required_power = match key {
                "room" => 50,
                _ => 0,
            };
            *sender_power >= required_power
        } else {
            false
        }
    }

    fn evaluate_event_property_contains(&self, key: &str, value: &str, event: &PDU) -> bool {
        // For Matrix v1.7 user mentions: "content.m\\.mentions.user_ids"
        if key == "content.m\\.mentions.user_ids" {
            if let Some(mentions) = event.content.get("m.mentions") {
                if let Some(user_ids) = mentions.get("user_ids") {
                    if let Some(user_ids_array) = user_ids.as_array() {
                        return user_ids_array.iter().any(|id| {
                            id.as_str().map_or(false, |id_str| id_str.contains(value))
                        });
                    }
                }
            }
        }
        false
    }

    fn evaluate_event_property_is(&self, key: &str, value: &serde_json::Value, event: &PDU) -> bool {
        // For Matrix v1.7 room mentions: "content.m\\.mentions.room"
        if key == "content.m\\.mentions.room" {
            if let Some(mentions) = event.content.get("m.mentions") {
                if let Some(room_mention) = mentions.get("room") {
                    return room_mention == value;
                }
            }
        }
        false
    }

    /// Create a push rule for a user using the repository
    pub async fn create_push_rule(
        &self,
        user_id: &str,
        rule: &PushRule,
    ) -> Result<(), RepositoryError> {
        self.push_repo.create_push_rule(user_id, rule).await
    }

    /// Get user's push rules using the repository
    pub async fn get_user_push_rules(
        &self,
        user_id: &str,
    ) -> Result<Vec<PushRule>, RepositoryError> {
        self.push_repo.get_user_push_rules(user_id).await
    }

    /// Update a push rule using the repository
    pub async fn update_push_rule(
        &self,
        user_id: &str,
        rule_id: &str,
        rule: &PushRule,
    ) -> Result<(), RepositoryError> {
        self.push_repo.update_push_rule(user_id, rule_id, rule).await
    }

    /// Delete a push rule using the repository
    pub async fn delete_push_rule(
        &self,
        user_id: &str,
        rule_id: &str,
    ) -> Result<(), RepositoryError> {
        self.push_repo.delete_push_rule(user_id, rule_id).await
    }

    /// Get a specific push rule using the repository
    pub async fn get_push_rule_by_id(
        &self,
        user_id: &str,
        rule_id: &str,
    ) -> Result<Option<PushRule>, RepositoryError> {
        self.push_repo.get_push_rule_by_id(user_id, rule_id).await
    }

    /// Enable or disable a push rule using the repository
    pub async fn enable_push_rule(
        &self,
        user_id: &str,
        rule_id: &str,
        enabled: bool,
    ) -> Result<(), RepositoryError> {
        self.push_repo.enable_push_rule(user_id, rule_id, enabled).await
    }

    /// Reset user's push rules to defaults using the repository
    pub async fn reset_user_push_rules(&self, user_id: &str) -> Result<(), RepositoryError> {
        self.push_repo.reset_user_push_rules(user_id).await
    }

    pub fn get_default_rules() -> Vec<PushRule> {
        vec![
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
            // Matrix v1.7 user mention rule
            PushRule {
                rule_id: ".m.rule.is_user_mention".to_string(),
                priority_class: 4, // Content rules
                priority: 1,
                conditions: vec![PushCondition::EventPropertyContains {
                    key: "content.m\\.mentions.user_ids".to_string(),
                    value: "[the user's Matrix ID]".to_string(), // Will be replaced at runtime
                }],
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
            // Matrix v1.7 room mention rule
            PushRule {
                rule_id: ".m.rule.is_room_mention".to_string(),
                priority_class: 4, // Content rules
                priority: 2,
                conditions: vec![
                    PushCondition::EventPropertyIs {
                        key: "content.m\\.mentions.room".to_string(),
                        value: serde_json::Value::Bool(true),
                    },
                    PushCondition::SenderNotificationPermission {
                        key: "room".to_string(),
                    },
                ],
                actions: vec![
                    PushAction::Notify,
                    PushAction::SetTweak {
                        set_tweak: "highlight".to_string(),
                        value: serde_json::Value::Bool(true),
                    },
                ],
                default: true,
                enabled: true,
            },
            // Room rules
            // (Room-specific rules would be added here)

            // Sender rules
            // (Sender-specific rules would be added here)

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
        ]
    }
}
