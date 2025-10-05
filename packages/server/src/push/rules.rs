//! Module contains intentional library code not yet fully integrated
#![allow(dead_code)]

use matryx_entity::PDU;
use matryx_surrealdb::repository::push::{
    PushAction, PushCondition, PushEvent, PushRule, PushRuleEvaluation, RoomContext,
};
use matryx_surrealdb::repository::{PushRepository, RepositoryError};

use surrealdb::Surreal;
use surrealdb::engine::any::Any;

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
        // Parse the dot-separated path
        let path_parts = Self::parse_dot_path(key);

        if path_parts.is_empty() {
            return false;
        }

        // Special case: content.body uses word boundary matching
        if key == "content.body" {
            if let Some(body_value) = event.content.get("body")
                && let Some(body_str) = body_value.as_str()
            {
                return Self::match_at_word_boundary(body_str, pattern);
            }
            return false;
        }

        // Get the value to match against based on the first path part
        match path_parts[0].as_str() {
            // Top-level PDU fields
            "type" => Self::glob_match(pattern, &event.event_type),
            "sender" => Self::glob_match(pattern, &event.sender),
            "room_id" => Self::glob_match(pattern, &event.room_id),
            "state_key" => {
                if let Some(state_key) = &event.state_key {
                    Self::glob_match(pattern, state_key)
                } else {
                    false
                }
            },

            // Content fields - need to navigate path
            "content" if path_parts.len() > 1 => {
                // Convert EventContent to JSON for navigation
                if let Ok(content_json) = serde_json::to_value(&event.content)
                    && let Some(content_obj) = content_json.as_object()
                {
                    // Navigate the remaining path parts
                    let remaining_path = &path_parts[1..];
                    let content_value = serde_json::Value::Object(content_obj.clone());
                    if let Some(value) = Self::get_nested_value(&content_value, remaining_path) {
                        // Must be a string value
                        if let Some(value_str) = value.as_str() {
                            return Self::glob_match(pattern, value_str);
                        }
                    }
                }
                false
            },

            _ => false,
        }
    }

    fn evaluate_contains_display_name(&self, event: &PDU, context: &RoomContext) -> bool {
        if let Some(display_name) = &context.user_display_name
            && let Some(body) = event.content.get("body")
            && let Some(body_str) = body.as_str()
        {
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
            && let Ok(num) = num_str.parse::<u64>()
        {
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
        if key == "content.m\\.mentions.user_ids"
            && let Some(mentions) = event.content.get("m.mentions")
            && let Some(user_ids) = mentions.get("user_ids")
            && let Some(user_ids_array) = user_ids.as_array()
        {
            return user_ids_array
                .iter()
                .any(|id| id.as_str().is_some_and(|id_str| id_str.contains(value)));
        }
        false
    }

    fn evaluate_event_property_is(
        &self,
        key: &str,
        value: &serde_json::Value,
        event: &PDU,
    ) -> bool {
        // For Matrix v1.7 room mentions: "content.m\\.mentions.room"
        if key == "content.m\\.mentions.room"
            && let Some(mentions) = event.content.get("m.mentions")
            && let Some(room_mention) = mentions.get("room")
        {
            return room_mention == value;
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
                    PushCondition::SenderNotificationPermission { key: "room".to_string() },
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

    // Helper functions for event matching

    /// Match a string against a glob pattern case-insensitively
    /// Supports: * (zero or more chars), ? (exactly one char)
    fn glob_match(pattern: &str, value: &str) -> bool {
        let pattern_lower = pattern.to_lowercase();
        let value_lower = value.to_lowercase();

        Self::glob_match_internal(pattern_lower.as_str(), value_lower.as_str())
    }

    fn glob_match_internal(pattern: &str, value: &str) -> bool {
        let mut p_chars = pattern.chars().peekable();
        let mut v_chars = value.chars().peekable();

        while let Some(&p) = p_chars.peek() {
            match p {
                '*' => {
                    p_chars.next();
                    // If * is last char in pattern, match rest of string
                    if p_chars.peek().is_none() {
                        return true;
                    }
                    // Try matching rest of pattern at each position
                    while v_chars.peek().is_some() {
                        if Self::glob_match_internal(
                            p_chars.clone().collect::<String>().as_str(),
                            v_chars.clone().collect::<String>().as_str(),
                        ) {
                            return true;
                        }
                        v_chars.next();
                    }
                    return false;
                },
                '?' => {
                    p_chars.next();
                    if v_chars.next().is_none() {
                        return false;
                    }
                },
                _ => {
                    p_chars.next();
                    if Some(p) != v_chars.next() {
                        return false;
                    }
                },
            }
        }

        v_chars.peek().is_none()
    }

    /// Parse dot-separated path with escape sequences
    /// content.m\.relates_to → ["content", "m.relates_to"]
    /// content.m\\foo → ["content", "m\foo"]
    fn parse_dot_path(path: &str) -> Vec<String> {
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut chars = path.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '\\' {
                // Escape sequence
                if let Some(&next) = chars.peek() {
                    if next == '.' || next == '\\' {
                        // Escaped dot or backslash
                        if let Some(escaped) = chars.next() {
                            current.push(escaped);
                        }
                    } else {
                        // Other escapes: keep both chars
                        current.push(ch);
                    }
                } else {
                    // Trailing backslash
                    current.push(ch);
                }
            } else if ch == '.' {
                // Path separator
                if !current.is_empty() {
                    parts.push(current.clone());
                    current.clear();
                }
            } else {
                current.push(ch);
            }
        }

        if !current.is_empty() {
            parts.push(current);
        }

        parts
    }

    /// Get value from JSON using dot-separated path parts
    fn get_nested_value<'a>(
        value: &'a serde_json::Value,
        path_parts: &[String],
    ) -> Option<&'a serde_json::Value> {
        let mut current = value;

        for part in path_parts {
            current = current.get(part)?;
        }

        Some(current)
    }

    /// Check if pattern matches substring at word boundaries
    /// Word boundary = start/end of string or non-alphanumeric/underscore char
    fn match_at_word_boundary(body: &str, pattern: &str) -> bool {
        let body_lower = body.to_lowercase();
        let pattern_lower = pattern.to_lowercase();

        // Find all occurrences of pattern
        let mut start = 0;
        while let Some(pos) = body_lower[start..].find(&pattern_lower) {
            let actual_pos = start + pos;
            let end_pos = actual_pos + pattern_lower.len();

            // Check start boundary
            let start_ok = actual_pos == 0 || {
                if let Some(prev_char) = body_lower[..actual_pos].chars().last() {
                    !prev_char.is_alphanumeric() && prev_char != '_'
                } else {
                    false
                }
            };

            // Check end boundary
            let end_ok = end_pos == body_lower.len() || {
                if let Some(next_char) = body_lower[end_pos..].chars().next() {
                    !next_char.is_alphanumeric() && next_char != '_'
                } else {
                    false
                }
            };

            if start_ok && end_ok {
                return true;
            }

            start = actual_pos + 1;
        }

        false
    }
}
