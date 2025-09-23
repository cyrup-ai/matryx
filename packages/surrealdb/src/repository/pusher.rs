use crate::repository::error::RepositoryError;
use matryx_entity::Pusher;
use serde_json::Value;
use std::collections::HashMap;
use surrealdb::{Connection, Surreal};

#[derive(Debug, Clone)]
pub struct RoomMember {
    pub user_id: String,
    pub display_name: Option<String>,
    pub power_level: i64,
}

#[derive(Clone)]
pub struct PusherRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> PusherRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    /// Get room members for push notifications
    pub async fn get_room_members_for_push(
        &self,
        room_id: &str,
    ) -> Result<Vec<RoomMember>, RepositoryError> {
        let query = "
            SELECT user_id, content.displayname as display_name, content.membership
            FROM room_memberships
            WHERE room_id = $room_id AND content.membership = 'join'
        ";

        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;

        let members: Vec<Value> = result.take(0)?;

        let room_members = members
            .into_iter()
            .filter_map(|member| {
                let user_id = member.get("user_id")?.as_str()?.to_string();
                let display_name =
                    member.get("display_name").and_then(|v| v.as_str()).map(|s| s.to_string());

                Some(RoomMember {
                    user_id,
                    display_name,
                    power_level: 0, // Default power level
                })
            })
            .collect();

        Ok(room_members)
    }

    /// Get room power levels
    pub async fn get_room_power_levels(
        &self,
        room_id: &str,
    ) -> Result<HashMap<String, i64>, RepositoryError> {
        let query = "
            SELECT content.users
            FROM room_state_events
            WHERE room_id = $room_id AND type = 'm.room.power_levels' AND state_key = ''
            ORDER BY origin_server_ts DESC
            LIMIT 1
        ";

        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;

        let power_level_events: Vec<Value> = result.take(0)?;

        let mut power_levels = HashMap::new();

        if let Some(event) = power_level_events.first() &&
            let Some(users) = event.get("users").and_then(|u| u.as_object())
        {
            for (user_id, level) in users {
                if let Some(level_num) = level.as_i64() {
                    power_levels.insert(user_id.clone(), level_num);
                }
            }
        }

        Ok(power_levels)
    }

    /// Get user pushers
    pub async fn get_user_pushers(&self, user_id: &str) -> Result<Vec<Pusher>, RepositoryError> {
        let query = "
            SELECT * FROM pushers
            WHERE user_id = $user_id AND kind = 'http'
        ";

        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;

        let pusher_records: Vec<Value> = result.take(0)?;

        let pushers = pusher_records
            .into_iter()
            .filter_map(|record| serde_json::from_value(record).ok())
            .collect();

        Ok(pushers)
    }

    /// Get room name
    pub async fn get_room_name(&self, room_id: &str) -> Result<String, RepositoryError> {
        let query = "
            SELECT content.name
            FROM room_state_events
            WHERE room_id = $room_id AND type = 'm.room.name' AND state_key = ''
            ORDER BY origin_server_ts DESC
            LIMIT 1
        ";

        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;

        let name_events: Vec<Value> = result.take(0)?;

        if let Some(event) = name_events.first() &&
            let Some(name) = event.get("name").and_then(|n| n.as_str())
        {
            return Ok(name.to_string());
        }

        Ok(format!("Room {}", room_id))
    }

    /// Get user display name
    pub async fn get_user_display_name(
        &self,
        user_id: &str,
    ) -> Result<Option<String>, RepositoryError> {
        let query = "
            SELECT content.displayname
            FROM user_profiles
            WHERE user_id = $user_id
            LIMIT 1
        ";

        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;

        let profile_records: Vec<Value> = result.take(0)?;

        if let Some(profile) = profile_records.first() &&
            let Some(display_name) = profile.get("displayname").and_then(|n| n.as_str())
        {
            return Ok(Some(display_name.to_string()));
        }

        Ok(None)
    }
}
