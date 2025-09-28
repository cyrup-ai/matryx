use serde_json::json;
use tracing::info;

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
        match state
            .server_notice_repository
            .get_server_notice_room(user_id)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?
        {
            Some(room_id) => Ok(room_id),
            None => Err("No server notices room found".into()),
        }
    }

    /// Create a new server notices room
    async fn create_server_notices_room(
        &self,
        user_id: &str,
        state: &AppState,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // Create room using repository - this handles all room setup
        let room_id = state
            .server_notice_repository
            .create_server_notice_room(user_id, "server_notice")
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

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

        // Send notice using repository
        let event_id = state
            .server_notice_repository
            .send_server_notice(
                user_id,
                &room_id,
                &serde_json::to_value(&notice_content)?,
                &self.server_name,
            )
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

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
            "monthly_active_user" => {
                UsageLimitReachedNotice::monthly_active_user_limit(admin_contact)
            },
            _ => {
                UsageLimitReachedNotice::new(
                    format!("The server has exceeded a {} limit.", limit_type),
                    limit_type.to_string(),
                    admin_contact,
                )
            },
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
}