use crate::db::{Dao, RoomMembership, Result};
use serde_json::json;

pub struct RoomMembershipDao {
    dao: Dao<RoomMembership>,
}

impl RoomMembershipDao {
    pub fn new() -> Self {
        Self { dao: Dao::new() }
    }
    
    pub async fn find_by_room(&self, room_id: &str) -> Result<Vec<RoomMembership>> {
        self.dao.query_with_params(
            "SELECT * FROM room_membership WHERE room_id = $room",
            json!({ "room": room_id })
        ).await
    }
    
    pub async fn find_by_user(&self, user_id: &str) -> Result<Vec<RoomMembership>> {
        self.dao.query_with_params(
            "SELECT * FROM room_membership WHERE user_id = $user",
            json!({ "user": user_id })
        ).await
    }
    
    pub async fn find_by_user_and_room(&self, user_id: &str, room_id: &str) -> Result<Option<RoomMembership>> {
        let results = self.dao.query_with_params(
            "SELECT * FROM room_membership WHERE user_id = $user AND room_id = $room LIMIT 1",
            json!({ "user": user_id, "room": room_id })
        ).await?;
        
        if results.is_empty() {
            Ok(None)
        } else {
            Ok(Some(results[0].clone()))
        }
    }
    
    pub async fn update_status(&self, id: &str, status: &str) -> Result<Option<RoomMembership>> {
        self.dao.query_with_params(
            "UPDATE room_membership SET membership_status = $status, updated_at = time::now() WHERE id = $id",
            json!({ "id": id, "status": status })
        ).await
    }
}