use crate::db::entity::Presence;
use crate::db::generic_dao::{Dao, BaseDao};
use crate::future::MatrixFuture;
use serde_json::{json, Value};
use chrono::Utc;

pub struct PresenceDao {
    dao: Dao<Presence>,
}

impl PresenceDao {
    pub fn new(client: crate::db::client::DatabaseClient) -> Self {
        Self { dao: Dao::new(client) }
    }
    
    /// Get presence event for a user
    pub fn get_presence_event(&self, user_id: &str) -> MatrixFuture<Option<Value>> {
        let dao = self.dao.clone();
        let user_id = user_id.to_string();
        
        MatrixFuture::spawn(async move {
            let presences: Vec<Presence> = dao.query_with_params::<Vec<Presence>>(
                "SELECT * FROM presence WHERE user_id = $user LIMIT 1",
                json!({ "user": user_id })
            ).await?;
            
            if let Some(presence) = presences.first() {
                Ok(Some(presence.event.clone()))
            } else {
                Ok(None)
            }
        })
    }
    
    /// Save presence event
    pub fn save_presence_event(&self, user_id: &str, event: Value) -> MatrixFuture<()> {
        let dao = self.dao.clone();
        let user_id = user_id.to_string();
        let event = event.clone();
        
        MatrixFuture::spawn(async move {
            let now = Utc::now();
            
            // Try to update if exists
            let updated: Vec<Presence> = dao.query_with_params::<Vec<Presence>>(
                "UPDATE presence SET event = $event, updated_at = $now WHERE user_id = $user",
                json!({ "user": user_id, "event": event, "now": now })
            ).await?;
            
            // If not updated, create new
            if updated.is_empty() {
                let presence = Presence {
                    id: None,
                    user_id,
                    event,
                    updated_at: now,
                };
                
                let mut presence = presence;
                dao.create(&mut presence).await?;
            }
            
            Ok(())
        })
    }
}