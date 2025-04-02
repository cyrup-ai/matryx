use crate::db::entity::AccountData;
use crate::db::generic_dao::{Dao, BaseDao};
use crate::future::MatrixFuture;
use serde_json::{json, Value};
use chrono::Utc;

pub struct AccountDataDao {
    dao: Dao<AccountData>,
}

impl AccountDataDao {
    pub fn new(client: crate::db::client::DatabaseClient) -> Self {
        Self { dao: Dao::new(client) }
    }
    
    /// Get global account data
    pub fn get_account_data(&self, event_type: &str) -> MatrixFuture<Option<Value>> {
        let dao = self.dao.clone();
        let event_type = event_type.to_string();
        
        MatrixFuture::spawn(async move {
            let data: Vec<AccountData> = dao.query_with_params::<Vec<AccountData>>(
                "SELECT * FROM account_data WHERE event_type = $type AND room_id IS NONE LIMIT 1",
                json!({ "type": event_type })
            ).await?;
            
            if let Some(account_data) = data.first() {
                Ok(Some(account_data.event.clone()))
            } else {
                Ok(None)
            }
        })
    }
    
    /// Get room account data
    pub fn get_room_account_data(&self, room_id: &str, event_type: &str) -> MatrixFuture<Option<Value>> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        let event_type = event_type.to_string();
        
        MatrixFuture::spawn(async move {
            let data: Vec<AccountData> = dao.query_with_params::<Vec<AccountData>>(
                "SELECT * FROM account_data WHERE event_type = $type AND room_id = $room LIMIT 1",
                json!({ "type": event_type, "room": room_id })
            ).await?;
            
            if let Some(account_data) = data.first() {
                Ok(Some(account_data.event.clone()))
            } else {
                Ok(None)
            }
        })
    }
    
    /// Save account data
    pub fn save_account_data(&self, event_type: &str, room_id: Option<&str>, event: Value) -> MatrixFuture<()> {
        let dao = self.dao.clone();
        let event_type = event_type.to_string();
        let room_id = room_id.map(|s| s.to_string());
        let event = event.clone();
        
        MatrixFuture::spawn(async move {
            let now = Utc::now();
            
            // Build query based on whether this is global or room account data
            let query = if let Some(room) = &room_id {
                ("UPDATE account_data SET event = $event, updated_at = $now WHERE event_type = $type AND room_id = $room",
                 json!({ "type": event_type, "room": room, "event": event, "now": now }))
            } else {
                ("UPDATE account_data SET event = $event, updated_at = $now WHERE event_type = $type AND room_id IS NONE",
                 json!({ "type": event_type, "event": event, "now": now }))
            };
            
            // Try to update if exists
            let updated: Vec<AccountData> = dao.query_with_params::<Vec<AccountData>>(
                query.0,
                query.1
            ).await?;
            
            // If not updated, create new
            if updated.is_empty() {
                let account_data = AccountData {
                    id: None,
                    event_type,
                    room_id,
                    event,
                    updated_at: now,
                };
                
                let mut account_data = account_data;
                dao.create(&mut account_data).await?;
            }
            
            Ok(())
        })
    }
    
    /// Remove room account data
    pub fn remove_room(&self, room_id: &str) -> MatrixFuture<()> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        
        MatrixFuture::spawn(async move {
            dao.query_with_params::<()>(
                "DELETE FROM account_data WHERE room_id = $room",
                json!({ "room": room_id })
            ).await?;
            
            Ok(())
        })
    }
    
    /// Get all user (global) account data events
    pub fn get_user_account_data_events(&self) -> MatrixFuture<Vec<Value>> {
        let dao = self.dao.clone();
        
        MatrixFuture::spawn(async move {
            let data: Vec<AccountData> = dao.query::<Vec<AccountData>>(
                "SELECT * FROM account_data WHERE room_id IS NONE"
            ).await?;
            
            let mut result = Vec::new();
            for account_data in data {
                result.push(account_data.event);
            }
            
            Ok(result)
        })
    }
    
    /// Get all room account data events for a specific room
    pub fn get_room_account_data_events(&self, room_id: &str) -> MatrixFuture<Vec<Value>> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        
        MatrixFuture::spawn(async move {
            let data: Vec<AccountData> = dao.query_with_params::<Vec<AccountData>>(
                "SELECT * FROM account_data WHERE room_id = $room",
                json!({ "room": room_id })
            ).await?;
            
            let mut result = Vec::new();
            for account_data in data {
                result.push(account_data.event);
            }
            
            Ok(result)
        })
    }
}