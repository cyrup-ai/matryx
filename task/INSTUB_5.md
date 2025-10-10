# INSTUB_5: Push Rule Evaluation Implementation

**Priority**: HIGH  
**Estimated Effort**: 1 session  
**Category**: Push Notifications

---

## OBJECTIVE

Implement push rule evaluation logic to trigger notifications when events match user-configured push rules, completing the Matrix push notification system.

**WHY**: Push rules exist in the database but aren't evaluated when events are created. Users don't receive notifications even when rules are configured. The notification infrastructure (push gateway, pushers) exists - we just need the evaluation engine.

---

## BACKGROUND

**Current Issue**: Test code shows push rule evaluation is incomplete:
```rust
// 4. Verify push rule evaluation (would need mock push gateway to fully test)
// For now, just verify the pusher was set up correctly
```

**What We Have**:
- ✅ `PushRuleRepository` - Store/retrieve rules
- ✅ `PushGatewayRepository` - Send notifications
- ✅ `PusherRepository` - Manage push endpoints
- ❌ **Missing**: Evaluation engine that matches events against rules

**Matrix Spec Requirement**: Events must be evaluated against user push rules to determine if/how to notify.

**Location**: [`packages/surrealdb/src/repository/push_service.rs`](../packages/surrealdb/src/repository/push_service.rs) (may need creation)

---

## SUBTASK 1: Create or Locate PushService

**WHAT**: Find or create the push evaluation service.

**WHERE**: Check [`packages/surrealdb/src/repository/push_service.rs`](../packages/surrealdb/src/repository/push_service.rs)

**IF FILE EXISTS**: Review current implementation and identify what's missing.

**IF MISSING**: Create new file with base structure:

```rust
use crate::repository::error::RepositoryError;
use crate::repository::push_rule::PushRuleRepository;
use crate::repository::event::EventRepository;
use matryx_entity::types::{Event, PushRule, PushAction};
use surrealdb::{Surreal, engine::any::Any};

pub struct PushService {
    db: Surreal<Any>,
    push_rule_repo: PushRuleRepository,
    event_repo: EventRepository,
}

impl PushService {
    pub fn new(db: Surreal<Any>) -> Self {
        Self {
            db: db.clone(),
            push_rule_repo: PushRuleRepository::new(db.clone()),
            event_repo: EventRepository::new(db.clone()),
        }
    }
}
```

**ADD TO** [`packages/surrealdb/src/repository/mod.rs`](../packages/surrealdb/src/repository/mod.rs):
```rust
pub mod push_service;
pub use push_service::PushService;
```

**DEFINITION OF DONE**:
- ✅ PushService file exists
- ✅ Basic structure with dependencies set up
- ✅ Exported from repository module

---

## SUBTASK 2: Implement Core Rule Evaluation Method

**WHAT**: Create method that evaluates if an event matches user's push rules.

**WHERE**: [`packages/surrealdb/src/repository/push_service.rs`](../packages/surrealdb/src/repository/push_service.rs)

**IMPLEMENT**:
```rust
impl PushService {
    /// Evaluate push rules for a user and event
    /// Returns list of actions to take if rules match
    pub async fn evaluate_push_rules(
        &self,
        user_id: &str,
        event: &Event,
        room_member_count: usize,
    ) -> Result<Vec<PushAction>, RepositoryError> {
        // Get user's push rules sorted by priority
        let rules = self.push_rule_repo.get_user_rules(user_id).await?;
        
        let mut actions = Vec::new();
        
        for rule in rules {
            // Check if rule matches this event
            if self.rule_matches_event(&rule, event, room_member_count).await? {
                // Add rule's actions
                actions.extend(rule.actions.clone());
                
                // Stop if rule says to stop processing
                if rule.enabled && self.should_stop_processing(&rule) {
                    break;
                }
            }
        }
        
        Ok(actions)
    }
    
    /// Check if we should stop processing more rules
    fn should_stop_processing(&self, rule: &PushRule) -> bool {
        // Check if rule actions include "dont_notify" or similar
        // This prevents lower-priority rules from triggering
        rule.actions.iter().any(|action| {
            matches!(action, PushAction::DontNotify)
        })
    }
}
```

**KEY CONCEPTS**:
- Rules are evaluated in priority order
- First matching rule wins (if it has stop_processing)
- Actions are cumulative unless stopped

**DEFINITION OF DONE**:
- ✅ Core evaluation method implemented
- ✅ Priority ordering respected
- ✅ Stop processing logic works

---

## SUBTASK 3: Implement Rule Condition Matching

**WHAT**: Implement the logic that checks if an event matches rule conditions.

**WHERE**: Same file, new method

**IMPLEMENT**:
```rust
impl PushService {
    /// Check if a push rule matches an event
    async fn rule_matches_event(
        &self,
        rule: &PushRule,
        event: &Event,
        room_member_count: usize,
    ) -> Result<bool, RepositoryError> {
        // If rule is disabled, it never matches
        if !rule.enabled {
            return Ok(false);
        }
        
        // Check all conditions - ALL must pass
        for condition in &rule.conditions {
            if !self.evaluate_condition(condition, event, room_member_count).await? {
                return Ok(false);
            }
        }
        
        Ok(true)
    }
    
    /// Evaluate a single condition
    async fn evaluate_condition(
        &self,
        condition: &PushCondition,
        event: &Event,
        room_member_count: usize,
    ) -> Result<bool, RepositoryError> {
        match condition.kind.as_str() {
            "event_match" => {
                // Pattern matching on event fields
                self.evaluate_event_match(condition, event)
            }
            "contains_display_name" => {
                // Check if event contains user's display name
                self.evaluate_contains_display_name(condition, event).await
            }
            "room_member_count" => {
                // Check room member count
                self.evaluate_room_member_count(condition, room_member_count)
            }
            "sender_notification_permission" => {
                // Check if sender has permission to notify
                self.evaluate_sender_permission(condition, event).await
            }
            _ => {
                tracing::warn!("Unknown push condition type: {}", condition.kind);
                Ok(false)
            }
        }
    }
}
```

**DEFINITION OF DONE**:
- ✅ Condition matching logic implemented
- ✅ All conditions must pass for rule to match
- ✅ Unknown condition types handled gracefully

---

## SUBTASK 4: Implement Individual Condition Evaluators

**WHAT**: Implement the specific condition type evaluators.

**WHERE**: Same file, helper methods

**IMPLEMENT BASIC EVALUATORS**:

```rust
impl PushService {
    /// Evaluate event_match condition (pattern matching)
    fn evaluate_event_match(
        &self,
        condition: &PushCondition,
        event: &Event,
    ) -> Result<bool, RepositoryError> {
        let key = condition.key.as_deref().unwrap_or("");
        let pattern = condition.pattern.as_deref().unwrap_or("");
        
        // Get field value from event
        let field_value = match key {
            "type" => Some(event.event_type.as_str()),
            "sender" => Some(event.sender.as_str()),
            "room_id" => Some(event.room_id.as_str()),
            "content.body" => {
                // Extract body from event content
                if let EventContent::Unknown(ref content) = event.content {
                    content.get("body").and_then(|v| v.as_str())
                } else {
                    None
                }
            }
            _ => None,
        };
        
        // Match pattern (simple glob matching for now)
        if let Some(value) = field_value {
            Ok(self.glob_match(pattern, value))
        } else {
            Ok(false)
        }
    }
    
    /// Simple glob pattern matching
    fn glob_match(&self, pattern: &str, value: &str) -> bool {
        // Simple implementation: check if pattern is in value
        // For production, use proper glob or regex
        if pattern.contains('*') {
            // Very basic wildcard support
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                value.starts_with(parts[0]) && value.ends_with(parts[1])
            } else {
                value.contains(pattern.replace('*', "").as_str())
            }
        } else {
            // Exact match
            value == pattern
        }
    }
    
    /// Evaluate room_member_count condition
    fn evaluate_room_member_count(
        &self,
        condition: &PushCondition,
        room_member_count: usize,
    ) -> Result<bool, RepositoryError> {
        let is_value = condition.is.as_deref().unwrap_or("");
        
        // Parse condition like "2", "<10", ">=5"
        if let Some(num_str) = is_value.strip_prefix(">=") {
            if let Ok(num) = num_str.parse::<usize>() {
                return Ok(room_member_count >= num);
            }
        } else if let Some(num_str) = is_value.strip_prefix("<=") {
            if let Ok(num) = num_str.parse::<usize>() {
                return Ok(room_member_count <= num);
            }
        } else if let Some(num_str) = is_value.strip_prefix('<') {
            if let Ok(num) = num_str.parse::<usize>() {
                return Ok(room_member_count < num);
            }
        } else if let Some(num_str) = is_value.strip_prefix('>') {
            if let Ok(num) = num_str.parse::<usize>() {
                return Ok(room_member_count > num);
            }
        } else if let Ok(num) = is_value.parse::<usize>() {
            return Ok(room_member_count == num);
        }
        
        Ok(false)
    }
    
    /// Placeholder for display name check (can enhance later)
    async fn evaluate_contains_display_name(
        &self,
        _condition: &PushCondition,
        _event: &Event,
    ) -> Result<bool, RepositoryError> {
        // For now, return false
        // Full implementation would:
        // 1. Get user's display name
        // 2. Check if event content contains it
        Ok(false)
    }
    
    /// Placeholder for sender permission check
    async fn evaluate_sender_permission(
        &self,
        _condition: &PushCondition,
        _event: &Event,
    ) -> Result<bool, RepositoryError> {
        // For now, return true (allow all)
        // Full implementation would check power levels
        Ok(true)
    }
}
```

**NOTE**: This implements basic versions. `contains_display_name` and `sender_notification_permission` are marked as enhanceable later.

**DEFINITION OF DONE**:
- ✅ event_match condition works (basic pattern matching)
- ✅ room_member_count condition works
- ✅ Placeholder conditions return safe defaults
- ✅ Can evaluate most common push rules

---

## SUBTASK 5: Integrate with Event Creation

**WHAT**: Call push evaluation when events are created/received.

**WHERE**: Event creation endpoint, likely [`packages/server/src/_matrix/client/v3/rooms/by_room_id/send.rs`](../packages/server/src/_matrix/client/v3/rooms/by_room_id/send.rs) or similar

**FIND**: The handler that creates events (message send, state events, etc.)

**ADD EVALUATION CALL**:
```rust
// After event is successfully created/stored
let event = event_repo.create(&event).await?;

// Evaluate push rules for room members
let room_members = membership_repo.get_room_members(&room_id).await?;
let member_count = room_members.len();

// Evaluate for each member
for member in room_members {
    let actions = push_service
        .evaluate_push_rules(&member.user_id, &event, member_count)
        .await?;
    
    if !actions.is_empty() {
        // Trigger push notification
        tokio::spawn(async move {
            if let Err(e) = send_push_notification(&member.user_id, &event, actions).await {
                tracing::error!("Failed to send push notification: {}", e);
            }
        });
    }
}
```

**IMPORTANT**: Don't block event creation on push evaluation. Use async spawn for notification delivery.

**DEFINITION OF DONE**:
- ✅ Push evaluation called after event creation
- ✅ Evaluated for all room members
- ✅ Doesn't block event creation
- ✅ Errors logged but don't fail event

---

## SUBTASK 6: Add PushService to AppState

**WHAT**: Make PushService available throughout the server.

**WHERE**: [`packages/server/src/state.rs`](../packages/server/src/state.rs)

**ADD**:
```rust
pub struct AppState {
    pub db: Surreal<Any>,
    pub session_service: SessionService,
    pub homeserver_name: String,
    pub push_service: PushService,  // ADD THIS
    // ... other fields
}

impl AppState {
    pub fn new(db: Surreal<Any>, homeserver_name: String) -> Self {
        Self {
            db: db.clone(),
            session_service: SessionService::new(db.clone()),
            homeserver_name: homeserver_name.clone(),
            push_service: PushService::new(db.clone()),  // INITIALIZE
            // ... other fields
        }
    }
}
```

**IMPORTS**:
```rust
use matryx_surrealdb::repository::push_service::PushService;
```

**DEFINITION OF DONE**:
- ✅ PushService in AppState
- ✅ Initialized in constructor
- ✅ Available to all endpoints

---

## SUBTASK 7: Verify Compilation

**WHAT**: Ensure all code compiles together.

**WHERE**: Run from workspace root

**HOW**:
```bash
# Build packages
cargo build --package matryx_surrealdb
cargo build --package matryx_server

# Check for errors
cargo check --workspace
```

**FIX**: Any type mismatches, missing imports, or compilation errors.

**DEFINITION OF DONE**:
- ✅ All packages compile
- ✅ No type errors
- ✅ Integration works

---

## RESEARCH NOTES

### Matrix Push Rule Specification
Location: [`./spec/client/05_advanced_features.md`](../spec/client/05_advanced_features.md)

**Push Rule Structure**:
```json
{
  "rule_id": "rule1",
  "priority_class": 5,
  "priority": 10,
  "enabled": true,
  "conditions": [
    {
      "kind": "event_match",
      "key": "content.body",
      "pattern": "hello"
    }
  ],
  "actions": ["notify", {"set_tweak": "sound", "value": "default"}]
}
```

**Condition Types**:
- `event_match` - Pattern matching on event fields
- `contains_display_name` - Event contains user's display name
- `room_member_count` - Number of room members (e.g., "2" for 1:1, "<10" for small group)
- `sender_notification_permission` - Sender has permission to notify

**Actions**:
- `notify` - Send notification
- `dont_notify` - Don't notify (suppress)
- `coalesce` - Coalesce with other notifications
- `set_tweak` - Set notification tweaks (sound, highlight, etc.)

### PushRule Repository
Location: [`packages/surrealdb/src/repository/push_rule.rs`](../packages/surrealdb/src/repository/push_rule.rs)

Key method:
- `get_user_rules(user_id)` - Returns rules sorted by priority

### Architecture
```
Event Creation
    ↓
PushService.evaluate_push_rules()
    ↓
Match conditions → Get actions
    ↓
PushGateway.send_notification()
    ↓
Push Provider (APNs, FCM, etc.)
```

---

## DEFINITION OF DONE

**Task complete when**:
- ✅ PushService created with evaluation logic
- ✅ Core rule evaluation method works
- ✅ Condition matchers implemented (at least basic ones)
- ✅ event_match and room_member_count conditions work
- ✅ Integrated with event creation flow
- ✅ PushService available in AppState
- ✅ Code compiles successfully
- ✅ Push notifications trigger when rules match

**ACCEPTABLE TO DEFER**:
- Full implementation of `contains_display_name` (placeholder OK)
- Full implementation of `sender_notification_permission` (placeholder OK)
- Advanced glob/regex patterns (basic matching OK)
- Actual push gateway delivery (can log for now)

**NO REQUIREMENTS FOR**:
- ❌ Unit tests
- ❌ Integration tests
- ❌ Benchmarks
- ❌ Documentation (beyond code comments)

---

## RELATED FILES

- [`packages/surrealdb/src/repository/push_service.rs`](../packages/surrealdb/src/repository/push_service.rs) - Create/modify this
- [`packages/surrealdb/src/repository/push_rule.rs`](../packages/surrealdb/src/repository/push_rule.rs) - Rule storage
- [`packages/server/src/state.rs`](../packages/server/src/state.rs) - Add PushService
- [`packages/server/src/_matrix/client/v3/rooms/by_room_id/send.rs`](../packages/server/src/_matrix/client/v3/rooms/by_room_id/send.rs) - Likely integration point
- [`./spec/client/05_advanced_features.md`](../spec/client/05_advanced_features.md) - Push notification spec
