use base64::{Engine as _, engine::general_purpose};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use surrealdb::{Surreal, engine::any::Any};

use crate::pagination;
use crate::repository::RepositoryError;

// Type alias for complex tuple types to satisfy clippy::type_complexity
type ThreadSummaryTuple = (Option<Vec<String>>, Option<u64>, Option<u64>);

// TASK14 SUBTASK 9: Add supporting types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub event_id: String,
    pub event_type: String,
    pub content: Value,
    pub sender: String,
    pub origin_server_ts: u64,
    pub unsigned: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateEvent {
    pub event_id: String,
    pub event_type: String,
    pub content: Value,
    pub sender: String,
    pub origin_server_ts: u64,
    pub state_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EphemeralEvent {
    pub event_id: String,
    pub event_type: String,
    pub content: Value,
    pub sender: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountDataEvent {
    pub account_data_type: String,
    pub content: Value,
}

// Re-export PresenceEvent and PresenceState from presence module
pub use crate::repository::presence::{PresenceEvent, PresenceState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filter {
    pub room: Option<RoomFilter>,
    pub presence: Option<PresenceFilter>,
    pub account_data: Option<EventFilter>,
    pub event_format: Option<String>,
    pub event_fields: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomFilter {
    pub not_rooms: Option<Vec<String>>,
    pub rooms: Option<Vec<String>>,
    pub ephemeral: Option<RoomEventFilter>,
    pub include_leave: Option<bool>,
    pub state: Option<StateFilter>,
    pub timeline: Option<RoomEventFilter>,
    pub account_data: Option<RoomEventFilter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEventFilter {
    pub limit: Option<u64>,
    pub not_senders: Option<Vec<String>>,
    pub not_types: Option<Vec<String>>,
    pub senders: Option<Vec<String>>,
    pub types: Option<Vec<String>>,
    pub contains_url: Option<bool>,
    pub lazy_load_members: Option<bool>,
    pub include_redundant_members: Option<bool>,
    pub not_rooms: Option<Vec<String>>,
    pub rooms: Option<Vec<String>>,
    pub unread_thread_notifications: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateFilter {
    pub limit: Option<u64>,
    pub not_senders: Option<Vec<String>>,
    pub not_types: Option<Vec<String>>,
    pub senders: Option<Vec<String>>,
    pub types: Option<Vec<String>>,
    pub lazy_load_members: Option<bool>,
    pub include_redundant_members: Option<bool>,
    pub not_rooms: Option<Vec<String>>,
    pub rooms: Option<Vec<String>>,
    pub contains_url: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceFilter {
    pub limit: Option<u64>,
    pub not_senders: Option<Vec<String>>,
    pub senders: Option<Vec<String>>,
    pub types: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventFilter {
    pub limit: Option<u64>,
    pub not_senders: Option<Vec<String>>,
    pub not_types: Option<Vec<String>>,
    pub senders: Option<Vec<String>>,
    pub types: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitialSyncResponse {
    pub rooms: Vec<RoomSyncData>,
    pub presence: PresenceSyncData,
    pub account_data: AccountDataSync,
    pub next_batch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResponse {
    pub next_batch: String,
    pub rooms: RoomsSyncData,
    pub presence: Option<PresenceSyncData>,
    pub account_data: Option<AccountDataSync>,
    pub to_device: Option<ToDeviceSyncData>,
    pub device_lists: Option<DeviceListSync>,
    pub device_one_time_keys_count: Option<HashMap<String, u64>>,
    pub device_unused_fallback_key_types: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomsSyncData {
    pub join: HashMap<String, JoinedRoomSync>,
    pub invite: HashMap<String, InvitedRoomSync>,
    pub leave: HashMap<String, LeftRoomSync>,
    pub knock: HashMap<String, KnockedRoomSync>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomSyncData {
    pub room_id: String,
    pub state: Vec<Value>,
    pub timeline: TimelineSync,
    pub ephemeral: EphemeralSync,
    pub account_data: Vec<Value>,
    pub unread_notifications: UnreadNotificationCounts,
    pub summary: Option<RoomSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinedRoomSync {
    pub state: StateSync,
    pub timeline: TimelineSync,
    pub ephemeral: EphemeralSync,
    pub account_data: AccountDataSync,
    pub unread_notifications: UnreadNotificationCounts,
    pub summary: Option<RoomSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvitedRoomSync {
    pub invite_state: InviteStateSync,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeftRoomSync {
    pub state: StateSync,
    pub timeline: TimelineSync,
    pub account_data: AccountDataSync,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnockedRoomSync {
    pub knock_state: KnockStateSync,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSync {
    pub events: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineSync {
    pub events: Vec<Value>,
    pub limited: bool,
    pub prev_batch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EphemeralSync {
    pub events: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteStateSync {
    pub events: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnockStateSync {
    pub events: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceSyncData {
    pub events: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountDataSync {
    pub events: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToDeviceSyncData {
    pub events: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceListSync {
    pub changed: Vec<String>,
    pub left: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnreadNotificationCounts {
    pub highlight_count: Option<u64>,
    pub notification_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomSummary {
    pub m_heroes: Option<Vec<String>>,
    pub m_joined_member_count: Option<u64>,
    pub m_invited_member_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPosition {
    pub stream_id: u64,
    pub timestamp: DateTime<Utc>,
    pub room_positions: HashMap<String, u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StreamPosition {
    id: String,
    position: u64,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StreamCounter {
    position: u64,
    updated_at: DateTime<Utc>,
}

pub struct SyncRepository {
    db: Surreal<Any>,
}

impl SyncRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    pub async fn get_initial_sync_data(
        &self,
        user_id: &str,
        filter: Option<&Filter>,
    ) -> Result<InitialSyncResponse, RepositoryError> {
        let rooms = self.get_user_rooms_sync_data(user_id, None, filter).await?;
        let presence = self.get_presence_sync_data(user_id, None).await?;
        let account_data = self.get_account_data_sync(user_id, None).await?;

        let sync_position = SyncPosition {
            stream_id: self.increment_stream_id().await?,
            timestamp: Utc::now(),
            room_positions: HashMap::new(),
        };

        let next_batch = self.create_sync_token(user_id, &sync_position).await?;

        Ok(InitialSyncResponse { rooms, presence, account_data, next_batch })
    }

    pub async fn get_incremental_sync_data(
        &self,
        user_id: &str,
        since: &str,
        filter: Option<&Filter>,
    ) -> Result<SyncResponse, RepositoryError> {
        let since_position = self.parse_sync_token(since).await?;

        let rooms_data = self.get_incremental_rooms_data(user_id, &since_position, filter).await?;
        let presence = Some(
            self.get_presence_sync_data(user_id, Some(&since_position.timestamp))
                .await?,
        );
        let account_data = Some(
            self.get_account_data_sync(user_id, Some(&since_position.timestamp))
                .await?,
        );
        let to_device = Some(self.get_to_device_sync_data(user_id, &since_position).await?);
        let device_lists =
            Some(self.get_device_list_sync(user_id, Some(&since_position.timestamp)).await?);

        let new_position = SyncPosition {
            stream_id: self.increment_stream_id().await?,
            timestamp: Utc::now(),
            room_positions: HashMap::new(),
        };

        let next_batch = self.create_sync_token(user_id, &new_position).await?;

        Ok(SyncResponse {
            next_batch,
            rooms: rooms_data,
            presence,
            account_data,
            to_device,
            device_lists,
            device_one_time_keys_count: Some(HashMap::new()),
            device_unused_fallback_key_types: Some(vec![]),
        })
    }

    pub async fn create_sync_token(
        &self,
        user_id: &str,
        position: &SyncPosition,
    ) -> Result<String, RepositoryError> {
        let token_data = serde_json::to_string(position).map_err(|e| {
            RepositoryError::SerializationError {
                message: format!("Failed to serialize sync position: {}", e),
            }
        })?;

        let token = general_purpose::STANDARD.encode(token_data);

        // Store sync token for potential validation
        let store_query = r#"
            CREATE sync_tokens CONTENT {
                token: $token,
                user_id: $user_id,
                position: $position,
                created_at: time::now(),
                expires_at: time::now() + 24h
            }
        "#;

        self.db
            .query(store_query)
            .bind(("token", token.clone()))
            .bind(("user_id", user_id.to_string()))
            .bind(("position", position.clone()))
            .await
            .map_err(|e| {
                RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "create_sync_token".to_string(),
                }
            })?;

        Ok(token)
    }

    pub async fn parse_sync_token(&self, token: &str) -> Result<SyncPosition, RepositoryError> {
        let decoded = general_purpose::STANDARD.decode(token).map_err(|e| {
            RepositoryError::ValidationError {
                field: "sync_token".to_string(),
                message: format!("Invalid sync_token format: {}", e),
            }
        })?;

        let token_data = String::from_utf8(decoded).map_err(|e| {
            RepositoryError::ValidationError {
                field: "sync_token".to_string(),
                message: format!("Invalid sync_token encoding: {}", e),
            }
        })?;

        serde_json::from_str(&token_data).map_err(|e| {
            RepositoryError::ValidationError {
                field: "sync_token".to_string(),
                message: format!("Invalid sync_token data: {}", e),
            }
        })
    }

    pub async fn get_room_sync_data(
        &self,
        user_id: &str,
        room_id: &str,
        since: Option<&str>,
    ) -> Result<RoomSyncData, RepositoryError> {
        let since_position = if let Some(since_token) = since {
            Some(self.parse_sync_token(since_token).await?)
        } else {
            None
        };

        let state_events = self.get_room_state_events(room_id, since_position.as_ref()).await?;
        let timeline_events =
            self.get_room_timeline_events(room_id, since_position.as_ref()).await?;
        let ephemeral_events = self.get_room_ephemeral_events_internal(room_id, since_position.as_ref()).await?;
        let account_data_events = self
            .get_room_account_data_events(user_id, room_id, since_position.as_ref())
            .await?;

        let unread_notifications = self.get_unread_notification_counts(user_id, room_id).await?;
        let summary = self.get_room_summary(room_id).await?;

        // Convert timeline_events from Vec<Value> to Vec<Event> for pagination
        let events_for_pagination: Vec<matryx_entity::types::Event> = timeline_events
            .iter()
            .filter_map(|v| serde_json::from_value(v.clone()).ok())
            .collect();

        // Generate prev_batch token using pagination helper
        let prev_batch_token = pagination::generate_prev_batch(
            &events_for_pagination,
            room_id,
            20, // limit used in query
        );

        Ok(RoomSyncData {
            room_id: room_id.to_string(),
            state: state_events,
            timeline: TimelineSync {
                events: timeline_events,
                limited: events_for_pagination.len() >= 20,
                prev_batch: prev_batch_token,
            },
            ephemeral: EphemeralSync { events: ephemeral_events },
            account_data: account_data_events,
            unread_notifications,
            summary,
        })
    }

    pub async fn get_presence_sync_data(
        &self,
        user_id: &str,
        since: Option<&DateTime<Utc>>,
    ) -> Result<PresenceSyncData, RepositoryError> {
        let mut query = String::from("SELECT * FROM presence_events WHERE user_id = $user_id");

        let mut db_query = self.db.query(&query).bind(("user_id", user_id.to_string()));

        if let Some(since_time) = since {
            query.push_str(" AND updated_at > $since");
            db_query = db_query.bind(("since", *since_time));
        }

        query.push_str(" ORDER BY updated_at DESC LIMIT 100");

        let mut response = db_query.await.map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_presence_sync_data".to_string(),
            }
        })?;

        let events: Vec<Value> = response.take(0).map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_presence_sync_data_parse".to_string(),
            }
        })?;

        Ok(PresenceSyncData { events })
    }

    pub async fn get_account_data_sync(
        &self,
        user_id: &str,
        since: Option<&DateTime<Utc>>,
    ) -> Result<AccountDataSync, RepositoryError> {
        let mut query = String::from("SELECT * FROM user_account_data WHERE user_id = $user_id");

        let mut db_query = self.db.query(&query).bind(("user_id", user_id.to_string()));

        if let Some(since_time) = since {
            query.push_str(" AND updated_at > $since");
            db_query = db_query.bind(("since", *since_time));
        }

        query.push_str(" ORDER BY updated_at DESC");

        let mut response = db_query.await.map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_account_data_sync".to_string(),
            }
        })?;

        let events: Vec<Value> = response.take(0).map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_account_data_sync_parse".to_string(),
            }
        })?;

        Ok(AccountDataSync { events })
    }

    pub async fn get_device_list_sync(
        &self,
        user_id: &str,
        since: Option<&DateTime<Utc>>,
    ) -> Result<DeviceListSync, RepositoryError> {
        let mut query = String::from(
            "SELECT user_id, device_change_type FROM device_list_updates WHERE user_id = $user_id",
        );

        let mut db_query = self.db.query(&query).bind(("user_id", user_id.to_string()));

        if let Some(since_time) = since {
            query.push_str(" AND updated_at > $since");
            db_query = db_query.bind(("since", *since_time));
        }

        let mut response = db_query.await.map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_device_list_sync".to_string(),
            }
        })?;

        let updates: Vec<(String, String)> = response.take(0).map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_device_list_sync_parse".to_string(),
            }
        })?;

        let mut changed = Vec::new();
        let mut left = Vec::new();

        for (user_id, change_type) in updates {
            match change_type.as_str() {
                "changed" | "new" => changed.push(user_id),
                "left" => left.push(user_id),
                _ => {},
            }
        }

        Ok(DeviceListSync { changed, left })
    }

    // Helper methods

    async fn get_user_rooms_sync_data(
        &self,
        user_id: &str,
        since: Option<&SyncPosition>,
        filter: Option<&Filter>,
    ) -> Result<Vec<RoomSyncData>, RepositoryError> {
        // Build query with optional filters
        let mut query_parts = vec![
            "SELECT room_id FROM room_members",
            "WHERE user_id = $user_id AND membership IN ['join', 'invite', 'leave', 'knock']"
        ];
        
        // Add since filter for incremental sync
        if since.is_some() {
            query_parts.push("AND updated_at > $since_timestamp");
        }
        
        let rooms_query = query_parts.join(" ");

        let mut query_builder = self
            .db
            .query(rooms_query)
            .bind(("user_id", user_id.to_string()));
            
        // Bind since timestamp if provided
        if let Some(sync_pos) = since {
            query_builder = query_builder.bind(("since_timestamp", sync_pos.timestamp));
        }
        
        let mut response = query_builder.await
            .map_err(|e| {
                RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "get_user_rooms_sync_data".to_string(),
                }
            })?;

        let room_ids: Vec<(String,)> = response.take(0).map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_user_rooms_sync_data_parse".to_string(),
            }
        })?;

        let mut rooms = Vec::new();
        for (room_id,) in room_ids {
            // Apply room filter if provided
            if let Some(filter) = filter {
                // Apply room inclusion/exclusion filters
                if let Some(room_filter) = &filter.room {
                    // Skip room if in not_rooms list
                    if let Some(not_rooms) = &room_filter.not_rooms
                        && not_rooms.contains(&room_id)
                    {
                        continue; // Skip this room
                    }
                    
                    // Skip room if rooms list exists and room not in it
                    if let Some(rooms_list) = &room_filter.rooms
                        && !rooms_list.is_empty() && !rooms_list.contains(&room_id)
                    {
                        continue; // Skip this room
                    }
                }
            }
            
            let room_data = self.get_room_sync_data(user_id, &room_id, None).await?;
            rooms.push(room_data);
        }

        Ok(rooms)
    }

    async fn get_incremental_rooms_data(
        &self,
        user_id: &str,
        since: &SyncPosition,
        filter: Option<&Filter>,
    ) -> Result<RoomsSyncData, RepositoryError> {
        // Get rooms where membership has changed since the sync position
        let membership_query = r#"
            SELECT room_id, membership FROM membership 
            WHERE user_id = $user_id AND updated_at > $since_timestamp
            ORDER BY updated_at ASC
        "#;

        let mut response = self
            .db
            .query(membership_query)
            .bind(("user_id", user_id.to_string()))
            .bind(("since_timestamp", since.timestamp))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_incremental_rooms_data".to_string(),
            })?;

        let memberships: Vec<(String, String)> = response.take(0).map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_incremental_rooms_data_parse".to_string(),
            }
        })?;

        let mut join_rooms = HashMap::new();
        let mut invite_rooms = HashMap::new();
        let mut leave_rooms = HashMap::new();
        let mut knock_rooms = HashMap::new();

        for (room_id, membership) in memberships {
            // Apply filter if provided
            if let Some(filter) = filter {
                // Apply room-level filters
                if let Some(room_filter) = &filter.room {
                    // Check room inclusion/exclusion
                    if let Some(not_rooms) = &room_filter.not_rooms
                        && not_rooms.contains(&room_id)
                    {
                        continue; // Skip this room
                    }
                    if let Some(rooms_list) = &room_filter.rooms
                        && !rooms_list.is_empty() && !rooms_list.contains(&room_id)
                    {
                        continue; // Skip this room
                    }
                    
                    // Check include_leave setting
                    if !room_filter.include_leave.unwrap_or(false) && membership == "leave" {
                        continue; // Skip left rooms if not included
                    }
                }
            }

            let room_sync_data = self.get_room_sync_data(user_id, &room_id, Some(&since.timestamp.to_rfc3339())).await?;
            
            match membership.as_str() {
                "join" => { 
                    let joined_room = JoinedRoomSync {
                        state: StateSync { events: room_sync_data.state },
                        timeline: room_sync_data.timeline,
                        ephemeral: room_sync_data.ephemeral,
                        account_data: AccountDataSync { events: room_sync_data.account_data },
                        unread_notifications: room_sync_data.unread_notifications,
                        summary: room_sync_data.summary,
                    };
                    join_rooms.insert(room_id, joined_room);
                },
                "invite" => { 
                    let invited_room = InvitedRoomSync {
                        invite_state: InviteStateSync { events: room_sync_data.state },
                    };
                    invite_rooms.insert(room_id, invited_room);
                },
                "leave" => { 
                    let left_room = LeftRoomSync {
                        state: StateSync { events: room_sync_data.state },
                        timeline: room_sync_data.timeline,
                        account_data: AccountDataSync { events: room_sync_data.account_data },
                    };
                    leave_rooms.insert(room_id, left_room);
                },
                "knock" => { 
                    let knocked_room = KnockedRoomSync {
                        knock_state: KnockStateSync { events: room_sync_data.state },
                    };
                    knock_rooms.insert(room_id, knocked_room);
                },
                _ => {}, // Unknown membership state
            }
        }

        Ok(RoomsSyncData {
            join: join_rooms,
            invite: invite_rooms,
            leave: leave_rooms,
            knock: knock_rooms,
        })
    }

    async fn get_room_state_events(
        &self,
        room_id: &str,
        since: Option<&SyncPosition>,
    ) -> Result<Vec<Value>, RepositoryError> {
        let mut query_parts = vec![
            "SELECT * FROM room_state",
            "WHERE room_id = $room_id"
        ];
        
        // Add since filter for incremental sync
        if since.is_some() {
            query_parts.push("AND origin_server_ts > $since_timestamp");
        }
        
        query_parts.push("ORDER BY origin_server_ts DESC");
        let state_query = query_parts.join(" ");

        let mut query_builder = self
            .db
            .query(state_query)
            .bind(("room_id", room_id.to_string()));
            
        // Bind since timestamp if provided
        if let Some(sync_pos) = since {
            query_builder = query_builder.bind(("since_timestamp", sync_pos.timestamp.to_string()));
        }
        
        let mut response = query_builder.await
            .map_err(|e| {
                RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "get_room_state_events".to_string(),
                }
            })?;

        let events: Vec<Value> = response.take(0).map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_room_state_events_parse".to_string(),
            }
        })?;

        Ok(events)
    }

    async fn get_room_timeline_events(
        &self,
        room_id: &str,
        since: Option<&SyncPosition>,
    ) -> Result<Vec<Value>, RepositoryError> {
        let mut query_parts = vec![
            "SELECT * FROM event",
            "WHERE room_id = $room_id"
        ];
        
        // Add since filter for incremental sync
        if since.is_some() {
            query_parts.push("AND origin_server_ts > $since_timestamp");
        }
        
        query_parts.push("ORDER BY origin_server_ts DESC");
        query_parts.push("LIMIT 20");
        let timeline_query = query_parts.join(" ");

        let mut query_builder = self
            .db
            .query(timeline_query)
            .bind(("room_id", room_id.to_string()));
            
        // Bind since timestamp if provided
        if let Some(sync_pos) = since {
            query_builder = query_builder.bind(("since_timestamp", sync_pos.timestamp.to_string()));
        }
        
        let mut response = query_builder
            .await
            .map_err(|e| {
                RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "get_room_timeline_events".to_string(),
                }
            })?;

        let events: Vec<Value> = response.take(0).map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_room_timeline_events_parse".to_string(),
            }
        })?;

        Ok(events)
    }



    async fn get_room_account_data_events(
        &self,
        user_id: &str,
        room_id: &str,
        since: Option<&SyncPosition>,
    ) -> Result<Vec<Value>, RepositoryError> {
        // Build query with optional since timestamp for incremental sync
        let account_data_query = if since.is_some() {
            r#"
                SELECT * FROM room_account_data 
                WHERE user_id = $user_id AND room_id = $room_id 
                AND updated_at > $since_timestamp
                ORDER BY updated_at DESC
            "#
        } else {
            r#"
                SELECT * FROM room_account_data 
                WHERE user_id = $user_id AND room_id = $room_id 
                ORDER BY updated_at DESC
            "#
        };

        let mut query_builder = self
            .db
            .query(account_data_query)
            .bind(("user_id", user_id.to_string()))
            .bind(("room_id", room_id.to_string()));

        // Add since timestamp if provided for incremental sync
        if let Some(sync_pos) = since {
            query_builder = query_builder.bind(("since_timestamp", sync_pos.timestamp));
        }

        let mut response = query_builder
            .await
            .map_err(|e| {
                RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "get_room_account_data_events".to_string(),
                }
            })?;

        let events: Vec<Value> = response.take(0).map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_room_account_data_events_parse".to_string(),
            }
        })?;

        Ok(events)
    }

    async fn get_to_device_sync_data(
        &self,
        user_id: &str,
        since: &SyncPosition,
    ) -> Result<ToDeviceSyncData, RepositoryError> {
        let to_device_query = r#"
            SELECT * FROM to_device_messages 
            WHERE user_id = $user_id AND created_at > $since
            ORDER BY created_at ASC
        "#;

        let mut response = self
            .db
            .query(to_device_query)
            .bind(("user_id", user_id.to_string()))
            .bind(("since", since.timestamp))
            .await
            .map_err(|e| {
                RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "get_to_device_sync_data".to_string(),
                }
            })?;

        let events: Vec<Value> = response.take(0).map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_to_device_sync_data_parse".to_string(),
            }
        })?;

        Ok(ToDeviceSyncData { events })
    }

    async fn get_unread_notification_counts(
        &self,
        user_id: &str,
        room_id: &str,
    ) -> Result<UnreadNotificationCounts, RepositoryError> {
        let counts_query = r#"
            SELECT 
                SUM(CASE WHEN highlight = true THEN 1 ELSE 0 END) AS highlight_count,
                COUNT(*) AS notification_count
            FROM notifications 
            WHERE user_id = $user_id AND room_id = $room_id AND read = false
        "#;

        let mut response = self
            .db
            .query(counts_query)
            .bind(("user_id", user_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .await
            .map_err(|e| {
                RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "get_unread_notification_counts".to_string(),
                }
            })?;

        let counts: Vec<(Option<u64>, Option<u64>)> = response.take(0).map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_unread_notification_counts_parse".to_string(),
            }
        })?;

        let (highlight_count, notification_count) =
            counts.first().map(|(h, n)| (*h, *n)).unwrap_or((Some(0), Some(0)));

        Ok(UnreadNotificationCounts { highlight_count, notification_count })
    }

    async fn get_room_summary(
        &self,
        room_id: &str,
    ) -> Result<Option<RoomSummary>, RepositoryError> {
        let summary_query = r#"
            SELECT 
                heroes,
                joined_member_count,
                invited_member_count
            FROM room_summaries 
            WHERE room_id = $room_id
        "#;

        let mut response = self
            .db
            .query(summary_query)
            .bind(("room_id", room_id.to_string()))
            .await
            .map_err(|e| {
                RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "get_room_summary".to_string(),
                }
            })?;

        let summaries: Vec<ThreadSummaryTuple> =
            response.take(0).map_err(|e| {
                RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "get_room_summary_parse".to_string(),
                }
            })?;

        if let Some((heroes, joined_count, invited_count)) = summaries.first() {
            Ok(Some(RoomSummary {
                m_heroes: heroes.clone(),
                m_joined_member_count: *joined_count,
                m_invited_member_count: *invited_count,
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_current_stream_id(&self) -> Result<u64, RepositoryError> {
        // Query the current position from the global counter
        let query = "SELECT VALUE position FROM stream_counter:global";
        
        let mut result = self.db
            .query(query)
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_current_stream_id".to_string(),
            })?;
        
        let position: Option<u64> = result
            .take(0)
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_current_stream_id_parse".to_string(),
            })?;
        
        // Return current position, or 0 if counter not initialized yet
        Ok(position.unwrap_or(0))
    }

    /// Atomically increment and return the next stream ID
    async fn increment_stream_id(&self) -> Result<u64, RepositoryError> {
        // Atomically increment the global counter using UPDATE with +=
        // This is safe under concurrent load because UPDATE on a single record is atomic
        let query = "
            UPDATE stream_counter:global 
            SET position += 1, updated_at = time::now() 
            RETURN position;
        ";
        
        let mut result = self.db
            .query(query)
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "increment_stream_id".to_string(),
            })?;
        
        // Parse the result - UPDATE returns array of updated records
        let counter_result: Option<Vec<StreamCounter>> = result
            .take(0)
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "increment_stream_id_parse".to_string(),
            })?;
        
        // Extract position from first (and only) result
        counter_result
            .and_then(|v| v.first().map(|c| c.position))
            .ok_or_else(|| RepositoryError::DatabaseError {
                message: "Failed to increment stream ID - counter not initialized".to_string(),
                operation: "increment_stream_id".to_string(),
            })
    }

    // TASK14 SUBTASK 2: Add missing sync methods
    
    /// Get room member count for joined members
    pub async fn get_room_member_count(&self, room_id: &str) -> Result<u32, RepositoryError> {
        let query = "SELECT count() FROM membership WHERE room_id = $room_id AND membership = 'join' GROUP ALL";
        
        let mut response = self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_room_member_count".to_string(),
            })?;

        let count: Option<i64> = response.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_room_member_count_parse".to_string(),
        })?;

        Ok(count.unwrap_or(0) as u32)
    }

    /// Get room invited member count
    pub async fn get_room_invited_member_count(&self, room_id: &str) -> Result<u32, RepositoryError> {
        let query = "SELECT count() FROM membership WHERE room_id = $room_id AND membership = 'invite' GROUP ALL";
        
        let mut response = self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_room_invited_member_count".to_string(),
            })?;

        let count: Option<i64> = response.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_room_invited_member_count_parse".to_string(),
        })?;

        Ok(count.unwrap_or(0) as u32)
    }

    /// Get room ephemeral events with optional since token
    pub async fn get_room_ephemeral_events(&self, room_id: &str, since: Option<&str>) -> Result<Vec<EphemeralEvent>, RepositoryError> {
        let mut ephemeral_events = Vec::new();

        // Get active typing users from typing_notification table
        let typing_query = "
            SELECT user_id FROM typing_notification
            WHERE room_id = $room_id
            AND expires_at > time::now()
            AND typing = true
            ORDER BY started_at DESC
        ";

        let mut response = self.db
            .query(typing_query)
            .bind(("room_id", room_id.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_room_ephemeral_events_typing".to_string(),
            })?;

        let typing_users: Vec<String> = response.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_room_ephemeral_events_typing_parse".to_string(),
        })?;

        // Construct m.typing event if there are active typers
        if !typing_users.is_empty() {
            let typing_content = json!({
                "user_ids": typing_users
            });

            let typing_event = EphemeralEvent {
                event_id: format!("typing_{}", room_id),
                event_type: "m.typing".to_string(),
                content: typing_content,
                sender: room_id.to_string(), // sender is room for typing events
            };

            ephemeral_events.push(typing_event);
        }

        // Query other ephemeral events from event table (receipts, etc.)
        let mut query_parts = vec![
            "SELECT event_id, event_type, content, sender FROM event",
            "WHERE room_id = $room_id AND event_type LIKE 'm.receipt%'"
        ];

        let mut bindings = vec![("room_id", room_id.to_string())];

        // Add since filter if provided
        if let Some(since_token) = since {
            let since_position = self.parse_sync_token(since_token).await?;
            query_parts.push("AND origin_server_ts > $since_timestamp");
            bindings.push(("since_timestamp", since_position.timestamp.timestamp_millis().to_string()));
        }

        query_parts.push("ORDER BY origin_server_ts DESC LIMIT 10");
        let final_query = query_parts.join(" ");

        let mut query_builder = self.db.query(final_query);
        for (key, value) in bindings {
            query_builder = query_builder.bind((key, value));
        }

        let mut response = query_builder.await.map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_room_ephemeral_events".to_string(),
        })?;

        let events: Vec<(String, String, Value, String)> = response.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_room_ephemeral_events_parse".to_string(),
        })?;

        // Add receipt events to ephemeral_events
        ephemeral_events.extend(events.into_iter().map(|(event_id, event_type, content, sender)| {
            EphemeralEvent {
                event_id,
                event_type,
                content,
                sender,
            }
        }));

        Ok(ephemeral_events)
    }

    // Internal method for compatibility with existing sync logic
    async fn get_room_ephemeral_events_internal(
        &self,
        room_id: &str,
        since: Option<&SyncPosition>,
    ) -> Result<Vec<Value>, RepositoryError> {
        // Build query with optional since timestamp for incremental sync
        let ephemeral_query = if since.is_some() {
            r#"
                SELECT * FROM ephemeral_events 
                WHERE room_id = $room_id AND timestamp > $since_timestamp
                ORDER BY timestamp DESC 
                LIMIT 10
            "#
        } else {
            r#"
                SELECT * FROM ephemeral_events 
                WHERE room_id = $room_id 
                ORDER BY timestamp DESC 
                LIMIT 10
            "#
        };

        let mut query_builder = self
            .db
            .query(ephemeral_query)
            .bind(("room_id", room_id.to_string()));

        // Add since timestamp if provided for incremental sync
        if let Some(sync_pos) = since {
            query_builder = query_builder.bind(("since_timestamp", sync_pos.timestamp));
        }

        let mut response = query_builder
            .await
            .map_err(|e| {
                RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "get_room_ephemeral_events_internal".to_string(),
                }
            })?;

        let events: Vec<Value> = response.take(0).map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_room_ephemeral_events_internal_parse".to_string(),
            }
        })?;

        Ok(events)
    }

    /// Get unread notification counts for a user in a room
    pub async fn get_room_unread_notifications(&self, user_id: &str, room_id: &str) -> Result<UnreadNotificationCounts, RepositoryError> {
        let query = r#"
            SELECT 
                SUM(CASE WHEN highlight = true THEN 1 ELSE 0 END) AS highlight_count,
                COUNT(*) AS notification_count
            FROM notifications 
            WHERE user_id = $user_id AND room_id = $room_id AND read = false
        "#;

        let mut response = self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_room_unread_notifications".to_string(),
            })?;

        let counts: Vec<(Option<u64>, Option<u64>)> = response.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_room_unread_notifications_parse".to_string(),
        })?;

        let (highlight_count, notification_count) = counts.first()
            .map(|(h, n)| (*h, *n))
            .unwrap_or((Some(0), Some(0)));

        Ok(UnreadNotificationCounts {
            highlight_count: Some(highlight_count.unwrap_or(0)),
            notification_count: Some(notification_count.unwrap_or(0)),
        })
    }

    /// Get room heroes (other prominent members for room summary)
    pub async fn get_room_heroes(
        &self,
        room_id: &str,
        current_user_id: &str,
    ) -> Result<Vec<String>, RepositoryError> {
        let query = r#"
            SELECT user_id FROM membership
            WHERE room_id = $room_id
            AND membership = 'join'
            AND user_id != $current_user_id
            ORDER BY updated_at DESC
            LIMIT 5
        "#;

        let mut response = self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("current_user_id", current_user_id.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_room_heroes".to_string(),
            })?;

        #[derive(serde::Deserialize)]
        struct MemberInfo {
            user_id: String,
        }

        let members: Vec<MemberInfo> = response.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_room_heroes_parse".to_string(),
        })?;

        let heroes = members.into_iter().map(|m| m.user_id).collect();
        Ok(heroes)
    }
}

#[cfg(test)]
mod tests {
    include!("sync_tests.rs");
}
