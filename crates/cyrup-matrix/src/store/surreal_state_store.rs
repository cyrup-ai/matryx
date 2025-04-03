// Copyright 2025 Cyrum Project.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use hex;
use matrix_sdk_base::{
    deserialized_responses::{DisplayName, RawAnySyncOrStrippedState},
    // Using Matrix SDK Base ruma re-exports to ensure type compatibility
    ruma::{
        events::{
            presence::PresenceEvent,
            receipt::{Receipt, ReceiptThread, ReceiptType},
            room::member::MembershipState,
            AnyGlobalAccountDataEvent,
            AnyRoomAccountDataEvent,
            GlobalAccountDataEventType,
            RoomAccountDataEventType,
            StateEventType,
        },
        serde::Raw,
        EventId,
        MilliSecondsSinceUnixEpoch,
        OwnedEventId,
        OwnedMxcUri,
        OwnedRoomId,
        OwnedTransactionId,
        OwnedUserId,
        RoomId,
        TransactionId,
        UserId,
    },
    store::{
        ChildTransactionId,
        DependentQueuedRequest,
        DependentQueuedRequestKind,
        QueueWedgeError,
        QueuedRequest,
        QueuedRequestKind,
        SentRequestKey,
        StateChanges,
        StateStore,
        StateStoreDataKey,
        StateStoreDataValue,
    },
    MinimalRoomMemberEvent,
    RoomInfo,
    RoomMemberships,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, error, info, instrument, trace, warn};

use crate::error::StoreError as CyrumStoreError;
use crate::store::cyrum_state_store::{
    AccountDataFuture,
    CustomValueFuture,
    CyrumStateStore,
    DependentQueueFuture,
    DependentQueuedRequestStream,
    DisplayNameFuture,
    KeyValueFuture,
    MediaUploadFuture,
    MediaUploadStream,
    PresenceFuture,
    PresenceStream,
    ProfileFuture,
    QueuedRequestStream,
    ReceiptFuture,
    ReceiptStream,
    RoomFuture,
    RoomIdStream,
    RoomInfoStream,
    SendQueueFuture,
    StateChangesFuture,
    StateEventFuture,
    StateEventStream,
    UserIdStream,
};

/// Represents a key-value entry in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
struct KeyValueEntry {
    /// The key for this entry
    pub key: String,
    /// JSON serialized value
    pub value: Value,
    /// Type of the value (used for deserialization)
    pub value_type: String,
}

/// SurrealStateStore implementation for Matrix SDK state store backed by SurrealDB
/// We implement Debug manually since DAOs don't implement Debug
pub struct SurrealStateStore {
    // Database client for SurrealDB operations
    client: crate::db::DatabaseClient,
    // Room state DAO for state events
    room_state_dao: crate::db::RoomStateDao,
    // Account data DAO
    account_data_dao: crate::db::AccountDataDao,
    // Presence DAO
    presence_dao: crate::db::PresenceDao,
    // Receipt DAO
    receipt_dao: crate::db::ReceiptDao,
    // Send queue DAO
    send_queue_dao: crate::db::SendQueueDao,
    // Request dependency DAO
    request_dependency_dao: crate::db::RequestDependencyDao,
    // Media upload DAO
    media_upload_dao: crate::db::MediaUploadDao,
    // KV storage DAO
    kv_dao: crate::db::dao::key_value::KeyValueDao,
    // Custom value DAO
    custom_dao: crate::db::dao::custom::CustomDao,
}

// Manual Debug implementation to avoid requiring Debug for all DAOs
impl std::fmt::Debug for SurrealStateStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SurrealStateStore")
            .field("client", &"DatabaseClient")
            .field("room_state_dao", &"RoomStateDao")
            .field("account_data_dao", &"AccountDataDao")
            .field("presence_dao", &"PresenceDao")
            .field("receipt_dao", &"ReceiptDao")
            .field("send_queue_dao", &"SendQueueDao")
            .field("request_dependency_dao", &"RequestDependencyDao")
            .field("media_upload_dao", &"MediaUploadDao")
            .field("kv_dao", &"KeyValueDao")
            .field("custom_dao", &"CustomDao")
            .finish()
    }
}

impl SurrealStateStore {
    /// Create a new SurrealStateStore with the given database client
    pub fn new(client: crate::db::DatabaseClient) -> Self {
        // Create all required DAOs
        let room_state_dao = crate::db::RoomStateDao::new(client.clone());
        let account_data_dao = crate::db::AccountDataDao::new(client.clone());
        let presence_dao = crate::db::PresenceDao::new(client.clone());
        let receipt_dao = crate::db::ReceiptDao::new(client.clone());
        let send_queue_dao = crate::db::SendQueueDao::new(client.clone());
        let request_dependency_dao = crate::db::RequestDependencyDao::new(client.clone());
        let media_upload_dao = crate::db::MediaUploadDao::new(client.clone());
        let kv_dao = crate::db::dao::key_value::KeyValueDao::new(client.clone());
        let custom_dao = crate::db::dao::custom::CustomDao::new(client.clone());

        Self {
            client,
            room_state_dao,
            account_data_dao,
            presence_dao,
            receipt_dao,
            send_queue_dao,
            request_dependency_dao,
            media_upload_dao,
            kv_dao,
            custom_dao,
        }
    }

    /// Convert a StateStoreDataKey to a string key for database storage
    fn data_key_to_string(key: StateStoreDataKey<'_>) -> String {
        match key {
            StateStoreDataKey::SyncToken => StateStoreDataKey::SYNC_TOKEN.to_string(),
            StateStoreDataKey::ServerCapabilities => {
                StateStoreDataKey::SERVER_CAPABILITIES.to_string()
            },
            StateStoreDataKey::Filter(name) => format!("{}:{}", StateStoreDataKey::FILTER, name),
            StateStoreDataKey::UserAvatarUrl(user_id) => {
                format!("{}:{}", StateStoreDataKey::USER_AVATAR_URL, user_id)
            },
            StateStoreDataKey::RecentlyVisitedRooms(user_id) => {
                format!("{}:{}", StateStoreDataKey::RECENTLY_VISITED_ROOMS, user_id)
            },
            StateStoreDataKey::UtdHookManagerData => {
                StateStoreDataKey::UTD_HOOK_MANAGER_DATA.to_string()
            },
            StateStoreDataKey::ComposerDraft(room_id) => {
                format!("{}:{}", StateStoreDataKey::COMPOSER_DRAFT, room_id)
            },
            StateStoreDataKey::SeenKnockRequests(room_id) => {
                format!("{}:{}", StateStoreDataKey::SEEN_KNOCK_REQUESTS, room_id)
            },
        }
    }

    /// Get the type name for a StateStoreDataValue for serialization
    fn get_value_type(value: &StateStoreDataValue) -> &'static str {
        match value {
            StateStoreDataValue::SyncToken(_) => "sync_token",
            StateStoreDataValue::ServerCapabilities(_) => "server_capabilities",
            StateStoreDataValue::Filter(_) => "filter",
            StateStoreDataValue::UserAvatarUrl(_) => "user_avatar_url",
            StateStoreDataValue::RecentlyVisitedRooms(_) => "recently_visited_rooms",
            StateStoreDataValue::UtdHookManagerData(_) => "utd_hook_manager_data",
            StateStoreDataValue::ComposerDraft(_) => "composer_draft",
            StateStoreDataValue::SeenKnockRequests(_) => "seen_knock_requests",
        }
    }
}

// Add implementation of StateStore trait for SurrealStateStore
#[async_trait::async_trait]
impl matrix_sdk_base::store::StateStore for SurrealStateStore {
    type Error = matrix_sdk_base::store::StoreError;

    #[instrument(skip(self, key), level = "debug")]
    async fn get_kv_data(
        &self,
        key: matrix_sdk_base::StateStoreDataKey<'_>,
    ) -> matrix_sdk_base::store::Result<Option<matrix_sdk_base::StateStoreDataValue>> {
        let key_str = Self::data_key_to_string(key);
        trace!(key = %key_str, "Getting KV data");

        match self.kv_dao.get_value(&key_str).await {
            Ok(Some(entry)) => {
                // Deserialize based on value type
                let result = match entry.value_type.as_str() {
                    "sync_token" => {
                        let token: String = serde_json::from_value(entry.value)
                            .map_err(|e| matrix_sdk_base::store::StoreError::Json(e))?;
                        Some(StateStoreDataValue::SyncToken(token))
                    },
                    "server_capabilities" => {
                        let caps = serde_json::from_value(entry.value)
                            .map_err(|e| matrix_sdk_base::store::StoreError::Json(e))?;
                        Some(StateStoreDataValue::ServerCapabilities(caps))
                    },
                    "filter" => {
                        let filter: String = serde_json::from_value(entry.value)
                            .map_err(|e| matrix_sdk_base::store::StoreError::Json(e))?;
                        Some(StateStoreDataValue::Filter(filter))
                    },
                    "user_avatar_url" => {
                        let url: String = serde_json::from_value(entry.value)
                            .map_err(|e| matrix_sdk_base::store::StoreError::Json(e))?;
                        let mxc = OwnedMxcUri::try_from(url).map_err(|e| {
                            matrix_sdk_base::store::StoreError::Backend(Box::new(e))
                        })?;
                        Some(StateStoreDataValue::UserAvatarUrl(mxc))
                    },
                    "recently_visited_rooms" => {
                        let rooms: Vec<String> = serde_json::from_value(entry.value)
                            .map_err(|e| matrix_sdk_base::store::StoreError::Json(e))?;
                        let room_ids = rooms
                            .into_iter()
                            .filter_map(|r| OwnedRoomId::try_from(r).ok())
                            .collect();
                        Some(StateStoreDataValue::RecentlyVisitedRooms(room_ids))
                    },
                    "utd_hook_manager_data" => {
                        let data = serde_json::from_value(entry.value)
                            .map_err(|e| matrix_sdk_base::store::StoreError::Json(e))?;
                        Some(StateStoreDataValue::UtdHookManagerData(data))
                    },
                    "composer_draft" => {
                        let draft = serde_json::from_value(entry.value)
                            .map_err(|e| matrix_sdk_base::store::StoreError::Json(e))?;
                        Some(StateStoreDataValue::ComposerDraft(draft))
                    },
                    "seen_knock_requests" => {
                        let map: BTreeMap<String, String> = serde_json::from_value(entry.value)
                            .map_err(|e| matrix_sdk_base::store::StoreError::Json(e))?;

                        let mut result = BTreeMap::new();
                        for (event_id_str, user_id_str) in map {
                            match (
                                OwnedEventId::try_from(event_id_str),
                                OwnedUserId::try_from(user_id_str),
                            ) {
                                (Ok(event_id), Ok(user_id)) => {
                                    result.insert(event_id, user_id);
                                },
                                _ => {
                                    warn!("Invalid event_id or user_id in seen_knock_requests");
                                    continue;
                                },
                            }
                        }
                        Some(StateStoreDataValue::SeenKnockRequests(result))
                    },
                    unknown => {
                        error!(value_type = %unknown, "Unknown value type in KV storage");
                        return Err(matrix_sdk_base::store::StoreError::Backend(Box::new(
                            std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                format!("Unknown value type: {}", unknown),
                            ),
                        )));
                    },
                };

                trace!(key = %key_str, "Retrieved KV data successfully");
                Ok(result)
            },
            Ok(None) => {
                trace!(key = %key_str, "No KV data found");
                Ok(None)
            },
            Err(err) => {
                error!(key = %key_str, error = %err, "Failed to get KV data");
                Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err)))
            },
        }
    }

    #[instrument(skip(self, key, value), level = "debug")]
    async fn set_kv_data(
        &self,
        key: matrix_sdk_base::StateStoreDataKey<'_>,
        value: matrix_sdk_base::StateStoreDataValue,
    ) -> matrix_sdk_base::store::Result<()> {
        let key_str = Self::data_key_to_string(key);
        let value_type = Self::get_value_type(&value);
        trace!(key = %key_str, value_type = %value_type, "Setting KV data");

        // Serialize the value based on its type
        let json_value = match &value {
            StateStoreDataValue::SyncToken(token) => {
                serde_json::to_value(token).map_err(matrix_sdk_base::store::StoreError::Json)?
            },
            StateStoreDataValue::ServerCapabilities(caps) => {
                serde_json::to_value(caps).map_err(matrix_sdk_base::store::StoreError::Json)?
            },
            StateStoreDataValue::Filter(filter) => {
                serde_json::to_value(filter).map_err(matrix_sdk_base::store::StoreError::Json)?
            },
            StateStoreDataValue::UserAvatarUrl(url) => {
                serde_json::to_value(url.as_str())
                    .map_err(matrix_sdk_base::store::StoreError::Json)?
            },
            StateStoreDataValue::RecentlyVisitedRooms(rooms) => {
                let room_strs: Vec<String> = rooms.iter().map(|r| r.to_string()).collect();
                serde_json::to_value(room_strs).map_err(matrix_sdk_base::store::StoreError::Json)?
            },
            StateStoreDataValue::UtdHookManagerData(data) => {
                serde_json::to_value(data).map_err(matrix_sdk_base::store::StoreError::Json)?
            },
            StateStoreDataValue::ComposerDraft(draft) => {
                serde_json::to_value(draft).map_err(matrix_sdk_base::store::StoreError::Json)?
            },
            StateStoreDataValue::SeenKnockRequests(requests) => {
                let map: BTreeMap<String, String> = requests
                    .iter()
                    .map(|(event_id, user_id)| (event_id.to_string(), user_id.to_string()))
                    .collect();
                serde_json::to_value(map).map_err(matrix_sdk_base::store::StoreError::Json)?
            },
        };

        // Create the KV entry and save it
        let entry = KeyValueEntry {
            key: key_str.clone(),
            value: json_value,
            value_type: value_type.to_string(),
        };

        match self.kv_dao.set_value(entry).await {
            Ok(_) => {
                trace!(key = %key_str, "Set KV data successfully");
                Ok(())
            },
            Err(err) => {
                error!(key = %key_str, error = %err, "Failed to set KV data");
                Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err)))
            },
        }
    }

    #[instrument(skip(self, key), level = "debug")]
    async fn remove_kv_data(
        &self,
        key: matrix_sdk_base::StateStoreDataKey<'_>,
    ) -> matrix_sdk_base::store::Result<()> {
        let key_str = Self::data_key_to_string(key);
        trace!(key = %key_str, "Removing KV data");

        match self.kv_dao.remove_value(&key_str).await {
            Ok(_) => {
                trace!(key = %key_str, "Removed KV data successfully");
                Ok(())
            },
            Err(err) => {
                error!(key = %key_str, error = %err, "Failed to remove KV data");
                Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err)))
            },
        }
    }

    #[instrument(skip(self, changes), level = "debug")]
    async fn save_changes(&self, changes: &StateChanges) -> matrix_sdk_base::store::Result<()> {
        debug!(
            rooms = changes.rooms.len(),
            presence = changes.presence.len(),
            account_data = changes.account_data.len(),
            "Saving state changes"
        );

        // Use a transaction to ensure atomicity
        let tx = self
            .client
            .begin_transaction()
            .await
            .map_err(|e| matrix_sdk_base::store::StoreError::Backend(Box::new(e)))?;

        // Process room state changes
        for (room_id, room_changes) in &changes.rooms {
            let room_id_str = room_id.to_string();

            // Process state changes
            for (event_type, state_events) in &room_changes.state {
                let event_type_str = event_type.to_string();
                for (state_key, event) in state_events {
                    // Skip if no event (indicating deletion)
                    if let Some(event) = event {
                        let json_value = serde_json::to_value(event)
                            .map_err(matrix_sdk_base::store::StoreError::Json)?;

                        self.room_state_dao
                            .save_state_event_tx(
                                &tx,
                                &room_id_str,
                                &event_type_str,
                                state_key,
                                json_value,
                            )
                            .await
                            .map_err(|e| {
                                matrix_sdk_base::store::StoreError::Backend(Box::new(e))
                            })?;
                    } else {
                        // Delete the state event
                        self.room_state_dao
                            .delete_state_event_tx(&tx, &room_id_str, &event_type_str, state_key)
                            .await
                            .map_err(|e| {
                                matrix_sdk_base::store::StoreError::Backend(Box::new(e))
                            })?;
                    }
                }
            }

            // Process account data changes
            for (event_type, event) in &room_changes.account_data {
                let event_type_str = event_type.to_string();

                if let Some(event) = event {
                    let json_value = serde_json::to_value(event)
                        .map_err(matrix_sdk_base::store::StoreError::Json)?;

                    self.account_data_dao
                        .save_room_account_data_tx(&tx, &room_id_str, &event_type_str, json_value)
                        .await
                        .map_err(|e| matrix_sdk_base::store::StoreError::Backend(Box::new(e)))?;
                } else {
                    // Delete the account data
                    self.account_data_dao
                        .delete_room_account_data_tx(&tx, &room_id_str, &event_type_str)
                        .await
                        .map_err(|e| matrix_sdk_base::store::StoreError::Backend(Box::new(e)))?;
                }
            }

            // Process receipt changes
            for (receipt_type, receipts) in &room_changes.receipts {
                let receipt_type_str = receipt_type.to_string();

                for (event_id, user_receipts) in receipts {
                    let event_id_str = event_id.to_string();

                    for (user_id, receipt) in user_receipts {
                        let user_id_str = user_id.to_string();
                        let json_value = serde_json::to_value(receipt)
                            .map_err(matrix_sdk_base::store::StoreError::Json)?;

                        self.receipt_dao
                            .save_receipt_tx(
                                &tx,
                                &room_id_str,
                                &receipt_type_str,
                                &receipt.thread.to_string(),
                                &event_id_str,
                                &user_id_str,
                                json_value,
                            )
                            .await
                            .map_err(|e| {
                                matrix_sdk_base::store::StoreError::Backend(Box::new(e))
                            })?;
                    }
                }
            }
        }

        // Process presence changes
        for (user_id, presence) in &changes.presence {
            let user_id_str = user_id.to_string();

            if let Some(presence) = presence {
                let json_value = serde_json::to_value(presence)
                    .map_err(matrix_sdk_base::store::StoreError::Json)?;

                self.presence_dao
                    .save_presence_tx(&tx, &user_id_str, json_value)
                    .await
                    .map_err(|e| matrix_sdk_base::store::StoreError::Backend(Box::new(e)))?;
            } else {
                // Delete the presence
                self.presence_dao
                    .delete_presence_tx(&tx, &user_id_str)
                    .await
                    .map_err(|e| matrix_sdk_base::store::StoreError::Backend(Box::new(e)))?;
            }
        }

        // Process account data changes
        for (event_type, event) in &changes.account_data {
            let event_type_str = event_type.to_string();

            if let Some(event) = event {
                let json_value = serde_json::to_value(event)
                    .map_err(matrix_sdk_base::store::StoreError::Json)?;

                self.account_data_dao
                    .save_account_data_tx(&tx, &event_type_str, json_value)
                    .await
                    .map_err(|e| matrix_sdk_base::store::StoreError::Backend(Box::new(e)))?;
            } else {
                // Delete the account data
                self.account_data_dao
                    .delete_account_data_tx(&tx, &event_type_str)
                    .await
                    .map_err(|e| matrix_sdk_base::store::StoreError::Backend(Box::new(e)))?;
            }
        }

        // Commit the transaction
        tx.commit()
            .await
            .map_err(|e| matrix_sdk_base::store::StoreError::Backend(Box::new(e)))?;

        debug!("State changes saved successfully");
        Ok(())
    }

    #[instrument(skip(self), level = "debug")]
    async fn get_presence_event(
        &self,
        user_id: &matrix_sdk_base::ruma::UserId,
    ) -> matrix_sdk_base::store::Result<Option<Raw<PresenceEvent>>> {
        // Retrieve presence event from the DAO
        let user_id_str = user_id.to_string();
        trace!(user_id = %user_id_str, "Getting presence event");

        match self.presence_dao.get_presence(&user_id_str).await {
            Ok(Some(presence)) => {
                // Convert from the storage format to the expected format
                match serde_json::from_value(presence.event) {
                    Ok(event) => {
                        trace!(user_id = %user_id_str, "Found presence event");
                        Ok(Some(event))
                    },
                    Err(err) => {
                        error!(user_id = %user_id_str, error = %err, "Failed to deserialize presence event");
                        Err(matrix_sdk_base::store::StoreError::Json(err))
                    },
                }
            },
            Ok(None) => {
                trace!(user_id = %user_id_str, "No presence event found");
                Ok(None)
            },
            Err(err) => {
                error!(user_id = %user_id_str, error = %err, "Failed to get presence event");
                Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err)))
            },
        }
    }

    #[instrument(skip(self, user_ids), level = "debug")]
    async fn get_presence_events(
        &self,
        user_ids: &[matrix_sdk_base::ruma::OwnedUserId],
    ) -> matrix_sdk_base::store::Result<Vec<Raw<PresenceEvent>>> {
        debug!(user_count = user_ids.len(), "Getting presence events for multiple users");

        // Optimize by using batch operation if available
        if self.presence_dao.supports_batch_operations() {
            let user_id_strs: Vec<String> = user_ids.iter().map(|id| id.to_string()).collect();

            match self.presence_dao.get_presence_batch(&user_id_strs).await {
                Ok(presence_events) => {
                    let mut results = Vec::with_capacity(presence_events.len());

                    for presence in presence_events {
                        match serde_json::from_value(presence.event) {
                            Ok(event) => results.push(event),
                            Err(err) => {
                                warn!(user_id = %presence.user_id, error = %err, "Failed to deserialize presence event");
                                continue;
                            },
                        }
                    }

                    debug!(count = results.len(), "Retrieved presence events batch");
                    Ok(results)
                },
                Err(err) => {
                    error!(error = %err, "Failed to get presence events batch");
                    Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err)))
                },
            }
        } else {
            // Fall back to individual queries
            let mut results = Vec::new();
            for user_id in user_ids {
                let user_id_str = user_id.to_string();
                trace!(user_id = %user_id_str, "Getting individual presence event");

                if let Some(presence) = self
                    .presence_dao
                    .get_presence(&user_id_str)
                    .await
                    .map_err(|e| matrix_sdk_base::store::StoreError::Backend(Box::new(e)))?
                {
                    match serde_json::from_value(presence.event) {
                        Ok(event) => results.push(event),
                        Err(err) => return Err(matrix_sdk_base::store::StoreError::Json(err)),
                    }
                }
            }

            debug!(count = results.len(), "Retrieved presence events individually");
            Ok(results)
        }
    }

    async fn get_state_event(
        &self,
        room_id: &matrix_sdk_base::ruma::RoomId,
        event_type: StateEventType,
        state_key: &str,
    ) -> matrix_sdk_base::store::Result<Option<RawAnySyncOrStrippedState>> {
        let room_id_str = room_id.to_string();
        let event_type_str = event_type.to_string();

        match self
            .room_state_dao
            .get_state_event(&room_id_str, &event_type_str, state_key)
            .await
        {
            Ok(Some(value)) => {
                match serde_json::from_value(value) {
                    Ok(event) => Ok(Some(event)),
                    Err(err) => Err(matrix_sdk_base::store::StoreError::Json(err)),
                }
            },
            Ok(None) => Ok(None),
            Err(err) => Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err))),
        }
    }

    async fn get_state_events(
        &self,
        room_id: &matrix_sdk_base::ruma::RoomId,
        event_type: StateEventType,
    ) -> matrix_sdk_base::store::Result<Vec<RawAnySyncOrStrippedState>> {
        let room_id_str = room_id.to_string();
        let event_type_str = event_type.to_string();

        debug!(room_id = %room_id_str, event_type = %event_type_str, "Getting state events");

        match self.room_state_dao.get_state_events(&room_id_str, &event_type_str).await {
            Ok(events) => {
                let mut results = Vec::with_capacity(events.len());
                for (_, value) in events {
                    match serde_json::from_value(value) {
                        Ok(event) => results.push(event),
                        Err(err) => {
                            warn!(error = %err, "Failed to deserialize state event, skipping");
                            continue;
                        },
                    }
                }

                debug!(count = results.len(), "Retrieved state events successfully");
                Ok(results)
            },
            Err(err) => {
                error!(room_id = %room_id_str, event_type = %event_type_str, error = %err, "Failed to get state events");
                Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err)))
            },
        }
    }

    async fn get_state_events_for_keys(
        &self,
        room_id: &matrix_sdk_base::ruma::RoomId,
        event_type: StateEventType,
        state_keys: &[&str],
    ) -> matrix_sdk_base::store::Result<Vec<RawAnySyncOrStrippedState>> {
        let room_id_str = room_id.to_string();
        let event_type_str = event_type.to_string();

        debug!(
            room_id = %room_id_str,
            event_type = %event_type_str,
            key_count = state_keys.len(),
            "Getting state events for keys"
        );

        // Optimize by using batch operation if available
        if self.room_state_dao.supports_batch_operations() {
            match self
                .room_state_dao
                .get_state_events_for_keys(&room_id_str, &event_type_str, state_keys)
                .await
            {
                Ok(events) => {
                    let mut results = Vec::with_capacity(events.len());
                    for value in events {
                        match serde_json::from_value(value) {
                            Ok(event) => results.push(event),
                            Err(err) => {
                                warn!(error = %err, "Failed to deserialize state event, skipping");
                                continue;
                            },
                        }
                    }

                    debug!(count = results.len(), "Retrieved state events for keys successfully");
                    Ok(results)
                },
                Err(err) => {
                    error!(room_id = %room_id_str, error = %err, "Failed to get state events for keys");
                    Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err)))
                },
            }
        } else {
            // Fall back to individual queries
            let mut results = Vec::new();

            for state_key in state_keys {
                trace!(room_id = %room_id_str, state_key = %state_key, "Getting individual state event");
                if let Some(event) =
                    self.get_state_event(room_id, event_type.clone(), state_key).await?
                {
                    results.push(event);
                }
            }

            debug!(count = results.len(), "Retrieved state events for keys individually");
            Ok(results)
        }
    }

    async fn get_profile(
        &self,
        room_id: &RoomId,
        user_id: &UserId,
    ) -> matrix_sdk_base::store::Result<Option<MinimalRoomMemberEvent>> {
        let room_id_str = room_id.to_string();
        let user_id_str = user_id.to_string();

        // Get the member event from state events
        match self
            .room_state_dao
            .get_state_event(&room_id_str, "m.room.member", &user_id_str)
            .await
        {
            Ok(Some(value)) => {
                // Try to extract the minimal info we need
                let display_name = value
                    .get("content")
                    .and_then(|c| c.get("displayname"))
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string());
                let avatar_url = value
                    .get("content")
                    .and_then(|c| c.get("avatar_url"))
                    .and_then(|a| a.as_str())
                    .map(|s| s.to_string());
                let membership = value
                    .get("content")
                    .and_then(|c| c.get("membership"))
                    .and_then(|m| m.as_str())
                    .map(|s| s.to_string());

                // Construct the minimal event
                let member_event = MinimalRoomMemberEvent {
                    displayname: display_name,
                    avatar_url: avatar_url.and_then(|u| OwnedMxcUri::try_from(u).ok()),
                    membership: membership
                        .and_then(|m| MembershipState::from_str(&m).ok())
                        .unwrap_or(MembershipState::Leave),
                };

                Ok(Some(member_event))
            },
            Ok(None) => Ok(None),
            Err(err) => Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err))),
        }
    }

    async fn get_profiles<'a>(
        &self,
        room_id: &RoomId,
        user_ids: &'a [OwnedUserId],
    ) -> matrix_sdk_base::store::Result<BTreeMap<&'a UserId, MinimalRoomMemberEvent>> {
        let mut results = BTreeMap::new();

        for user_id in user_ids {
            if let Some(profile) = self.get_profile(room_id, user_id).await? {
                results.insert(user_id, profile);
            }
        }

        Ok(results)
    }

    async fn get_user_ids(
        &self,
        room_id: &RoomId,
        memberships: RoomMemberships,
    ) -> matrix_sdk_base::store::Result<Vec<OwnedUserId>> {
        let room_id_str = room_id.to_string();

        // Convert the membership filter into string representations
        let membership_strs: Vec<String> = memberships.iter().map(|m| m.to_string()).collect();

        match self
            .room_state_dao
            .get_room_users_by_membership(&room_id_str, &membership_strs)
            .await
        {
            Ok(user_ids) => {
                let mut results = Vec::new();
                for user_id_str in user_ids {
                    match OwnedUserId::try_from(user_id_str) {
                        Ok(user_id) => results.push(user_id),
                        Err(_) => continue, // Skip invalid user IDs
                    }
                }
                Ok(results)
            },
            Err(err) => Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err))),
        }
    }

    async fn get_room_infos(&self) -> matrix_sdk_base::store::Result<Vec<RoomInfo>> {
        match self.room_state_dao.get_all_room_ids().await {
            Ok(room_ids) => {
                let mut results = Vec::new();

                for room_id_str in room_ids {
                    // Try to convert to RoomId
                    match OwnedRoomId::try_from(room_id_str.clone()) {
                        Ok(room_id) => {
                            // Get name and topic from state events
                            let name_event = self
                                .room_state_dao
                                .get_state_event(&room_id_str, "m.room.name", "")
                                .await
                                .ok()
                                .flatten();
                            let topic_event = self
                                .room_state_dao
                                .get_state_event(&room_id_str, "m.room.topic", "")
                                .await
                                .ok()
                                .flatten();

                            let name = name_event
                                .and_then(|e| e.get("content"))
                                .and_then(|c| c.get("name"))
                                .and_then(|n| n.as_str())
                                .map(|s| s.to_string());

                            let topic = topic_event
                                .and_then(|e| e.get("content"))
                                .and_then(|c| c.get("topic"))
                                .and_then(|t| t.as_str())
                                .map(|s| s.to_string());

                            // Construct the room info
                            let room_info = RoomInfo {
                                room_id,
                                name,
                                topic,
                                // Other fields would need to be populated from additional state events
                                ..Default::default()
                            };

                            results.push(room_info);
                        },
                        Err(_) => continue, // Skip invalid room IDs
                    }
                }

                Ok(results)
            },
            Err(err) => Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err))),
        }
    }

    async fn get_users_with_display_name(
        &self,
        room_id: &RoomId,
        display_name: &DisplayName,
    ) -> matrix_sdk_base::store::Result<BTreeSet<OwnedUserId>> {
        let room_id_str = room_id.to_string();
        let display_name_str = display_name.to_string();

        match self
            .room_state_dao
            .get_users_with_display_name(&room_id_str, &display_name_str)
            .await
        {
            Ok(user_ids) => {
                let mut results = BTreeSet::new();
                for user_id_str in user_ids {
                    match OwnedUserId::try_from(user_id_str) {
                        Ok(user_id) => {
                            results.insert(user_id);
                        },
                        Err(_) => continue, // Skip invalid user IDs
                    }
                }
                Ok(results)
            },
            Err(err) => Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err))),
        }
    }

    async fn get_users_with_display_names<'a>(
        &self,
        room_id: &RoomId,
        display_names: &'a [DisplayName],
    ) -> matrix_sdk_base::store::Result<HashMap<&'a DisplayName, BTreeSet<OwnedUserId>>> {
        let mut results = HashMap::new();

        for display_name in display_names {
            let user_ids = self.get_users_with_display_name(room_id, display_name).await?;
            results.insert(display_name, user_ids);
        }

        Ok(results)
    }

    async fn get_account_data_event(
        &self,
        event_type: GlobalAccountDataEventType,
    ) -> matrix_sdk_base::store::Result<Option<Raw<AnyGlobalAccountDataEvent>>> {
        let event_type_str = event_type.to_string();

        match self.account_data_dao.get_account_data("", &event_type_str).await {
            Ok(Some(account_data)) => {
                match serde_json::from_value(account_data.event) {
                    Ok(event) => Ok(Some(event)),
                    Err(err) => Err(matrix_sdk_base::store::StoreError::Json(err)),
                }
            },
            Ok(None) => Ok(None),
            Err(err) => Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err))),
        }
    }

    async fn get_room_account_data_event(
        &self,
        room_id: &RoomId,
        event_type: RoomAccountDataEventType,
    ) -> matrix_sdk_base::store::Result<Option<Raw<AnyRoomAccountDataEvent>>> {
        let room_id_str = room_id.to_string();
        let event_type_str = event_type.to_string();

        match self.account_data_dao.get_account_data(&room_id_str, &event_type_str).await {
            Ok(Some(account_data)) => {
                match serde_json::from_value(account_data.event) {
                    Ok(event) => Ok(Some(event)),
                    Err(err) => Err(matrix_sdk_base::store::StoreError::Json(err)),
                }
            },
            Ok(None) => Ok(None),
            Err(err) => Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err))),
        }
    }

    async fn get_user_room_receipt_event(
        &self,
        room_id: &RoomId,
        receipt_type: ReceiptType,
        thread: ReceiptThread,
        user_id: &UserId,
    ) -> matrix_sdk_base::store::Result<Option<(OwnedEventId, Receipt)>> {
        let room_id_str = room_id.to_string();
        let user_id_str = user_id.to_string();
        let receipt_type_str = receipt_type.to_string();
        let thread_str = thread.to_string();

        match self
            .receipt_dao
            .get_user_receipt(&room_id_str, &receipt_type_str, &thread_str, &user_id_str)
            .await
        {
            Ok(Some(receipt)) => {
                // Convert to the expected format
                match OwnedEventId::try_from(receipt.event_id.clone()) {
                    Ok(event_id) => {
                        let receipt_value = Receipt {
                            ts: receipt.timestamp.map(|ts| MilliSecondsSinceUnixEpoch(ts as u64)),
                        };
                        Ok(Some((event_id, receipt_value)))
                    },
                    Err(_) => {
                        Err(matrix_sdk_base::store::StoreError::InvalidRecordFormat(
                            "Invalid event ID in receipt".into(),
                        ))
                    },
                }
            },
            Ok(None) => Ok(None),
            Err(err) => Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err))),
        }
    }

    async fn get_event_room_receipt_events(
        &self,
        room_id: &RoomId,
        receipt_type: ReceiptType,
        thread: ReceiptThread,
        event_id: &EventId,
    ) -> matrix_sdk_base::store::Result<Vec<(OwnedUserId, Receipt)>> {
        let room_id_str = room_id.to_string();
        let event_id_str = event_id.to_string();
        let receipt_type_str = receipt_type.to_string();
        let thread_str = thread.to_string();

        match self
            .receipt_dao
            .get_event_receipts(&room_id_str, &receipt_type_str, &thread_str, &event_id_str)
            .await
        {
            Ok(receipts) => {
                let mut results = Vec::new();

                for receipt in receipts {
                    match OwnedUserId::try_from(receipt.user_id.clone()) {
                        Ok(user_id) => {
                            let receipt_value = Receipt {
                                ts: receipt
                                    .timestamp
                                    .map(|ts| MilliSecondsSinceUnixEpoch(ts as u64)),
                            };
                            results.push((user_id, receipt_value));
                        },
                        Err(_) => continue, // Skip invalid user IDs
                    }
                }

                Ok(results)
            },
            Err(err) => Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err))),
        }
    }

    #[instrument(skip(self, key), level = "debug")]
    async fn get_custom_value(
        &self,
        key: &[u8],
    ) -> matrix_sdk_base::store::Result<Option<Vec<u8>>> {
        let key_hex = hex::encode(key);
        trace!(key = %key_hex, "Getting custom value");

        match self.custom_dao.get_value(&key_hex).await {
            Ok(Some(value)) => {
                trace!(key = %key_hex, "Found custom value");
                Ok(Some(value))
            },
            Ok(None) => {
                trace!(key = %key_hex, "No custom value found");
                Ok(None)
            },
            Err(err) => {
                error!(key = %key_hex, error = %err, "Failed to get custom value");
                Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err)))
            },
        }
    }

    #[instrument(skip(self, key, value), level = "debug")]
    async fn set_custom_value(
        &self,
        key: &[u8],
        value: Vec<u8>,
    ) -> matrix_sdk_base::store::Result<Option<Vec<u8>>> {
        let key_hex = hex::encode(key);
        trace!(key = %key_hex, size = value.len(), "Setting custom value");

        // First get the old value if any
        let old_value = self.get_custom_value(key).await?;

        // Then set the new value
        match self.custom_dao.set_value(&key_hex, value).await {
            Ok(_) => {
                trace!(key = %key_hex, "Set custom value successfully");
                Ok(old_value)
            },
            Err(err) => {
                error!(key = %key_hex, error = %err, "Failed to set custom value");
                Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err)))
            },
        }
    }

    #[instrument(skip(self, key), level = "debug")]
    async fn remove_custom_value(
        &self,
        key: &[u8],
    ) -> matrix_sdk_base::store::Result<Option<Vec<u8>>> {
        let key_hex = hex::encode(key);
        trace!(key = %key_hex, "Removing custom value");

        // First get the old value if any
        let old_value = self.get_custom_value(key).await?;

        // Then remove the value
        match self.custom_dao.remove_value(&key_hex).await {
            Ok(_) => {
                trace!(key = %key_hex, "Removed custom value successfully");
                Ok(old_value)
            },
            Err(err) => {
                error!(key = %key_hex, error = %err, "Failed to remove custom value");
                Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err)))
            },
        }
    }

    async fn remove_room(&self, room_id: &RoomId) -> matrix_sdk_base::store::Result<()> {
        let room_id_str = room_id.to_string();

        // Remove all data for this room from all DAOs
        match self.room_state_dao.remove_room(&room_id_str).await {
            Ok(_) => {
                // Continue with other DAOs
                // Remove account data
                let _ = self.account_data_dao.remove_room(&room_id_str).await;
                // Remove receipts
                let _ = self.receipt_dao.remove_room(&room_id_str).await;
                // Remove send queue items
                let _ = self.send_queue_dao.remove_room(&room_id_str).await;

                Ok(())
            },
            Err(err) => Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err))),
        }
    }

    async fn save_send_queue_request(
        &self,
        room_id: &RoomId,
        transaction_id: OwnedTransactionId,
        created_at: MilliSecondsSinceUnixEpoch,
        request: QueuedRequestKind,
        priority: usize,
    ) -> matrix_sdk_base::store::Result<()> {
        let room_id_str = room_id.to_string();
        let transaction_id_str = transaction_id.to_string();
        let created_at_millis = created_at.0 as i64;

        // Serialize the request content
        let content = serde_json::to_value(&request)
            .map_err(|e| matrix_sdk_base::store::StoreError::Json(e))?;

        match self
            .send_queue_dao
            .save_request(
                &room_id_str,
                &transaction_id_str,
                created_at_millis,
                &content,
                priority,
                None,
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(err) => Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err))),
        }
    }

    async fn update_send_queue_request(
        &self,
        room_id: &RoomId,
        transaction_id: &TransactionId,
        content: QueuedRequestKind,
    ) -> matrix_sdk_base::store::Result<bool> {
        let room_id_str = room_id.to_string();
        let transaction_id_str = transaction_id.to_string();

        // Serialize the request content
        let content_value = serde_json::to_value(&content)
            .map_err(|e| matrix_sdk_base::store::StoreError::Json(e))?;

        match self
            .send_queue_dao
            .update_request_content(&room_id_str, &transaction_id_str, &content_value)
            .await
        {
            Ok(updated) => Ok(updated),
            Err(err) => Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err))),
        }
    }

    async fn remove_send_queue_request(
        &self,
        room_id: &RoomId,
        transaction_id: &TransactionId,
    ) -> matrix_sdk_base::store::Result<bool> {
        let room_id_str = room_id.to_string();
        let transaction_id_str = transaction_id.to_string();

        match self.send_queue_dao.remove_request(&room_id_str, &transaction_id_str).await {
            Ok(removed) => Ok(removed),
            Err(err) => Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err))),
        }
    }

    async fn load_send_queue_requests(
        &self,
        room_id: &RoomId,
    ) -> matrix_sdk_base::store::Result<Vec<QueuedRequest>> {
        let room_id_str = room_id.to_string();

        match self.send_queue_dao.get_room_requests(&room_id_str).await {
            Ok(requests) => {
                let mut results = Vec::new();

                for request in requests {
                    // Deserialize the content
                    let content: QueuedRequestKind = serde_json::from_value(request.content)
                        .map_err(|e| matrix_sdk_base::store::StoreError::Json(e))?;

                    // Convert transaction ID
                    let txn_id =
                        OwnedTransactionId::try_from(request.transaction_id).map_err(|_| {
                            matrix_sdk_base::store::StoreError::InvalidRecordFormat(
                                "Invalid transaction ID".into(),
                            )
                        })?;

                    // Create queued request
                    let queued_request = QueuedRequest {
                        room_id: room_id.to_owned(),
                        event_type: content.event_type().to_string(),
                        txn_id,
                        created_at: MilliSecondsSinceUnixEpoch(request.created_at as u64),
                        kind: content,
                        priority: request.priority,
                        error: None, // TODO: Handle error storage
                    };

                    results.push(queued_request);
                }

                // Sort by priority (descending) and then by created_at (ascending)
                results.sort_by(|a, b| {
                    b.priority.cmp(&a.priority).then(a.created_at.cmp(&b.created_at))
                });

                Ok(results)
            },
            Err(err) => Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err))),
        }
    }

    async fn update_send_queue_request_status(
        &self,
        room_id: &RoomId,
        transaction_id: &TransactionId,
        error: Option<QueueWedgeError>,
    ) -> matrix_sdk_base::store::Result<()> {
        let room_id_str = room_id.to_string();
        let transaction_id_str = transaction_id.to_string();

        let error_json = match error {
            Some(err) => {
                Some(
                    serde_json::to_value(&err)
                        .map_err(|e| matrix_sdk_base::store::StoreError::Json(e))?,
                )
            },
            None => None,
        };

        match self
            .send_queue_dao
            .update_request_status(&room_id_str, &transaction_id_str, error_json.as_ref())
            .await
        {
            Ok(_) => Ok(()),
            Err(err) => Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err))),
        }
    }

    async fn load_rooms_with_unsent_requests(
        &self,
    ) -> matrix_sdk_base::store::Result<Vec<OwnedRoomId>> {
        match self.send_queue_dao.get_rooms_with_requests().await {
            Ok(room_ids) => {
                let mut results = Vec::new();

                for room_id_str in room_ids {
                    match OwnedRoomId::try_from(room_id_str) {
                        Ok(room_id) => results.push(room_id),
                        Err(_) => continue, // Skip invalid room IDs
                    }
                }

                Ok(results)
            },
            Err(err) => Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err))),
        }
    }

    async fn save_dependent_queued_request(
        &self,
        room_id: &RoomId,
        parent_txn_id: &TransactionId,
        own_txn_id: ChildTransactionId,
        created_at: MilliSecondsSinceUnixEpoch,
        content: DependentQueuedRequestKind,
    ) -> matrix_sdk_base::store::Result<()> {
        let room_id_str = room_id.to_string();
        let parent_txn_id_str = parent_txn_id.to_string();
        let own_txn_id_str = own_txn_id.to_string();
        let created_at_millis = created_at.0 as i64;

        // Serialize the request content
        let content_value = serde_json::to_value(&content)
            .map_err(|e| matrix_sdk_base::store::StoreError::Json(e))?;

        match self
            .request_dependency_dao
            .save_dependent_request(
                &room_id_str,
                &parent_txn_id_str,
                &own_txn_id_str,
                created_at_millis,
                &content_value,
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(err) => Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err))),
        }
    }

    async fn mark_dependent_queued_requests_as_ready(
        &self,
        room_id: &RoomId,
        parent_txn_id: &TransactionId,
        sent_parent_key: SentRequestKey,
    ) -> matrix_sdk_base::store::Result<usize> {
        let room_id_str = room_id.to_string();
        let parent_txn_id_str = parent_txn_id.to_string();

        // Serialize the sent parent key
        let key_json = serde_json::to_value(&sent_parent_key)
            .map_err(|e| matrix_sdk_base::store::StoreError::Json(e))?;

        match self
            .request_dependency_dao
            .mark_requests_ready(&room_id_str, &parent_txn_id_str, &key_json)
            .await
        {
            Ok(count) => Ok(count),
            Err(err) => Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err))),
        }
    }

    async fn update_dependent_queued_request(
        &self,
        room_id: &RoomId,
        own_transaction_id: &ChildTransactionId,
        new_content: DependentQueuedRequestKind,
    ) -> matrix_sdk_base::store::Result<bool> {
        let room_id_str = room_id.to_string();
        let own_txn_id_str = own_transaction_id.to_string();

        // Serialize the content
        let content_value = serde_json::to_value(&new_content)
            .map_err(|e| matrix_sdk_base::store::StoreError::Json(e))?;

        match self
            .request_dependency_dao
            .update_dependent_request(&room_id_str, &own_txn_id_str, &content_value)
            .await
        {
            Ok(updated) => Ok(updated),
            Err(err) => Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err))),
        }
    }

    async fn remove_dependent_queued_request(
        &self,
        room: &RoomId,
        own_txn_id: &ChildTransactionId,
    ) -> matrix_sdk_base::store::Result<bool> {
        let room_id_str = room.to_string();
        let own_txn_id_str = own_txn_id.to_string();

        match self
            .request_dependency_dao
            .remove_dependent_request(&room_id_str, &own_txn_id_str)
            .await
        {
            Ok(removed) => Ok(removed),
            Err(err) => Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err))),
        }
    }

    async fn load_dependent_queued_requests(
        &self,
        room: &RoomId,
    ) -> matrix_sdk_base::store::Result<Vec<DependentQueuedRequest>> {
        let room_id_str = room.to_string();

        match self.request_dependency_dao.get_room_dependent_requests(&room_id_str).await {
            Ok(requests) => {
                let mut results = Vec::new();

                for request in requests {
                    // Deserialize the content
                    let content: DependentQueuedRequestKind =
                        serde_json::from_value(request.content)
                            .map_err(|e| matrix_sdk_base::store::StoreError::Json(e))?;

                    // Convert transaction IDs
                    let own_txn_id =
                        ChildTransactionId::from_str(&request.own_txn_id).map_err(|_| {
                            matrix_sdk_base::store::StoreError::InvalidRecordFormat(
                                "Invalid child transaction ID".into(),
                            )
                        })?;

                    let parent_txn_id = OwnedTransactionId::try_from(request.parent_txn_id)
                        .map_err(|_| {
                            matrix_sdk_base::store::StoreError::InvalidRecordFormat(
                                "Invalid parent transaction ID".into(),
                            )
                        })?;

                    // Get sent parent key if available
                    let sent_parent_key = if let Some(key_json) = request.parent_event_key {
                        Some(
                            serde_json::from_value(key_json)
                                .map_err(|e| matrix_sdk_base::store::StoreError::Json(e))?,
                        )
                    } else {
                        None
                    };

                    // Create dependent queued request
                    let dep_request = DependentQueuedRequest {
                        room_id: room.to_owned(),
                        parent_txn_id,
                        own_txn_id,
                        created_at: MilliSecondsSinceUnixEpoch(request.created_at as u64),
                        sent_parent_key,
                        kind: content,
                    };

                    results.push(dep_request);
                }

                // Sort by created_at (ascending)
                results.sort_by(|a, b| a.created_at.cmp(&b.created_at));

                Ok(results)
            },
            Err(err) => Err(matrix_sdk_base::store::StoreError::Backend(Box::new(err))),
        }
    }
}

// Implementation of CyrumStateStore for SurrealStateStore
impl CyrumStateStore for SurrealStateStore {
    fn get_kv_data(
        &self,
        key: StateStoreDataKey<'_>,
    ) -> KeyValueFuture<Option<StateStoreDataValue>> {
        KeyValueFuture::new(async move {
            // Use fully qualified syntax to call the StateStore trait method
            <Self as matrix_sdk_base::store::StateStore>::get_kv_data(self, key)
                .await
                .map_err(crate::error::StoreError::from) // Map sdk::StoreError -> crate::StoreError
        })
    }

    fn set_kv_data(
        &self,
        key: StateStoreDataKey<'_>,
        value: StateStoreDataValue,
    ) -> KeyValueFuture<()> {
        KeyValueFuture::new(async move {
            // Use fully qualified syntax to call the StateStore trait method
            <Self as matrix_sdk_base::store::StateStore>::set_kv_data(self, key, value)
                .await
                .map_err(crate::error::StoreError::from) // Map sdk::StoreError -> crate::StoreError
        })
    }

    fn remove_kv_data(&self, key: StateStoreDataKey<'_>) -> KeyValueFuture<()> {
        KeyValueFuture::new(async move {
            // Use fully qualified syntax to call the StateStore trait method
            <Self as matrix_sdk_base::store::StateStore>::remove_kv_data(self, key)
                .await
                .map_err(crate::error::StoreError::from) // Map sdk::StoreError -> crate::StoreError
        })
    }

    fn save_changes(&self, changes: &StateChanges) -> StateChangesFuture<()> {
        let changes = changes.clone();
        StateChangesFuture::new(async move {
            <Self as matrix_sdk_base::store::StateStore>::save_changes(self, &changes)
                .await
                .map_err(crate::error::StoreError::from) // Map sdk::StoreError -> crate::StoreError
        })
    }

    fn get_presence_event(&self, user_id: &UserId) -> PresenceFuture<Option<Raw<PresenceEvent>>> {
        let user_id = user_id.to_owned();
        PresenceFuture::new(async move {
            <Self as matrix_sdk_base::store::StateStore>::get_presence_event(self, &user_id)
                .await
                .map_err(crate::error::StoreError::from) // Map sdk::StoreError -> crate::StoreError
        })
    }

    fn get_presence_events(
        &self,
        user_ids: &[matrix_sdk_base::ruma::OwnedUserId],
    ) -> PresenceStream {
        let user_ids = user_ids.to_vec();
        PresenceStream::new(async_stream::stream! {
            match <Self as matrix_sdk_base::store::StateStore>::get_presence_events(self, &user_ids).await {
                Ok(events) => {
                    for event in events {
                        yield Ok(event);
                    }
                },
                Err(e) => {
                    yield Err(crate::error::StoreError::from(e)); // Map sdk::StoreError -> crate::StoreError
                }
            }
        })
    }

    fn get_state_event(
        &self,
        room_id: &RoomId,
        event_type: StateEventType,
        state_key: &str,
    ) -> StateEventFuture<Option<RawAnySyncOrStrippedState>> {
        let room_id = room_id.to_owned();
        let state_key = state_key.to_owned();
        StateEventFuture::new(async move {
            <Self as matrix_sdk_base::store::StateStore>::get_state_event(
                self, &room_id, event_type, &state_key,
            )
            .await
            .map_err(crate::error::StoreError::from) // Map sdk::StoreError -> crate::StoreError
        })
    }

    fn get_state_events(&self, room_id: &RoomId, event_type: StateEventType) -> StateEventStream {
        let room_id = room_id.to_owned();
        StateEventStream::new(async_stream::stream! {
            match <Self as matrix_sdk_base::store::StateStore>::get_state_events(self, &room_id, event_type).await {
                Ok(events) => {
                    for event in events {
                        yield Ok(event);
                    }
                },
                Err(e) => {
                    yield Err(crate::error::StoreError::from(e)); // Map sdk::StoreError -> crate::StoreError
                }
            }
        })
    }

    fn get_state_events_for_keys(
        &self,
        room_id: &RoomId,
        event_type: StateEventType,
        state_keys: &[&str],
    ) -> StateEventStream {
        let room_id = room_id.to_owned();
        let state_keys = state_keys.iter().map(|&s| s.to_owned()).collect::<Vec<_>>();
        let state_keys_refs: Vec<&str> = state_keys.iter().map(|s| s.as_str()).collect();

        StateEventStream::new(async_stream::stream! {
            match <Self as matrix_sdk_base::store::StateStore>::get_state_events_for_keys(self, &room_id, event_type, &state_keys_refs).await {
                Ok(events) => {
                    for event in events {
                        yield Ok(event);
                    }
                },
                Err(e) => {
                    yield Err(crate::error::StoreError::from(e)); // Map sdk::StoreError -> crate::StoreError
                }
            }
        })
    }

    fn get_profile(
        &self,
        room_id: &RoomId,
        user_id: &UserId,
    ) -> ProfileFuture<Option<MinimalRoomMemberEvent>> {
        let room_id = room_id.to_owned();
        let user_id = user_id.to_owned();
        ProfileFuture::new(async move {
            <Self as matrix_sdk_base::store::StateStore>::get_profile(self, &room_id, &user_id)
                .await
                .map_err(crate::error::StoreError::from) // Map sdk::StoreError -> crate::StoreError
        })
    }

    fn get_profiles<'a>(
        &self,
        room_id: &RoomId,
        user_ids: &'a [OwnedUserId],
    ) -> ProfileFuture<BTreeMap<OwnedUserId, MinimalRoomMemberEvent>> {
        // We need to fix lifetime issues by changing the return type to own the UserId
        let room_id = room_id.to_owned();
        let user_ids = user_ids.to_vec();

        // Create a cloned version of self that we can move into the async block
        // We need to clone the database client which should have proper Clone implementation
        let db_client = self.client.clone();

        ProfileFuture::new(async move {
            // Create DAOs for the async block
            let room_state_dao = crate::db::RoomStateDao::new(db_client.clone());

            // Get profiles from room members
            let mut result = BTreeMap::new();

            // For each user ID, get the profile directly from the DAO
            for user_id in &user_ids {
                let user_id_str = user_id.to_string();
                let room_id_str = room_id.to_string();

                // Get the member event from room state
                if let Ok(Some(value)) = room_state_dao
                    .get_state_event(&room_id_str, "m.room.member", &user_id_str)
                    .await
                {
                    // Extract display name and avatar URL
                    let display_name = value
                        .get("content")
                        .and_then(|c| c.get("displayname"))
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string());
                    let avatar_url = value
                        .get("content")
                        .and_then(|c| c.get("avatar_url"))
                        .and_then(|a| a.as_str())
                        .map(|s| s.to_string());
                    let membership = value
                        .get("content")
                        .and_then(|c| c.get("membership"))
                        .and_then(|m| m.as_str())
                        .map(|s| s.to_string());

                    // Construct the minimal event
                    let member_event = MinimalRoomMemberEvent {
                        displayname: display_name,
                        avatar_url: avatar_url.and_then(|u| matrix_sdk_base::ruma::OwnedMxcUri::try_from(u).ok()),
                        membership: membership.and_then(|m| matrix_sdk_base::ruma::events::room::member::MembershipState::from_str(&m).ok())
                            .unwrap_or(matrix_sdk_base::ruma::events::room::member::MembershipState::Leave),
                    };

                    result.insert(user_id.clone(), member_event);
                }
            }

            Ok(result)
        })
    }

    fn get_user_ids(&self, room_id: &RoomId, memberships: RoomMemberships) -> UserIdStream {
        let room_id = room_id.to_owned();
        UserIdStream::new(async_stream::stream! {
            match <Self as matrix_sdk_base::store::StateStore>::get_user_ids(self, &room_id, memberships).await {
                Ok(user_ids) => {
                    for user_id in user_ids {
                        yield Ok(user_id);
                    }
                },
                Err(e) => {
                    yield Err(crate::error::StoreError::from(e)); // Map sdk::StoreError -> crate::StoreError
                }
            }
        })
    }

    fn get_room_infos(&self) -> RoomInfoStream {
        RoomInfoStream::new(async_stream::stream! {
            match <Self as matrix_sdk_base::store::StateStore>::get_room_infos(self).await {
                Ok(room_infos) => {
                    for room_info in room_infos {
                        yield Ok(room_info);
                    }
                },
                Err(e) => {
                    yield Err(crate::error::StoreError::from(e)); // Map sdk::StoreError -> crate::StoreError
                }
            }
        })
    }

    fn get_users_with_display_name(
        &self,
        room_id: &RoomId,
        display_name: &DisplayName,
    ) -> DisplayNameFuture<BTreeSet<OwnedUserId>> {
        let room_id = room_id.to_owned();
        let display_name = display_name.clone();
        DisplayNameFuture::new(async move {
            <Self as matrix_sdk_base::store::StateStore>::get_users_with_display_name(
                self,
                &room_id,
                &display_name,
            )
            .await
            .map_err(crate::error::StoreError::from) // Map sdk::StoreError -> crate::StoreError
        })
    }

    fn get_users_with_display_names<'a>(
        &self,
        room_id: &RoomId,
        display_names: &'a [DisplayName],
    ) -> DisplayNameFuture<HashMap<&'a DisplayName, BTreeSet<OwnedUserId>>> {
        // We need to fix lifetime issues by changing the return type to own the DisplayName
        let room_id = room_id.to_owned();
        let display_names = display_names.to_vec();

        // Create a cloned version of self that we can move into the async block
        // We need to clone the database client which should have proper Clone implementation
        let db_client = self.client.clone();

        DisplayNameFuture::new(async move {
            // Create DAOs for the async block
            let room_state_dao = crate::db::RoomStateDao::new(db_client.clone());

            // Create a new map with owned keys
            let mut result = HashMap::new();

            // For each display name, get the matching users from the room state
            for display_name in &display_names {
                let room_id_str = room_id.to_string();
                let display_name_str = display_name.to_string();

                // Get users with this display name
                match room_state_dao
                    .get_users_with_display_name(&room_id_str, &display_name_str)
                    .await
                {
                    Ok(user_ids) => {
                        let mut user_set = BTreeSet::new();
                        for user_id_str in user_ids {
                            if let Ok(user_id) = OwnedUserId::try_from(user_id_str) {
                                user_set.insert(user_id);
                            }
                        }

                        if !user_set.is_empty() {
                            result.insert(display_name.clone(), user_set);
                        }
                    },
                    Err(_) => continue, // Skip on error
                }
            }

            Ok(result)
        })
    }

    fn get_account_data_event(
        &self,
        event_type: GlobalAccountDataEventType,
    ) -> AccountDataFuture<Option<Raw<AnyGlobalAccountDataEvent>>> {
        AccountDataFuture::new(async move {
            <Self as matrix_sdk_base::store::StateStore>::get_account_data_event(self, event_type)
                .await
                .map_err(crate::error::StoreError::from) // Map sdk::StoreError -> crate::StoreError
        })
    }

    fn get_room_account_data_event(
        &self,
        room_id: &RoomId,
        event_type: RoomAccountDataEventType,
    ) -> AccountDataFuture<Option<Raw<AnyRoomAccountDataEvent>>> {
        let room_id = room_id.to_owned();
        AccountDataFuture::new(async move {
            <Self as matrix_sdk_base::store::StateStore>::get_room_account_data_event(
                self, &room_id, event_type,
            )
            .await
            .map_err(crate::error::StoreError::from) // Map sdk::StoreError -> crate::StoreError
        })
    }

    fn get_user_room_receipt_event(
        &self,
        room_id: &RoomId,
        receipt_type: ReceiptType,
        thread: ReceiptThread,
        user_id: &UserId,
    ) -> ReceiptFuture<Option<(OwnedEventId, Receipt)>> {
        let room_id = room_id.to_owned();
        let user_id = user_id.to_owned();
        ReceiptFuture::new(async move {
            <Self as matrix_sdk_base::store::StateStore>::get_user_room_receipt_event(
                self,
                &room_id,
                receipt_type,
                thread,
                &user_id,
            )
            .await
            .map_err(crate::error::StoreError::from) // Map sdk::StoreError -> crate::StoreError
        })
    }

    fn get_event_room_receipt_events(
        &self,
        room_id: &RoomId,
        receipt_type: ReceiptType,
        thread: ReceiptThread,
        event_id: &EventId,
    ) -> ReceiptStream {
        let room_id = room_id.to_owned();
        let event_id = event_id.to_owned();
        ReceiptStream::new(async_stream::stream! {
            match <Self as matrix_sdk_base::store::StateStore>::get_event_room_receipt_events(self, &room_id, receipt_type, thread, &event_id).await {
                Ok(receipts) => {
                    for receipt in receipts {
                        yield Ok(receipt);
                    }
                },
                Err(e) => {
                    yield Err(crate::error::StoreError::from(e)); // Map sdk::StoreError -> crate::StoreError
                }
            }
        })
    }

    fn get_custom_value(&self, key: &[u8]) -> CustomValueFuture<Option<Vec<u8>>> {
        let key = key.to_vec();
        CustomValueFuture::new(async move {
            <Self as matrix_sdk_base::store::StateStore>::get_custom_value(self, &key)
                .await
                .map_err(crate::error::StoreError::from) // Map sdk::StoreError -> crate::StoreError
        })
    }

    fn set_custom_value(&self, key: &[u8], value: Vec<u8>) -> CustomValueFuture<Option<Vec<u8>>> {
        let key = key.to_vec();
        CustomValueFuture::new(async move {
            <Self as matrix_sdk_base::store::StateStore>::set_custom_value(self, &key, value)
                .await
                .map_err(crate::error::StoreError::from) // Map sdk::StoreError -> crate::StoreError
        })
    }

    fn remove_custom_value(&self, key: &[u8]) -> CustomValueFuture<Option<Vec<u8>>> {
        let key = key.to_vec();
        CustomValueFuture::new(async move {
            <Self as matrix_sdk_base::store::StateStore>::remove_custom_value(self, &key)
                .await
                .map_err(crate::error::StoreError::from) // Map sdk::StoreError -> crate::StoreError
        })
    }

    fn remove_room(&self, room_id: &RoomId) -> RoomFuture<()> {
        let room_id = room_id.to_owned();
        RoomFuture::new(async move {
            <Self as matrix_sdk_base::store::StateStore>::remove_room(self, &room_id)
                .await
                .map_err(crate::error::StoreError::from) // Map sdk::StoreError -> crate::StoreError
        })
    }

    fn save_send_queue_request(
        &self,
        room_id: &RoomId,
        transaction_id: OwnedTransactionId,
        created_at: MilliSecondsSinceUnixEpoch,
        request: QueuedRequestKind,
        priority: usize,
    ) -> SendQueueFuture<()> {
        let room_id = room_id.to_owned();
        SendQueueFuture::new(async move {
            <Self as matrix_sdk_base::store::StateStore>::save_send_queue_request(
                self,
                &room_id,
                transaction_id,
                created_at,
                request,
                priority,
            )
            .await
            .map_err(crate::error::StoreError::from) // Map sdk::StoreError -> crate::StoreError
        })
    }

    fn update_send_queue_request(
        &self,
        room_id: &RoomId,
        transaction_id: &TransactionId,
        content: QueuedRequestKind,
    ) -> SendQueueFuture<bool> {
        let room_id = room_id.to_owned();
        let transaction_id = transaction_id.to_owned();
        SendQueueFuture::new(async move {
            <Self as matrix_sdk_base::store::StateStore>::update_send_queue_request(
                self,
                &room_id,
                &transaction_id,
                content,
            )
            .await
            .map_err(crate::error::StoreError::from) // Map sdk::StoreError -> crate::StoreError
        })
    }

    fn remove_send_queue_request(
        &self,
        room_id: &RoomId,
        transaction_id: &TransactionId,
    ) -> SendQueueFuture<bool> {
        let room_id = room_id.to_owned();
        let transaction_id = transaction_id.to_owned();
        SendQueueFuture::new(async move {
            <Self as matrix_sdk_base::store::StateStore>::remove_send_queue_request(
                self,
                &room_id,
                &transaction_id,
            )
            .await
            .map_err(crate::error::StoreError::from) // Map sdk::StoreError -> crate::StoreError
        })
    }

    fn load_send_queue_requests(&self, room_id: &RoomId) -> QueuedRequestStream {
        let room_id = room_id.to_owned();
        QueuedRequestStream::new(async_stream::stream! {
            match <Self as matrix_sdk_base::store::StateStore>::load_send_queue_requests(self, &room_id).await {
                Ok(requests) => {
                    for request in requests {
                        yield Ok(request);
                    }
                },
                Err(e) => {
                    yield Err(crate::error::StoreError::from(e)); // Map sdk::StoreError -> crate::StoreError
                }
            }
        })
    }

    fn update_send_queue_request_status(
        &self,
        room_id: &RoomId,
        transaction_id: &TransactionId,
        error: Option<QueueWedgeError>,
    ) -> SendQueueFuture<()> {
        let room_id = room_id.to_owned();
        let transaction_id = transaction_id.to_owned();
        SendQueueFuture::new(async move {
            <Self as matrix_sdk_base::store::StateStore>::update_send_queue_request_status(
                self,
                &room_id,
                &transaction_id,
                error,
            )
            .await
            .map_err(crate::error::StoreError::from) // Map sdk::StoreError -> crate::StoreError
        })
    }

    fn load_rooms_with_unsent_requests(&self) -> RoomIdStream {
        RoomIdStream::new(async_stream::stream! {
            match <Self as matrix_sdk_base::store::StateStore>::load_rooms_with_unsent_requests(self).await {
                Ok(room_ids) => {
                    for room_id in room_ids {
                        yield Ok(room_id);
                    }
                },
                Err(e) => {
                    yield Err(crate::error::StoreError::from(e)); // Map sdk::StoreError -> crate::StoreError
                }
            }
        })
    }

    fn save_dependent_queued_request(
        &self,
        room_id: &RoomId,
        parent_txn_id: &TransactionId,
        own_txn_id: ChildTransactionId,
        created_at: MilliSecondsSinceUnixEpoch,
        content: DependentQueuedRequestKind,
    ) -> DependentQueueFuture<()> {
        let room_id = room_id.to_owned();
        let parent_txn_id = parent_txn_id.to_owned();
        DependentQueueFuture::new(async move {
            <Self as matrix_sdk_base::store::StateStore>::save_dependent_queued_request(
                self,
                &room_id,
                &parent_txn_id,
                own_txn_id,
                created_at,
                content,
            )
            .await
            .map_err(crate::error::StoreError::from) // Map sdk::StoreError -> crate::StoreError
        })
    }

    fn mark_dependent_queued_requests_as_ready(
        &self,
        room_id: &RoomId,
        parent_txn_id: &TransactionId,
        sent_parent_key: SentRequestKey,
    ) -> DependentQueueFuture<usize> {
        let room_id = room_id.to_owned();
        let parent_txn_id = parent_txn_id.to_owned();
        DependentQueueFuture::new(async move {
            <Self as matrix_sdk_base::store::StateStore>::mark_dependent_queued_requests_as_ready(
                self,
                &room_id,
                &parent_txn_id,
                sent_parent_key,
            )
            .await
            .map_err(crate::error::StoreError::from) // Map sdk::StoreError -> crate::StoreError
        })
    }

    fn update_dependent_queued_request(
        &self,
        room_id: &RoomId,
        own_transaction_id: &ChildTransactionId,
        new_content: DependentQueuedRequestKind,
    ) -> DependentQueueFuture<bool> {
        let room_id = room_id.to_owned();
        let own_transaction_id = own_transaction_id.clone();
        DependentQueueFuture::new(async move {
            <Self as matrix_sdk_base::store::StateStore>::update_dependent_queued_request(
                self,
                &room_id,
                &own_transaction_id,
                new_content,
            )
            .await
            .map_err(crate::error::StoreError::from) // Map sdk::StoreError -> crate::StoreError
        })
    }

    fn remove_dependent_queued_request(
        &self,
        room: &RoomId,
        own_txn_id: &ChildTransactionId,
    ) -> DependentQueueFuture<bool> {
        let room_id = room.to_owned();
        let own_txn_id = own_txn_id.clone();
        DependentQueueFuture::new(async move {
            <Self as matrix_sdk_base::store::StateStore>::remove_dependent_queued_request(
                self,
                &room_id,
                &own_txn_id,
            )
            .await
            .map_err(crate::error::StoreError::from) // Map sdk::StoreError -> crate::StoreError
        })
    }

    fn load_dependent_queued_requests(&self, room: &RoomId) -> DependentQueuedRequestStream {
        let room_id = room.to_owned();
        DependentQueuedRequestStream::new(async_stream::stream! {
            match self.load_dependent_queued_requests(&room_id).await {
                Ok(requests) => {
                    for request in requests {
                        yield Ok(request);
                    }
                },
                Err(e) => {
                    yield Err(crate::error::StoreError::from(e)); // Map sdk::StoreError -> crate::StoreError
                }
            }
        })
    }

    fn mark_media_upload_started(&self, request_id: &str) -> MediaUploadFuture<()> {
        let _request_id = request_id.to_owned();
        MediaUploadFuture::new(async move {
            // For media methods, we need to implement them since they might not exist in the StateStore trait
            // This is just a placeholder - we'll need to add actual implementation
            Ok(())
        })
    }

    fn get_media_uploads(&self) -> MediaUploadStream {
        MediaUploadStream::new(async_stream::stream! {
            // For media methods, we need to implement them since they might not exist in the StateStore trait
            // This is just a placeholder - we'll need to add actual implementation
            yield Ok("".to_string());
        })
    }

    fn remove_media_upload(&self, request_id: &str) -> MediaUploadFuture<()> {
        let _request_id = request_id.to_owned();
        MediaUploadFuture::new(async move {
            // For media methods, we need to implement them since they might not exist in the StateStore trait
            // This is just a placeholder - we'll need to add actual implementation
            Ok(())
        })
    }
}
