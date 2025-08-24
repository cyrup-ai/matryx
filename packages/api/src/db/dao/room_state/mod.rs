use crate::db::client::DatabaseClient;
use crate::db::entity::room_state::RoomState;
use crate::db::generic_dao::Dao;
use crate::future::MatrixFuture;
use chrono::Utc;
use matrix_sdk_base::ruma::OwnedRoomId;
use serde_json::{json, Value};

/// RoomState DAO
#[derive(Clone)]
pub struct RoomStateDao {
    dao: Dao<RoomState>,
}

impl RoomStateDao {
    const TABLE_NAME: &'static str = "room_state";

    /// Create a new RoomStateDao
    pub fn new(client: DatabaseClient) -> Self {
        Self {
            dao: Dao::new(client, Self::TABLE_NAME),
        }
    }

    /// Get a state event
    pub fn get_state_event(
        &self,
        room_id: &str,
        event_type: &str,
        state_key: &str,
    ) -> MatrixFuture<Option<Value>> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        let event_type = event_type.to_string();
        let state_key = state_key.to_string();

        MatrixFuture::spawn(async move {
            let states: Vec<RoomState> = dao.query_with_params::<Vec<RoomState>>(
                "SELECT * FROM room_state WHERE room_id = $room AND event_type = $type AND state_key = $key LIMIT 1",
                json!({ "room": room_id, "type": event_type, "key": state_key })
            ).await?;

            if let Some(state) = states.first() {
                Ok(Some(state.event.clone()))
            } else {
                Ok(None)
            }
        })
    }

    /// Get all state events of a specific type for a room
    pub fn get_state_events(
        &self,
        room_id: &str,
        event_type: &str,
    ) -> MatrixFuture<Vec<(String, Value)>> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        let event_type = event_type.to_string();

        MatrixFuture::spawn(async move {
            let states: Vec<RoomState> = dao
                .query_with_params::<Vec<RoomState>>(
                    "SELECT * FROM room_state WHERE room_id = $room AND event_type = $type",
                    json!({ "room": room_id, "type": event_type }),
                )
                .await?;

            let mut result = Vec::new();
            for state in states {
                result.push((state.state_key, state.event));
            }

            Ok(result)
        })
    }

    /// Get all state events for a room
    pub fn get_state_events_for_room(&self, room_id: &str) -> MatrixFuture<Vec<Value>> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();

        MatrixFuture::spawn(async move {
            let states: Vec<RoomState> = dao
                .query_with_params::<Vec<RoomState>>(
                    "SELECT * FROM room_state WHERE room_id = $room",
                    json!({ "room": room_id }),
                )
                .await?;

            let mut result = Vec::new();
            for state in states {
                result.push(state.event);
            }

            Ok(result)
        })
    }

    /// Save a state event
    pub fn save_state_event(
        &self,
        room_id: &str,
        event_type: &str,
        state_key: &str,
        event: Value,
    ) -> MatrixFuture<()> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        let event_type = event_type.to_string();
        let state_key = state_key.to_string();
        let event = event.clone();

        MatrixFuture::spawn(async move {
            // Try to update if exists
            let updated: Vec<RoomState> = dao.query_with_params::<Vec<RoomState>>(
                "UPDATE room_state SET event = $event, updated_at = $now WHERE room_id = $room AND event_type = $type AND state_key = $key",
                json!({ 
                    "room": room_id, 
                    "type": event_type, 
                    "key": state_key,
                    "event": event,
                    "now": Utc::now()
                })
            ).await?;

            // If not updated, create new
            if updated.is_empty() {
                let state = RoomState {
                    id: None,
                    room_id: room_id.to_string(),
                    event_type: event_type.to_string(),
                    state_key: state_key.to_string(),
                    event,
                    updated_at: Utc::now(),
                };

                let mut state = state;
                dao.create(&mut state).await?;
            }

            Ok(())
        })
    }

    /// Remove a room and all associated state
    pub fn remove_room(&self, room_id: &str) -> MatrixFuture<()> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();

        MatrixFuture::spawn(async move {
            dao.query_with_params::<()>(
                "DELETE FROM room_state WHERE room_id = $room",
                json!({ "room": room_id }),
            )
            .await?;

            Ok(())
        })
    }

    /// Get rooms with a specific membership state from room state events
    pub fn get_rooms_by_membership(&self, membership: &str) -> MatrixFuture<Vec<OwnedRoomId>> {
        let dao = self.dao.clone();
        let membership = membership.to_string();

        MatrixFuture::spawn(async move {
            // Get all rooms where there's a membership event for the current user
            // with the specified membership value
            let states: Vec<RoomState> = dao.query_with_params::<Vec<RoomState>>(
                "SELECT DISTINCT room_id FROM room_state WHERE event_type = 'm.room.member' AND event.content.membership = $membership",
                json!({ "membership": membership })
            ).await?;

            let mut result = Vec::new();
            for state in states {
                match OwnedRoomId::try_from(state.room_id) {
                    Ok(room_id) => result.push(room_id),
                    Err(_) => continue, // Skip invalid room IDs
                }
            }

            Ok(result)
        })
    }

    /// Get all users in a room
    pub fn get_users_in_room(&self, room_id: &str) -> MatrixFuture<Vec<String>> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();

        MatrixFuture::spawn(async move {
            // Get all state keys (user IDs) from room membership events for this room
            let states: Vec<RoomState> = dao.query_with_params::<Vec<RoomState>>(
                "SELECT state_key FROM room_state WHERE room_id = $room AND event_type = 'm.room.member' AND event.content.membership IN ['join', 'invite']",
                json!({ "room": room_id })
            ).await?;

            let mut result = Vec::new();
            for state in states {
                result.push(state.state_key);
            }

            Ok(result)
        })
    }

    /// Get users in a room with specific membership states
    pub fn get_room_users_by_membership(
        &self,
        room_id: &str,
        memberships: &[String],
    ) -> MatrixFuture<Vec<String>> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        let memberships = memberships.to_vec();

        MatrixFuture::spawn(async move {
            // Get all state keys (user IDs) from room membership events for this room with specified memberships
            let states: Vec<RoomState> = dao.query_with_params::<Vec<RoomState>>(
                "SELECT state_key FROM room_state WHERE room_id = $room AND event_type = 'm.room.member' AND event.content.membership IN $memberships",
                json!({ "room": room_id, "memberships": memberships })
            ).await?;

            let mut result = Vec::new();
            for state in states {
                result.push(state.state_key);
            }

            Ok(result)
        })
    }

    /// Get all room IDs stored in the database
    pub fn get_all_room_ids(&self) -> MatrixFuture<Vec<String>> {
        let dao = self.dao.clone();

        MatrixFuture::spawn(async move {
            // Get all distinct room IDs from room state events
            let query_result: Vec<Value> =
                dao.query::<Vec<Value>>("SELECT DISTINCT room_id FROM room_state").await?;

            let mut result = Vec::new();
            for value in query_result {
                if let Some(room_id) = value.get("room_id").and_then(|r| r.as_str()) {
                    result.push(room_id.to_string());
                }
            }

            Ok(result)
        })
    }

    /// Get all users with a specific display name in a room
    pub fn get_users_with_display_name(
        &self,
        room_id: &str,
        display_name: &str,
    ) -> MatrixFuture<Vec<String>> {
        let dao = self.dao.clone();
        let room_id = room_id.to_string();
        let display_name = display_name.to_string();

        MatrixFuture::spawn(async move {
            // Get all user IDs from member events where the display name matches
            let states: Vec<RoomState> = dao.query_with_params::<Vec<RoomState>>(
                "SELECT state_key FROM room_state WHERE room_id = $room AND event_type = 'm.room.member' AND event.content.displayname = $display_name AND event.content.membership IN ['join', 'invite']",
                json!({ "room": room_id, "display_name": display_name })
            ).await?;

            let mut result = Vec::new();
            for state in states {
                result.push(state.state_key);
            }

            Ok(result)
        })
    }
}
