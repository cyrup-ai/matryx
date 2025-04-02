use crate::db::entity::SendQueueRequest;
use crate::db::generic_dao::{Dao, BaseDao};
use crate::db::error::Error;
use crate::future::MatrixFuture;
use serde_json::{json, Value};
use chrono::Utc;
use std::collections::HashMap;

pub struct SendQueueDao {
    dao: Dao<SendQueueRequest>,
}

impl SendQueueDao {
    pub fn new(client: crate::db::client::DatabaseClient) -> Self {
        Self { dao: Dao::new(client) }
    }
    
    /// Save a request to the send queue
    pub fn save_queue_request(
        &self,
        room_id: &str, 
        transaction_id: &str,
        created_at: i64,
        kind: &str,
        content: Value,
        priority: usize
    ) -> MatrixFuture<()> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        let transaction_id = transaction_id.to_string();
        let kind = kind.to_string();
        let content = content.clone();
        
        MatrixFuture::spawn(async move {
            // Try to update if exists
            let updated: Vec<SendQueueRequest> = dao.query_with_params::<Vec<SendQueueRequest>>(
                "UPDATE send_queue_request SET kind = $kind, content = $content, updated_at = $now WHERE room_id = $room AND transaction_id = $txn",
                json!({ 
                    "room": room_id,
                    "txn": transaction_id,
                    "kind": kind,
                    "content": content,
                    "now": Utc::now() 
                })
            ).await?;
            
            // If not updated, create new
            if updated.is_empty() {
                let send_request = SendQueueRequest {
                    id: None,
                    room_id,
                    transaction_id,
                    created_at,
                    kind,
                    content,
                    priority,
                    error: None,
                    updated_at: Utc::now(),
                };
                
                let mut send_request = send_request;
                dao.create(&mut send_request).await?;
            }
            
            Ok(())
        })
    }
    
    /// Save a request with a pre-serialized value
    pub fn save_request(&self, request_id: &str, request_data: Value) -> MatrixFuture<()> {
        let dao = self.dao.clone();
        let request_id = request_id.to_string();
        let request_data = request_data.clone();
        
        MatrixFuture::spawn(async move {
            // Extract room_id from the request data
            let room_id = match request_data.get("room_id") {
                Some(val) => val.as_str().unwrap_or("").to_string(),
                None => "".to_string()
            };
            
            // Try to update if exists
            let updated: Vec<SendQueueRequest> = dao.query_with_params::<Vec<SendQueueRequest>>(
                "UPDATE send_queue_request SET transaction_id = $txn, updated_at = $now WHERE transaction_id = $txn",
                json!({ 
                    "txn": request_id,
                    "now": Utc::now() 
                })
            ).await?;
            
            // If not updated, create new from the provided data
            if updated.is_empty() {
                let created_at = match request_data.get("created_at") {
                    Some(val) => val.as_i64().unwrap_or(Utc::now().timestamp_millis()),
                    None => Utc::now().timestamp_millis()
                };
                
                let kind = match request_data.get("kind") {
                    Some(val) => val.as_str().unwrap_or("").to_string(),
                    None => "".to_string()
                };
                
                let content = match request_data.get("content") {
                    Some(val) => val.clone(),
                    None => json!({})
                };
                
                let priority = match request_data.get("priority") {
                    Some(val) => val.as_u64().unwrap_or(0) as usize,
                    None => 0
                };
                
                let error = request_data.get("error").map(|e| e.clone());
                
                let send_request = SendQueueRequest {
                    id: None,
                    room_id,
                    transaction_id: request_id,
                    created_at,
                    kind,
                    content,
                    priority,
                    error,
                    updated_at: Utc::now(),
                };
                
                let mut send_request = send_request;
                dao.create(&mut send_request).await?;
            }
            
            Ok(())
        })
    }
    
    /// Get a specific request by ID
    pub fn get_request(&self, request_id: &str) -> MatrixFuture<Option<Value>> {
        let dao = self.dao.clone();
        let request_id = request_id.to_string();
        
        MatrixFuture::spawn(async move {
            let requests: Vec<SendQueueRequest> = dao.query_with_params::<Vec<SendQueueRequest>>(
                "SELECT * FROM send_queue_request WHERE transaction_id = $txn",
                json!({ "txn": request_id })
            ).await?;
            
            if let Some(request) = requests.first() {
                Ok(Some(serde_json::to_value(request).map_err(|e| Error::Serialization(e))?))
            } else {
                Ok(None)
            }
        })
    }
    
    /// Remove a request using only the request ID
    pub fn remove_request(&self, request_id: &str) -> MatrixFuture<()> {
        let dao = self.dao.clone();
        let request_id = request_id.to_string();
        
        MatrixFuture::spawn(async move {
            dao.query_with_params::<()>(
                "DELETE FROM send_queue_request WHERE transaction_id = $txn",
                json!({ "txn": request_id })
            ).await?;
            
            Ok(())
        })
    }
    
    /// Get all requests in the queue
    pub fn get_all_requests(&self) -> MatrixFuture<HashMap<String, Value>> {
        let dao = self.dao.clone();
        
        MatrixFuture::spawn(async move {
            let requests: Vec<SendQueueRequest> = dao.query::<Vec<SendQueueRequest>>(
                "SELECT * FROM send_queue_request ORDER BY priority DESC, created_at ASC"
            ).await?;
            
            let mut result = HashMap::new();
            for request in requests {
                let transaction_id = request.transaction_id.clone();
                let value = serde_json::to_value(&request).map_err(|e| Error::Serialization(e))?;
                result.insert(transaction_id, value);
            }
            
            Ok(result)
        })
    }
    
    /// Update a request in the send queue
    pub fn update_queue_request(
        &self,
        room_id: &str, 
        transaction_id: &str,
        kind: &str,
        content: Value
    ) -> MatrixFuture<bool> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        let transaction_id = transaction_id.to_string();
        let kind = kind.to_string();
        let content = content.clone();
        
        MatrixFuture::spawn(async move {
            // Update if exists
            let updated: Vec<SendQueueRequest> = dao.query_with_params::<Vec<SendQueueRequest>>(
                "UPDATE send_queue_request SET kind = $kind, content = $content, error = NULL, updated_at = $now WHERE room_id = $room AND transaction_id = $txn",
                json!({ 
                    "room": room_id,
                    "txn": transaction_id,
                    "kind": kind,
                    "content": content,
                    "now": Utc::now() 
                })
            ).await?;
            
            Ok(!updated.is_empty())
        })
    }

    /// Update request error status
    pub fn update_request_status(
        &self,
        room_id: &str, 
        transaction_id: &str,
        error: Option<Value>
    ) -> MatrixFuture<()> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        let transaction_id = transaction_id.to_string();
        
        MatrixFuture::spawn(async move {
            dao.query_with_params::<()>(
                "UPDATE send_queue_request SET error = $error, updated_at = $now WHERE room_id = $room AND transaction_id = $txn",
                json!({ 
                    "room": room_id,
                    "txn": transaction_id,
                    "error": error,
                    "now": Utc::now() 
                })
            ).await?;
            
            Ok(())
        })
    }
    
    /// Remove a request from the send queue
    pub fn remove_queue_request(&self, room_id: &str, transaction_id: &str) -> MatrixFuture<bool> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        let transaction_id = transaction_id.to_string();
        
        MatrixFuture::spawn(async move {
            let deleted: Vec<SendQueueRequest> = dao.query_with_params::<Vec<SendQueueRequest>>(
                "DELETE FROM send_queue_request WHERE room_id = $room AND transaction_id = $txn",
                json!({ "room": room_id, "txn": transaction_id })
            ).await?;
            
            Ok(!deleted.is_empty())
        })
    }
    
    /// Get all requests for a room in the send queue
    pub fn get_room_requests(&self, room_id: &str) -> MatrixFuture<HashMap<String, Value>> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        
        MatrixFuture::spawn(async move {
            let requests: Vec<SendQueueRequest> = dao.query_with_params::<Vec<SendQueueRequest>>(
                "SELECT * FROM send_queue_request WHERE room_id = $room ORDER BY priority DESC, created_at ASC",
                json!({ "room": room_id })
            ).await?;
            
            let mut result = HashMap::new();
            for request in requests {
                let transaction_id = request.transaction_id.clone();
                let value = serde_json::to_value(&request).map_err(|e| Error::Serialization(e))?;
                result.insert(transaction_id, value);
            }
            
            Ok(result)
        })
    }
    
    /// Get all rooms with pending requests
    pub fn get_rooms_with_requests(&self) -> MatrixFuture<Vec<String>> {
        let dao = self.dao.clone();
        
        MatrixFuture::spawn(async move {
            let result: Vec<Value> = dao.query::<Vec<Value>>(
                "SELECT DISTINCT room_id FROM send_queue_request"
            ).await?;
            
            let mut room_ids = Vec::new();
            for value in result {
                if let Some(room_id) = value.get("room_id").and_then(|r| r.as_str()) {
                    room_ids.push(room_id.to_string());
                }
            }
            
            Ok(room_ids)
        })
    }
}