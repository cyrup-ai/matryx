//! Module for processing mentions in Matrix messages

use once_cell::sync::Lazy;
use regex::Regex;
use ruma_html::Html;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashSet;
use tracing::{info, warn};

use crate::state::AppState;

/// Errors that can occur during mentions processing
#[derive(Debug, thiserror::Error)]
pub enum MentionsError {
    #[error("JSON serialization failed: {0}")]
    SerializationError(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] matryx_surrealdb::repository::error::RepositoryError),

    #[error("Invalid mention format: {0}")]
    ValidationError(String),

    #[error("User not found: {0}")]
    UserNotFound(String),

    #[error("Room not found: {0}")]
    RoomNotFound(String),
}

impl axum::response::IntoResponse for MentionsError {
    fn into_response(self) -> axum::response::Response {
        use axum::{http::StatusCode, response::Json};

        let (status, error_message) = match self {
            MentionsError::SerializationError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            },
            MentionsError::DatabaseError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Database error")
            },
            MentionsError::ValidationError(_) => (StatusCode::BAD_REQUEST, "Invalid request"),
            MentionsError::UserNotFound(_) => (StatusCode::NOT_FOUND, "User not found"),
            MentionsError::RoomNotFound(_) => (StatusCode::NOT_FOUND, "Room not found"),
        };

        let body = Json(json!({
            "errcode": "M_UNKNOWN",
            "error": error_message
        }));

        (status, body).into_response()
    }
}

/// Mentions metadata for Matrix events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MentionsMetadata {
    /// User IDs that are mentioned in the event
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_ids: Option<Vec<String>>,

    /// Whether this is a room-wide mention (@room)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room: Option<bool>,
}

/// Static regex patterns for safe compilation
static USER_MENTION_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"@([a-zA-Z0-9._=-]+):([a-zA-Z0-9.-]+\.[a-zA-Z]{2,})").unwrap_or_else(|e| {
        panic!("Invalid user mention regex pattern - this indicates a programming error: {}", e)
    })
});

static ROOM_MENTION_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"@room\b").unwrap_or_else(|e| {
        panic!("Invalid room mention regex pattern - this indicates a programming error: {}", e)
    })
});

static ROOM_ALIAS_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"#([a-zA-Z0-9._=-]+):([a-zA-Z0-9.-]+\.[a-zA-Z]{2,})").unwrap_or_else(|e| {
        panic!("Invalid room alias regex pattern - this indicates a programming error: {}", e)
    })
});

/// Mention detection and processing
pub struct MentionsProcessor;

impl MentionsProcessor {
    pub fn new() -> Self {
        Self
    }

    /// Process mentions in event content and return mentions metadata
    pub async fn process_mentions(
        &self,
        event_content: &Value,
        room_id: &str,
        sender: &str,
        state: &AppState,
    ) -> Result<Option<MentionsMetadata>, Box<dyn std::error::Error>> {
        // Extract text content from event
        let text_content = self.extract_text_content(event_content);
        if text_content.is_empty() {
            return Ok(None);
        }

        let mut mentioned_users = HashSet::new();
        let mut has_room_mention = false;

        // Check for existing m.mentions in content (client-provided)
        if let Some(existing_mentions) = event_content.get("m.mentions") {
            if let Some(user_ids) = existing_mentions.get("user_ids").and_then(|v| v.as_array()) {
                for user_id in user_ids {
                    if let Some(user_id_str) = user_id.as_str() {
                        mentioned_users.insert(user_id_str.to_string());
                    }
                }
            }

            if let Some(room) = existing_mentions.get("room").and_then(|v| v.as_bool()) {
                has_room_mention = room;
            }
        } else {
            // Detect mentions from content text (fallback for backwards compatibility)
            mentioned_users.extend(self.detect_user_mentions(&text_content, room_id, state).await?);
            has_room_mention = self.detect_room_mentions(&text_content);

            // Also detect room alias mentions for context and logging
            let room_alias_mentions = self.detect_room_alias_mentions(&text_content);
            if !room_alias_mentions.is_empty() {
                info!(
                    "Detected room alias mentions in room {}: {:?}",
                    room_id, room_alias_mentions
                );
                // Room alias mentions could be used for cross-room notifications or context
                // For now, we log them but don't add to mentions metadata as Matrix spec
                // defines m.mentions for user and @room mentions only
            }
        }

        // Remove sender from mentions (can't mention yourself)
        mentioned_users.remove(sender);

        // Validate mentioned users are in the room
        let valid_mentions = self.validate_room_members(&mentioned_users, room_id, state).await?;

        // Create mentions metadata if we have any mentions
        if !valid_mentions.is_empty() || has_room_mention {
            let mentions = MentionsMetadata {
                user_ids: if valid_mentions.is_empty() {
                    None
                } else {
                    Some(valid_mentions)
                },
                room: if has_room_mention { Some(true) } else { None },
            };

            info!("Detected mentions in room {}: {:?}", room_id, mentions);
            Ok(Some(mentions))
        } else {
            Ok(None)
        }
    }

    /// Extract text content from various event types
    fn extract_text_content(&self, content: &Value) -> String {
        let mut text = String::new();

        // Check body field
        if let Some(body) = content.get("body").and_then(|v| v.as_str()) {
            text.push_str(body);
        }

        // Check formatted_body field (HTML content)
        if let Some(formatted_body) = content.get("formatted_body").and_then(|v| v.as_str()) {
            text.push(' ');
            text.push_str(&self.strip_html(formatted_body));
        }

        text
    }

    /// Strip HTML tags from formatted content using Matrix-compliant parsing
    fn strip_html(&self, html: &str) -> String {
        match self.safe_strip_html(html) {
            Ok(text) => text,
            Err(e) => {
                warn!("HTML parsing failed: {}, falling back to original content", e);
                // Fallback to original content if parsing fails
                html.to_string()
            },
        }
    }

    /// Safely strip HTML with error handling
    fn safe_strip_html(&self, html: &str) -> Result<String, Box<dyn std::error::Error>> {
        // Parse HTML using Matrix-compliant parser
        let parsed_html = Html::parse(html);

        // Extract text content safely
        Ok(self.extract_html_text_content(&parsed_html))
    }

    /// Extract text content from HTML nodes
    fn extract_html_text_content(&self, html: &Html) -> String {
        let mut text_content = String::new();

        // Traverse HTML nodes and extract text
        for child in html.children() {
            Self::extract_node_text(&child, &mut text_content);
        }

        text_content
    }

    /// Recursively extract text from HTML nodes
    fn extract_node_text(node: &ruma_html::NodeRef, text_content: &mut String) {
        if let Some(text) = node.as_text() {
            text_content.push_str(&text.borrow());
        } else {
            // Recursively process child nodes
            for child in node.children() {
                Self::extract_node_text(&child, text_content);
            }
        }
    }

    /// Detect user mentions in text content
    async fn detect_user_mentions(
        &self,
        text: &str,
        room_id: &str,
        state: &AppState,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut mentions = Vec::new();

        // Find all @user:domain patterns
        for capture in USER_MENTION_REGEX.captures_iter(text) {
            if let Some(full_match) = capture.get(0) {
                let user_id = full_match.as_str();
                mentions.push(user_id.to_string());
            }
        }

        // Also check for display name mentions
        let display_name_mentions = self.detect_display_name_mentions(text, room_id, state).await?;
        mentions.extend(display_name_mentions);

        Ok(mentions)
    }

    /// Detect mentions by display name
    async fn detect_display_name_mentions(
        &self,
        text: &str,
        room_id: &str,
        state: &AppState,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut mentions = Vec::new();

        // Get room members with display names
        let members = state
            .mention_repository
            .get_room_members_for_mentions(room_id)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

        for (user_id, display_name) in members {
            if text.contains(&display_name) {
                mentions.push(user_id);
            }
        }

        Ok(mentions)
    }

    /// Detect @room mentions
    fn detect_room_mentions(&self, text: &str) -> bool {
        ROOM_MENTION_REGEX.is_match(text)
    }

    /// Detect room alias mentions in text content (e.g., #room:server.com)
    fn detect_room_alias_mentions(&self, text: &str) -> Vec<String> {
        let mut aliases = Vec::new();

        // Find all #room:domain patterns
        for capture in ROOM_ALIAS_REGEX.captures_iter(text) {
            if let Some(full_match) = capture.get(0) {
                let room_alias = full_match.as_str();
                aliases.push(room_alias.to_string());
                info!("Detected room alias mention: {}", room_alias);
            }
        }

        aliases
    }

    /// Validate that mentioned users are members of the room
    async fn validate_room_members(
        &self,
        mentioned_users: &HashSet<String>,
        room_id: &str,
        state: &AppState,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        if mentioned_users.is_empty() {
            return Ok(Vec::new());
        }

        let user_ids: Vec<String> = mentioned_users.iter().cloned().collect();

        let valid_user_ids = state
            .mention_repository
            .validate_mentioned_users(room_id, &user_ids)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

        Ok(valid_user_ids)
    }

    /// Add mentions metadata to event content
    pub fn add_mentions_to_content(
        &self,
        content: &mut Value,
        mentions: &MentionsMetadata,
    ) -> Result<(), MentionsError> {
        let mentions_json = serde_json::to_value(mentions)
            .map_err(|e| MentionsError::SerializationError(e.to_string()))?;

        if let Some(content_obj) = content.as_object_mut() {
            content_obj.insert("m.mentions".to_string(), mentions_json);
        }

        Ok(())
    }

    /// Trigger push notifications for mentions
    pub async fn trigger_mention_notifications(
        &self,
        mentions: &MentionsMetadata,
        event_id: &str,
        room_id: &str,
        sender: &str,
        state: &AppState,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Trigger notifications for user mentions
        if let Some(user_ids) = &mentions.user_ids {
            for user_id in user_ids {
                self.trigger_user_mention_notification(user_id, event_id, room_id, sender, state)
                    .await?;
            }
        }

        // Trigger notifications for room mentions
        if mentions.room.unwrap_or(false) {
            self.trigger_room_mention_notification(event_id, room_id, sender, state)
                .await?;
        }

        Ok(())
    }

    async fn trigger_user_mention_notification(
        &self,
        user_id: &str,
        event_id: &str,
        room_id: &str,
        sender: &str,
        state: &AppState,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Triggering mention notification for user {} in room {}", user_id, room_id);

        // Create mention notification record
        state
            .mention_repository
            .create_mention_notification(user_id, event_id, room_id, sender)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

        // Integrate with push notification system
        if let Ok(event) = state.room_operations.get_event(event_id).await
            && let Err(e) = state.push_engine.process_event(&event, room_id).await
        {
            warn!("Failed to process push notifications for mention: {}", e);
        }

        Ok(())
    }

    async fn trigger_room_mention_notification(
        &self,
        event_id: &str,
        room_id: &str,
        sender: &str,
        state: &AppState,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Triggering room mention notification in room {}", room_id);

        // Create room-wide mention notifications
        state
            .mention_repository
            .create_room_notification(event_id, room_id, sender)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

        // Integrate with push notification system
        if let Ok(event) = state.room_operations.get_event(event_id).await
            && let Err(e) = state.push_engine.process_event(&event, room_id).await
        {
            warn!("Failed to process push notifications for room mention: {}", e);
        }

        Ok(())
    }
}

impl Default for MentionsProcessor {
    fn default() -> Self {
        Self::new()
    }
}
