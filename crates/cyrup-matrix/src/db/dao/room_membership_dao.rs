use crate::db::{Dao, RoomMembership, Result};
use serde_json::json;
use std::future::Future;

pub struct RoomMembershipDao {
    dao: Dao<RoomMembership>,
}

// Helper type for a single membership
pub struct MembershipItem {
    membership: Option<RoomMembership>,
}

impl MembershipItem {
    pub fn get(self) -> Option<RoomMembership> {
        self.membership
    }
}

// Helper type for multiple memberships
pub struct MembershipList {
    memberships: Vec<RoomMembership>,
}

impl MembershipList {
    pub fn get_all(self) -> Vec<RoomMembership> {
        self.memberships
    }
    
    pub fn first(self) -> Option<RoomMembership> {
        self.memberships.into_iter().next()
    }
}

impl RoomMembershipDao {
    pub fn new(client: crate::db::client::DatabaseClient) -> Self {
        Self { dao: Dao::new(client) }
    }
    
    // Find all memberships for a room
    pub fn find_by_room(&self, room_id: &str) -> impl Future<Output = Result<MembershipList>> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        
        async move {
            let results = dao.query_with_params(
                "SELECT * FROM room_membership WHERE room_id = $room",
                json!({ "room": room_id })
            ).await?;
            
            Ok(MembershipList { memberships: results })
        }
    }
    
    // Find all memberships for a user
    pub fn find_by_user(&self, user_id: &str) -> impl Future<Output = Result<MembershipList>> {
        let dao = self.dao.clone();
        let user_id = user_id.to_string();
        
        async move {
            let results = dao.query_with_params(
                "SELECT * FROM room_membership WHERE user_id = $user",
                json!({ "user": user_id })
            ).await?;
            
            Ok(MembershipList { memberships: results })
        }
    }
    
    // Find a specific membership by user and room
    pub fn find_by_user_and_room(&self, user_id: &str, room_id: &str) -> impl Future<Output = Result<MembershipItem>> {
        let dao = self.dao.clone();
        let user_id = user_id.to_string();
        let room_id = room_id.to_string();
        
        async move {
            let results = dao.query_with_params(
                "SELECT * FROM room_membership WHERE user_id = $user AND room_id = $room LIMIT 1",
                json!({ "user": user_id, "room": room_id })
            ).await?;
            
            let membership = if results.is_empty() {
                None
            } else {
                Some(results[0].clone())
            };
            
            Ok(MembershipItem { membership })
        }
    }
    
    // Update the status of a membership
    pub fn update_status(&self, id: &str, status: &str) -> impl Future<Output = Result<MembershipItem>> {
        let dao = self.dao.clone();
        let id = id.to_string();
        let status = status.to_string();
        
        async move {
            let result = dao.query_with_params(
                "UPDATE room_membership SET membership_status = $status, updated_at = time::now() WHERE id = $id",
                json!({ "id": id, "status": status })
            ).await?;
            
            Ok(MembershipItem { membership: result })
        }
    }
}
}