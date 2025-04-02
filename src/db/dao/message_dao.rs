use crate::db::{Dao, Message, Result};
use serde_json::json;
use chrono::{DateTime, Utc};

pub struct MessageDao {
    dao: Dao<Message>,
}

impl MessageDao {
    pub fn new() -> Self {
        Self { dao: Dao::new() }
    }
    
    pub async fn find_by_room(&self, room_id: &str, limit: usize) -> Result<Vec<Message>> {
        self.dao.query_with_params(
            "SELECT * FROM message WHERE room_id = $room ORDER BY sent_at DESC LIMIT $limit",
            json!({ "room": room_id, "limit": limit })
        ).await
    }
    
    pub async fn find_by_room_since(&self, room_id: &str, since: DateTime<Utc>) -> Result<Vec<Message>> {
        self.dao.query_with_params(
            "SELECT * FROM message WHERE room_id = $room AND sent_at > $since ORDER BY sent_at ASC",
            json!({ "room": room_id, "since": since })
        ).await
    }
    
    pub async fn find_by_sender(&self, sender_id: &str, limit: usize) -> Result<Vec<Message>> {
        self.dao.query_with_params(
            "SELECT * FROM message WHERE sender_id = $sender ORDER BY sent_at DESC LIMIT $limit",
            json!({ "sender": sender_id, "limit": limit })
        ).await
    }
    
    pub async fn add_reaction(&self, message_id: &str, emoji: &str, user_id: &str) -> Result<Option<Message>> {
        self.dao.query_with_params(
            r#"
            UPDATE message 
            SET reactions = array::append(
                reactions, 
                { emoji: $emoji, user_id: $user, reacted_at: time::now() }
            )
            WHERE id = $id
            "#,
            json!({ "id": message_id, "emoji": emoji, "user": user_id })
        ).await
    }
    
    pub async fn remove_reaction(&self, message_id: &str, emoji: &str, user_id: &str) -> Result<Option<Message>> {
        self.dao.query_with_params(
            r#"
            UPDATE message 
            SET reactions = array::filter(
                reactions, 
                fn($reaction) { 
                    return !($reaction.emoji == $emoji && $reaction.user_id == $user);
                }
            )
            WHERE id = $id
            "#,
            json!({ "id": message_id, "emoji": emoji, "user": user_id })
        ).await
    }
    
    pub async fn search_content(&self, room_id: &str, query: &str) -> Result<Vec<Message>> {
        self.dao.query_with_params(
            "SELECT * FROM message WHERE room_id = $room AND content CONTAINS $query ORDER BY sent_at DESC",
            json!({ "room": room_id, "query": query })
        ).await
    }
}