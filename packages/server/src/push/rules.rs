use serde::{Deserialize, Serialize};
use matryx_entity::PDU;
use std::collections::HashMap;

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

pub struct RoomContext {
    pub room_id: String,
    pub member_count: u64,
    pub user_display_name: Option<String>,
    pub power_levels: HashMap<String, i64>,
}

pub struct PushRuleEngine {
    rules: Vec<PushRule>,
}

impl PushRuleEngine {
    pub fn new(rules: Vec<PushRule>) -> Self {
        Self { rules }
    }

    pub fn evaluate_event(&self, event: &PDU, context: &RoomContext) -> Vec<PushAction> {
        let mut actions = Vec::new();
        
        // Sort rules by priority class and priority
        let mut sorted_rules = self.rules.clone();
        sorted_rules.sort_by(|a, b| {
            a.priority_class.cmp(&b.priority_class)
                .then(a.priority.cmp(&b.priority))
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

    fn evaluate_conditions(&self, conditions: &[PushCondition], event: &PDU, context: &RoomContext) -> bool {
        for condition in conditions {
            if !self.evaluate_condition(condition, event, context) {
                return false; // All conditions must match
            }
        }
        true
    }

    fn evaluate_condition(&self, condition: &PushCondition, event: &PDU, context: &RoomContext) -> bool {
        match condition {
            PushCondition::EventMatch { key, pattern } => {
                self.evaluate_event_match(key, pattern, event)
            },
            PushCondition::ContainsDisplayName => {
                self.evaluate_contains_display_name(event, context)
            },
            PushCondition::RoomMemberCount { is } => {
                self.evaluate_room_member_count(is, context)
            },
            PushCondition::SenderNotificationPermission { key } => {
                self.evaluate_sender_notification_permission(key, event, context)
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
        // Parse condition like "==2" or ">10"
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

    fn evaluate_sender_notification_permission(&self, key: &str, event: &PDU, context: &RoomContext) -> bool {
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

            // Room rules
            // (Room-specific rules would be added here)

            // Sender rules  
            // (Sender-specific rules would be added here)

            // Underride rules (lowest priority)
            PushRule {
                rule_id: ".m.rule.message".to_string(),
                priority_class: 1,
                priority: 0,
                conditions: vec![
                    PushCondition::EventMatch {
                        key: "type".to_string(),
                        pattern: "m.room.message".to_string(),
                    },
                ],
                actions: vec![PushAction::Notify],
                default: true,
                enabled: true,
            },
        ]
    }
}