use crate::db::entity::Message;
use crate::db::generic_dao::Dao;
use crate::future::MatrixFuture;
use crate::db::client::DatabaseClient;
use serde_json::json;
use chrono::{DateTime, Utc};

pub struct MessageDao {
    dao: Dao<Message>,
}

// Helper type for multiple messages
pub struct MessageList {
    messages: Vec<Message>,
}

impl MessageList {
    pub fn get_all(self) -> Vec<Message> {
        self.messages
    }
    
    pub fn first(self) -> Option<Message> {
        self.messages.into_iter().next()
    }
    
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

// Helper type for a single message
pub struct MessageItem {
    message: Option<Message>,
}

impl MessageItem {
    pub fn get(self) -> Option<Message> {
        self.message
    }
}

impl MessageDao {
    pub fn new(client: DatabaseClient) -> Self {
        Self { dao: Dao::new(client) }
    }
    
    // Find messages by room with a limit
    pub fn find_by_room(&self, room_id: &str, limit: usize) -> MatrixFuture<MessageList> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        
        MatrixFuture::spawn(async move {
            // Query to get messages with given parameters
            let results: Vec<Message> = dao.query_with_params(
                "SELECT * FROM message WHERE room_id = $room ORDER BY sent_at DESC LIMIT $limit",
                json!({ "room": room_id, "limit": limit })
            ).await?;
            
            Ok(MessageList { messages: results })
        })
    }
    
    // Find messages by room since a given timestamp
    pub fn find_by_room_since(
        &self,
        room_id: &str,
        since: DateTime<Utc>,
    ) -> MatrixFuture<MessageList> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        
        MatrixFuture::spawn(async move {
            // Query messages since a timestamp
            let results: Vec<Message> = dao.query_with_params(
                "SELECT * FROM message WHERE room_id = $room AND sent_at > $since ORDER BY sent_at ASC",
                json!({ "room": room_id, "since": since })
            ).await?;
            
            Ok(MessageList { messages: results })
        })
    }
    
    // Find messages from a specific sender
    pub fn find_by_sender(&self, sender_id: &str, limit: usize) -> MatrixFuture<MessageList> {
        let dao = self.dao.clone();
        let sender_id = sender_id.to_string();
        
        MatrixFuture::spawn(async move {
            // Query by sender ID
            let results: Vec<Message> = dao.query_with_params(
                "SELECT * FROM message WHERE sender_id = $sender ORDER BY sent_at DESC LIMIT $limit",
                json!({ "sender": sender_id, "limit": limit })
            ).await?;
            
            Ok(MessageList { messages: results })
        })
    }
    
    // Add a reaction to a message
    pub fn add_reaction(&self, message_id: &str, emoji: &str, user_id: &str) -> MatrixFuture<MessageItem> {
        let dao = self.dao.clone();
        let message_id = message_id.to_string();
        let emoji = emoji.to_string();
        let user_id = user_id.to_string();
        
        MatrixFuture::spawn(async move {
            // Execute update query
            let results: Vec<Message> = dao.query_with_params(
                r#"
                UPDATE message 
                SET reactions = array::append(
                    reactions, 
                    { emoji: $emoji, user_id: $user, reacted_at: time::now() }
                )
                WHERE id = $id
                "#,
                json!({ "id": message_id, "emoji": emoji, "user": user_id })
            ).await?;
            
            let message = if results.is_empty() {
                None
            } else {
                Some(results[0].clone())
            };
            
            Ok(MessageItem { message })
        })
    }
    
    // Remove a reaction from a message
    pub fn remove_reaction(&self, message_id: &str, emoji: &str, user_id: &str) -> MatrixFuture<MessageItem> {
        let dao = self.dao.clone();
        let message_id = message_id.to_string();
        let emoji = emoji.to_string();
        let user_id = user_id.to_string();
        
        MatrixFuture::spawn(async move {
            // Execute update query
            let results: Vec<Message> = dao.query_with_params(
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
            ).await?;
            
            let message = if results.is_empty() {
                None
            } else {
                Some(results[0].clone())
            };
            
            Ok(MessageItem { message })
        })
    }
    
    // Search for messages containing specific content
    pub fn search_content(&self, room_id: &str, query: &str) -> MatrixFuture<MessageList> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        let query = query.to_string();
        
        MatrixFuture::spawn(async move {
            // Execute search query
            let results: Vec<Message> = dao.query_with_params(
                "SELECT * FROM message WHERE room_id = $room AND content CONTAINS $query ORDER BY sent_at DESC",
                json!({ "room": room_id, "query": query })
            ).await?;
            
            Ok(MessageList { messages: results })
        })
    }
}