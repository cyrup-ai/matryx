use crate::db::Entity;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomMembership {
    pub id: Option<String>,
    pub user_id: String,
    pub room_id: String,
    pub display_name: Option<String>,
    pub membership_status: String,
    pub joined_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Entity for RoomMembership {
    fn table_name() -> &'static str {
        "room_membership"
    }

    fn id(&self) -> Option<String> {
        self.id.clone()
    }

    fn set_id(&mut self, id: String) {
        self.id = Some(id);
    }
}
