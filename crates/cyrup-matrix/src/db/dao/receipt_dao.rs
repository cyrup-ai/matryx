use crate::db::entity::Receipt;
use crate::db::generic_dao::{Dao, BaseDao};
use crate::future::MatrixFuture;
use serde_json::{json, Value};
use chrono::Utc;

pub struct ReceiptDao {
    dao: Dao<Receipt>,
}

impl ReceiptDao {
    pub fn new(client: crate::db::client::DatabaseClient) -> Self {
        Self { dao: Dao::new(client) }
    }
    
    /// Get all receipts for a specific user in a room
    pub fn get_user_receipts(&self, room_id: &str, receipt_type: &str, thread: &str, user_id: &str) -> MatrixFuture<Option<(String, Value)>> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        let receipt_type = receipt_type.to_string();
        let thread = thread.to_string();
        let user_id = user_id.to_string();
        
        MatrixFuture::spawn(async move {
            let results: Vec<Receipt> = dao.query_with_params::<Vec<Receipt>>(
                "SELECT * FROM receipts WHERE room_id = $room_id AND receipt_type = $receipt_type AND thread = $thread AND user_id = $user_id",
                json!({
                    "room_id": room_id,
                    "receipt_type": receipt_type,
                    "thread": thread,
                    "user_id": user_id,
                }),
            ).await?;
            
            if let Some(receipt) = results.into_iter().next() {
                Ok(Some((receipt.event_id, receipt.receipt_data)))
            } else {
                Ok(None)
            }
        })
    }
    
    /// Get all receipts for a specific event in a room
    pub fn get_event_receipts(&self, room_id: &str, receipt_type: &str, thread: &str, event_id: &str) -> MatrixFuture<Vec<(String, Value)>> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        let receipt_type = receipt_type.to_string();
        let thread = thread.to_string();
        let event_id = event_id.to_string();
        
        MatrixFuture::spawn(async move {
            let results: Vec<Receipt> = dao.query_with_params::<Vec<Receipt>>(
                "SELECT * FROM receipts WHERE room_id = $room_id AND receipt_type = $receipt_type AND thread = $thread AND event_id = $event_id",
                json!({
                    "room_id": room_id,
                    "receipt_type": receipt_type,
                    "thread": thread,
                    "event_id": event_id,
                }),
            ).await?;
            
            Ok(results.into_iter().map(|r| (r.user_id, r.receipt_data)).collect())
        })
    }
    
    /// Set a receipt for a specific event, room, user
    pub fn set_receipt(&self, room_id: &str, receipt_type: &str, thread: &str, event_id: &str, user_id: &str, receipt_data: Value) -> MatrixFuture<()> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        let receipt_type = receipt_type.to_string();
        let thread = thread.to_string();
        let event_id = event_id.to_string();
        let user_id = user_id.to_string();
        
        MatrixFuture::spawn(async move {
            // First check if a receipt already exists for this user
            let existing: Vec<Receipt> = dao.query_with_params::<Vec<Receipt>>(
                "SELECT * FROM receipts WHERE room_id = $room_id AND receipt_type = $receipt_type AND thread = $thread AND user_id = $user_id",
                json!({
                    "room_id": room_id,
                    "receipt_type": receipt_type,
                    "thread": thread,
                    "user_id": user_id,
                }),
            ).await?;
            
            if let Some(mut existing) = existing.into_iter().next() {
                // Update existing receipt
                existing.event_id = event_id;
                existing.receipt_data = receipt_data;
                existing.updated_at = Utc::now();
                dao.create(&mut existing).await?;
            } else {
                // Create new receipt
                let mut receipt = Receipt {
                    id: None,
                    room_id,
                    receipt_type,
                    thread,
                    event_id,
                    user_id,
                    receipt_data,
                    updated_at: Utc::now(),
                };
                
                dao.create(&mut receipt).await?;
            }
            
            Ok(())
        })
    }
    
    /// Remove all receipts for a room
    pub fn remove_room_receipts(&self, room_id: &str) -> MatrixFuture<()> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        
        MatrixFuture::spawn(async move {
            dao.query_with_params::<()>(
                "DELETE FROM receipts WHERE room_id = $room_id",
                json!({"room_id": room_id}),
            ).await?;
            
            Ok(())
        })
    }
}