use crate::db::entity::RequestDependency;
use crate::db::generic_dao::{Dao, BaseDao};
use crate::future::MatrixFuture;
use serde_json::{json, Value};
use chrono::Utc;

pub struct RequestDependencyDao {
    dao: Dao<RequestDependency>,
}

impl RequestDependencyDao {
    pub fn new(client: crate::db::client::DatabaseClient) -> Self {
        Self { dao: Dao::new(client) }
    }
    
    /// Save a dependent request
    pub fn save_dependent_request(
        &self,
        room_id: &str,
        parent_txn_id: &str, 
        child_txn_id: &str,
        created_at: i64,
        kind: &str,
        content: Value
    ) -> MatrixFuture<()> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        let parent_txn_id = parent_txn_id.to_string();
        let child_txn_id = child_txn_id.to_string();
        let kind = kind.to_string();
        let content = content.clone();
        
        MatrixFuture::spawn(async move {
            // Check if already exists
            let existing: Vec<RequestDependency> = dao.query_with_params::<Vec<RequestDependency>>(
                "SELECT * FROM request_dependency WHERE room_id = $room AND parent_txn_id = $parent AND child_txn_id = $child LIMIT 1",
                json!({ "room": room_id, "parent": parent_txn_id, "child": child_txn_id })
            ).await?;
            
            // Only create if doesn't exist
            if existing.is_empty() {
                let dependency = RequestDependency {
                    id: None,
                    room_id,
                    parent_txn_id,
                    child_txn_id,
                    created_at,
                    kind,
                    content,
                    sent_parent_key: None,
                    updated_at: Utc::now(),
                };
                
                let mut dependency = dependency;
                dao.create(&mut dependency).await?;
            }
            
            Ok(())
        })
    }
    
    /// Get dependent requests
    pub fn get_dependent_requests(&self, room_id: &str, parent_txn_id: &str) -> MatrixFuture<Vec<RequestDependency>> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        let parent_txn_id = parent_txn_id.to_string();
        
        MatrixFuture::spawn(async move {
            let dependencies: Vec<RequestDependency> = dao.query_with_params::<Vec<RequestDependency>>(
                "SELECT * FROM request_dependency WHERE room_id = $room AND parent_txn_id = $parent ORDER BY created_at ASC",
                json!({ "room": room_id, "parent": parent_txn_id })
            ).await?;
                
            Ok(dependencies)
        })
    }
    
    /// Mark dependent requests as ready with parent key
    pub fn mark_dependent_requests_ready(
        &self,
        room_id: &str,
        parent_txn_id: &str,
        sent_parent_key: Value
    ) -> MatrixFuture<usize> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        let parent_txn_id = parent_txn_id.to_string();
        let sent_parent_key = sent_parent_key.clone();
        
        MatrixFuture::spawn(async move {
            let updated: Vec<RequestDependency> = dao.query_with_params::<Vec<RequestDependency>>(
                "UPDATE request_dependency SET sent_parent_key = $key, updated_at = $now WHERE room_id = $room AND parent_txn_id = $parent AND sent_parent_key IS NONE",
                json!({ 
                    "room": room_id, 
                    "parent": parent_txn_id, 
                    "key": sent_parent_key,
                    "now": Utc::now()
                })
            ).await?;
            
            Ok(updated.len())
        })
    }
    
    /// Update dependent request content
    pub fn update_dependent_request(
        &self,
        room_id: &str,
        child_txn_id: &str,
        kind: &str,
        content: Value
    ) -> MatrixFuture<bool> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        let child_txn_id = child_txn_id.to_string();
        let kind = kind.to_string();
        let content = content.clone();
        
        MatrixFuture::spawn(async move {
            let updated: Vec<RequestDependency> = dao.query_with_params::<Vec<RequestDependency>>(
                "UPDATE request_dependency SET kind = $kind, content = $content, updated_at = $now WHERE room_id = $room AND child_txn_id = $child",
                json!({ 
                    "room": room_id, 
                    "child": child_txn_id, 
                    "kind": kind,
                    "content": content,
                    "now": Utc::now()
                })
            ).await?;
            
            Ok(!updated.is_empty())
        })
    }
    
    /// Remove a dependent request
    pub fn remove_dependent_request(
        &self,
        room_id: &str,
        child_txn_id: &str
    ) -> MatrixFuture<bool> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        let child_txn_id = child_txn_id.to_string();
        
        MatrixFuture::spawn(async move {
            let deleted: Vec<RequestDependency> = dao.query_with_params::<Vec<RequestDependency>>(
                "DELETE FROM request_dependency WHERE room_id = $room AND child_txn_id = $child",
                json!({ "room": room_id, "child": child_txn_id })
            ).await?;
            
            Ok(!deleted.is_empty())
        })
    }
    
    /// Get all dependent requests for a room
    pub fn get_all_room_dependent_requests(&self, room_id: &str) -> MatrixFuture<Vec<RequestDependency>> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        
        MatrixFuture::spawn(async move {
            let dependencies: Vec<RequestDependency> = dao.query_with_params::<Vec<RequestDependency>>(
                "SELECT * FROM request_dependency WHERE room_id = $room ORDER BY created_at ASC",
                json!({ "room": room_id })
            ).await?;
                
            Ok(dependencies)
        })
    }
}