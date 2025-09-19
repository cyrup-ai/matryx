use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::state::AppState;
use matryx_entity::{ServerNoticeContent, UsageLimitReachedNotice};

/// Server notices room management
pub struct ServerNoticesManager {
    server_name: String,
}

impl ServerNoticesManager {
    pub fn new(server_name: String) -> Self {
        Self { server_name }
    }

    /// Create or get the server notices room for a user
    pub async fn get_or_create_server_notices_room(
        &self,
        user_id: &str,
        state: &AppState,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // Check if user already has a server notices room
        if let Ok(room_id) = self.get_existing_server_notices_room(user_id, state).await {
            return Ok(room_id);
        }

        // Create new server notices room
        self.create_server_notices_room(user_id, state).await
    }

    /// Get existing server notices room for user
    async fn get_existing_server_notices_room(
        &self,
        user_id: &str,
        state: &AppState,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let query = "
            SELECT room_id
            FROM room_account_data 
            WHERE user_id = $user_id 
            AND data_type = 'm.tag' 
            AND content.tags CONTAINS 'm.server_notice'
            LIMIT 1
        ";
        
        let mut result = state.db
            .query(query)
            .bind(("user_id", user_id))
            .await?;

        let rooms: Vec<Value> = result.take(0)?;
        
        if let Some(room) = rooms.first() {
            if let Some(room_id) = room.get("room_id").and_then(|v| v.as_str()) {
                return Ok(room_id.to_string());
            }
        }

        Err("No server notices room found".into())
    }

    /// Create a new server notices room
    async fn create_server_notices_room(
        &self,
        user_id: &str,
        state: &AppState,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let room_id = format!("!{}", Uuid::new_v4().to_string());
        let server_user_id = format!("@server:{}", self.server_name);

        info!("Creating server notices room {} for user {}", room_id, user_id);

        // Create room
        let create_room_query = "
            CREATE rooms SET
                room_id = $room_id,
                creator = $creator,
                room_version = '10',
                created_at = time::now()
        ";
        
        state.db
            .query(create_room_query)
            .bind(("room_id", &room_id))
            .bind(("creator", &server_user_id))
            .await?;

        // Add m.room.create event
        self.add_room_create_event(&room_id, &server_user_id, state).await?;

        // Add m.room.name event
        self.add_room_name_event(&room_id, &server_user_id, "Server Notices", state).await?;

        // Add m.room.topic event
        self.add_room_topic_event(&room_id, &server_user_id, "Important notices from your server administrator", state).await?;

        // Invite the target user
        self.invite_user_to_room(&room_id, user_id, &server_user_id, state).await?;

        // Add m.server_notice tag to room for user
        self.add_server_notice_tag(&room_id, user_id, state).await?;

        info!("Successfully created server notices room {} for user {}", room_id, user_id);

        Ok(room_id)
    }

    /// Send a server notice to a user
    pub async fn send_server_notice(
        &self,
        user_id: &str,
        notice_content: ServerNoticeContent,
        state: &AppState,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // Validate notice content
        notice_content.validate()?;

        // Get or create server notices room
        let room_id = self.get_or_create_server_notices_room(user_id, state).await?;

        // Create message event
        let event_id = format!("${}", Uuid::new_v4().to_string());
        let server_user_id = format!("@server:{}", self.server_name);
        let timestamp = chrono::Utc::now().timestamp_millis() as u64;

        let message_event = json!({
            "type": "m.room.message",
            "content": notice_content,
            "event_id": event_id,
            "sender": server_user_id,
            "origin_server_ts": timestamp,
            "room_id": room_id
        });

        // Store message event
        let query = "
            CREATE room_timeline_events SET
                event_id = $event_id,
                room_id = $room_id,
                sender = $sender,
                type = $type,
                content = $content,
                origin_server_ts = $timestamp,
                created_at = time::now()
        ";
        
        state.db
            .query(query)
            .bind(("event_id", &event_id))
            .bind(("room_id", &room_id))
            .bind(("sender", &server_user_id))
            .bind(("type", "m.room.message"))
            .bind(("content", &notice_content))
            .bind(("timestamp", timestamp))
            .await?;

        // Pin the notice (active notices are pinned events)
        self.pin_server_notice(&room_id, &event_id, &server_user_id, state).await?;

        info!("Sent server notice {} to user {} in room {}", event_id, user_id, room_id);

        Ok(event_id)
    }

    /// Send a usage limit reached notice
    pub async fn send_usage_limit_notice(
        &self,
        user_id: &str,
        limit_type: &str,
        admin_contact: Option<String>,
        state: &AppState,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let notice = match limit_type {
            "monthly_active_user" => UsageLimitReachedNotice::monthly_active_user_limit(admin_contact),
            _ => UsageLimitReachedNotice::new(
                format!("The server has exceeded a {} limit.", limit_type),
                limit_type.to_string(),
                admin_contact,
            ),
        };

        let notice_content = ServerNoticeContent {
            msgtype: notice.msgtype,
            body: notice.body,
            server_notice_type: notice.server_notice_type,
            additional_data: {
                let mut map = serde_json::Map::new();
                map.insert("limit_type".to_string(), json!(notice.limit_type));
                if let Some(contact) = notice.admin_contact {
                    map.insert("admin_contact".to_string(), json!(contact));
                }
                map
            },
        };

        self.send_server_notice(user_id, notice_content, state).await
    }

    // Helper methods for room creation

    async fn add_room_create_event(
        &self,
        room_id: &str,
        creator: &str,
        state: &AppState,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let event_id = format!("${}", Uuid::new_v4().to_string());
        let timestamp = chrono::Utc::now().timestamp_millis() as u64;

        let content = json!({
            "creator": creator,
            "room_version": "10"
        });

        let query = "
            CREATE room_state_events SET
                event_id = $event_id,
                room_id = $room_id,
                sender = $sender,
                type = 'm.room.create',
                state_key = '',
                content = $content,
                origin_server_ts = $timestamp,
                created_at = time::now()
        ";
        
        state.db
            .query(query)
            .bind(("event_id", event_id))
            .bind(("room_id", room_id))
            .bind(("sender", creator))
            .bind(("content", content))
            .bind(("timestamp", timestamp))
            .await?;

        Ok(())
    }

    async fn add_room_name_event(
        &self,
        room_id: &str,
        sender: &str,
        name: &str,
        state: &AppState,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let event_id = format!("${}", Uuid::new_v4().to_string());
        let timestamp = chrono::Utc::now().timestamp_millis() as u64;

        let content = json!({ "name": name });

        let query = "
            CREATE room_state_events SET
                event_id = $event_id,
                room_id = $room_id,
                sender = $sender,
                type = 'm.room.name',
                state_key = '',
                content = $content,
                origin_server_ts = $timestamp,
                created_at = time::now()
        ";
        
        state.db
            .query(query)
            .bind(("event_id", event_id))
            .bind(("room_id", room_id))
            .bind(("sender", sender))
            .bind(("content", content))
            .bind(("timestamp", timestamp))
            .await?;

        Ok(())
    }

    async fn add_room_topic_event(
        &self,
        room_id: &str,
        sender: &str,
        topic: &str,
        state: &AppState,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let event_id = format!("${}", Uuid::new_v4().to_string());
        let timestamp = chrono::Utc::now().timestamp_millis() as u64;

        let content = json!({ "topic": topic });

        let query = "
            CREATE room_state_events SET
                event_id = $event_id,
                room_id = $room_id,
                sender = $sender,
                type = 'm.room.topic',
                state_key = '',
                content = $content,
                origin_server_ts = $timestamp,
                created_at = time::now()
        ";
        
        state.db
            .query(query)
            .bind(("event_id", event_id))
            .bind(("room_id", room_id))
            .bind(("sender", sender))
            .bind(("content", content))
            .bind(("timestamp", timestamp))
            .await?;

        Ok(())
    }

    async fn invite_user_to_room(
        &self,
        room_id: &str,
        user_id: &str,
        sender: &str,
        state: &AppState,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let event_id = format!("${}", Uuid::new_v4().to_string());
        let timestamp = chrono::Utc::now().timestamp_millis() as u64;

        let content = json!({
            "membership": "invite",
            "displayname": null,
            "avatar_url": null
        });

        let query = "
            CREATE room_memberships SET
                event_id = $event_id,
                room_id = $room_id,
                user_id = $user_id,
                sender = $sender,
                type = 'm.room.member',
                state_key = $user_id,
                content = $content,
                origin_server_ts = $timestamp,
                created_at = time::now()
        ";
        
        state.db
            .query(query)
            .bind(("event_id", event_id))
            .bind(("room_id", room_id))
            .bind(("user_id", user_id))
            .bind(("sender", sender))
            .bind(("content", content))
            .bind(("timestamp", timestamp))
            .await?;

        Ok(())
    }

    async fn add_server_notice_tag(
        &self,
        room_id: &str,
        user_id: &str,
        state: &AppState,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let content = json!({
            "tags": {
                "m.server_notice": {}
            }
        });

        let query = "
            CREATE room_account_data SET
                id = rand::uuid(),
                user_id = $user_id,
                room_id = $room_id,
                data_type = 'm.tag',
                content = $content,
                created_at = time::now(),
                updated_at = time::now()
        ";
        
        state.db
            .query(query)
            .bind(("user_id", user_id))
            .bind(("room_id", room_id))
            .bind(("content", content))
            .await?;

        Ok(())
    }

    async fn pin_server_notice(
        &self,
        room_id: &str,
        event_id: &str,
        sender: &str,
        state: &AppState,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Get current pinned events
        let query = "
            SELECT content.pinned
            FROM room_state_events 
            WHERE room_id = $room_id AND type = 'm.room.pinned_events' AND state_key = ''
            ORDER BY origin_server_ts DESC
            LIMIT 1
        ";
        
        let mut result = state.db
            .query(query)
            .bind(("room_id", room_id))
            .await?;

        let pinned_events: Vec<Value> = result.take(0)?;
        
        let mut pinned_list: Vec<String> = if let Some(event) = pinned_events.first() {
            if let Some(pinned) = event.get("pinned").and_then(|p| p.as_array()) {
                pinned.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // Add new event to pinned list
        pinned_list.push(event_id.to_string());

        // Create pinned events state event
        let pin_event_id = format!("${}", Uuid::new_v4().to_string());
        let timestamp = chrono::Utc::now().timestamp_millis() as u64;

        let content = json!({ "pinned": pinned_list });

        let query = "
            CREATE room_state_events SET
                event_id = $event_id,
                room_id = $room_id,
                sender = $sender,
                type = 'm.room.pinned_events',
                state_key = '',
                content = $content,
                origin_server_ts = $timestamp,
                created_at = time::now()
        ";
        
        state.db
            .query(query)
            .bind(("event_id", pin_event_id))
            .bind(("room_id", room_id))
            .bind(("sender", sender))
            .bind(("content", content))
            .bind(("timestamp", timestamp))
            .await?;

        Ok(())
    }
}