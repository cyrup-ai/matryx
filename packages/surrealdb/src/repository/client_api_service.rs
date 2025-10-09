use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::repository::RepositoryError;
use crate::repository::capabilities::CapabilitiesRepository;
use crate::repository::device::DeviceRepository;

use crate::repository::notification::{NotificationRepository, NotificationResponse};
use crate::repository::search::SearchRepository;
use crate::repository::sync::{Filter, SyncRepository, SyncResponse};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub search_categories: SearchCategories,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchCategories {
    pub room_events: Option<RoomEventsCriteria>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEventsCriteria {
    pub search_term: String,
    pub keys: Option<Vec<String>>,
    pub filter: Option<RoomEventFilter>,
    pub order_by: Option<String>,
    pub event_context: Option<EventContext>,
    pub include_state: Option<bool>,
    pub groupings: Option<Groupings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEventFilter {
    pub limit: Option<u64>,
    pub not_senders: Option<Vec<String>>,
    pub not_types: Option<Vec<String>>,
    pub senders: Option<Vec<String>>,
    pub types: Option<Vec<String>>,
    pub rooms: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventContext {
    pub before_limit: Option<u64>,
    pub after_limit: Option<u64>,
    pub include_profile: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Groupings {
    pub group_by: Option<Vec<GroupBy>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupBy {
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub search_categories: SearchResultCategories,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultCategories {
    pub room_events: Option<RoomEventsResults>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEventsResults {
    pub results: Vec<SearchResult>,
    pub count: Option<u64>,
    pub highlights: Vec<String>,
    pub next_batch: Option<String>,
    pub groups: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub rank: f64,
    pub result: Value,
    pub context: Option<SearchResultContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultContext {
    pub events_before: Vec<Value>,
    pub events_after: Vec<Value>,
    pub start: String,
    pub end: String,
    pub profile_info: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilitiesResponse {
    pub capabilities: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceListResponse {
    pub devices: Vec<DeviceInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_id: String,
    pub display_name: Option<String>,
    pub last_seen_ip: Option<String>,
    pub last_seen_ts: Option<u64>,
    pub user_id: String,
    pub created_ts: u64,
    pub device_keys: Option<Value>,
    pub trust_level: String,
    pub is_deleted: bool,
}

pub struct ClientApiService {
    search_repo: SearchRepository,
    notification_repo: NotificationRepository,
    capabilities_repo: CapabilitiesRepository,
    sync_repo: SyncRepository,
    device_repo: DeviceRepository,
}

impl ClientApiService {
    pub fn new(
        search_repo: SearchRepository,
        notification_repo: NotificationRepository,
        capabilities_repo: CapabilitiesRepository,
        sync_repo: SyncRepository,
        device_repo: DeviceRepository,
    ) -> Self {
        Self {
            search_repo,
            notification_repo,
            capabilities_repo,
            sync_repo,
            device_repo,
        }
    }

    pub async fn handle_search_request(
        &self,
        user_id: &str,
        search_request: &SearchRequest,
    ) -> Result<SearchResponse, RepositoryError> {
        // Convert search request to search criteria
        let search_criteria = if let Some(room_events) =
            &search_request.search_categories.room_events
        {
            crate::repository::search::SearchCriteria {
                search_term: room_events.search_term.clone(),
                room_events: Some(crate::repository::search::RoomEventsCriteria {
                    search_term: room_events.search_term.clone(),
                    keys: room_events.keys.clone(),
                    filter: room_events.filter.as_ref().map(|f| {
                        crate::repository::search::RoomEventFilter {
                            limit: f.limit,
                            not_senders: f.not_senders.clone(),
                            not_types: f.not_types.clone(),
                            senders: f.senders.clone(),
                            types: f.types.clone(),
                            rooms: f.rooms.clone(),
                        }
                    }),
                    order_by: room_events.order_by.clone(),
                    event_context: room_events.event_context.as_ref().map(|ec| {
                        crate::repository::search::EventContext {
                            before_limit: ec.before_limit,
                            after_limit: ec.after_limit,
                            include_profile: ec.include_profile,
                        }
                    }),
                    include_state: room_events.include_state,
                    groupings: room_events.groupings.as_ref().map(|g| {
                        crate::repository::search::SearchGroupings {
                            group_by: g.group_by.as_ref().map(|gb| {
                                gb.iter()
                                    .map(|item| {
                                        crate::repository::search::GroupBy { key: item.key.clone() }
                                    })
                                    .collect()
                            }),
                        }
                    }),
                }),
                order_by: None,
                event_context: None,
                include_state: false,
                groupings: None,
            }
        } else {
            return Err(RepositoryError::ValidationError {
                field: "search_categories".to_string(),
                message: "No search categories provided".to_string(),
            });
        };

        // Perform search
        let search_results = self.search_repo.search_events(user_id, &search_criteria).await?;

        // Convert results to response format
        Ok(SearchResponse {
            search_categories: SearchResultCategories {
                room_events: search_results.search_categories.room_events.map(|re| {
                    RoomEventsResults {
                        results: re
                            .results
                            .into_iter()
                            .map(|sr| {
                                SearchResult {
                                    rank: sr.rank,
                                    result: sr.result,
                                    context: sr.context.map(|ctx| {
                                        SearchResultContext {
                                            events_before: ctx.events_before,
                                            events_after: ctx.events_after,
                                            start: ctx.start,
                                            end: ctx.end,
                                            profile_info: ctx.profile_info,
                                        }
                                    }),
                                }
                            })
                            .collect(),
                        count: re.count,
                        highlights: re.highlights,
                        next_batch: re.next_batch,
                        groups: re.groups,
                    }
                }),
            },
        })
    }

    pub async fn handle_notification_request(
        &self,
        user_id: &str,
        from: Option<&str>,
        limit: Option<u32>,
        only: Option<&str>,
    ) -> Result<NotificationResponse, RepositoryError> {
        self.notification_repo.get_user_notifications(user_id, from, limit, only).await
    }

    pub async fn handle_capabilities_request(
        &self,
    ) -> Result<CapabilitiesResponse, RepositoryError> {
        let capabilities = self.capabilities_repo.get_server_capabilities().await?;
        let unstable_features = self.capabilities_repo.get_unstable_features().await?;

        let mut capabilities_map = HashMap::new();

        // Add standard capabilities
        capabilities_map.insert(
            "m.change_password".to_string(),
            serde_json::json!({"enabled": capabilities.change_password}),
        );
        capabilities_map.insert(
            "m.room_versions".to_string(),
            serde_json::json!({
                "default": capabilities.room_versions.default,
                "available": capabilities.room_versions.available
            }),
        );
        capabilities_map.insert(
            "m.set_displayname".to_string(),
            serde_json::json!({"enabled": capabilities.set_displayname}),
        );
        capabilities_map.insert(
            "m.set_avatar_url".to_string(),
            serde_json::json!({"enabled": capabilities.set_avatar_url}),
        );
        capabilities_map.insert(
            "m.3pid_changes".to_string(),
            serde_json::json!({"enabled": capabilities.threepid_changes}),
        );
        capabilities_map.insert(
            "m.get_login_token".to_string(),
            serde_json::json!({"enabled": capabilities.get_login_token}),
        );
        capabilities_map.insert(
            "org.matrix.lazy_loading".to_string(),
            serde_json::json!({"enabled": capabilities.lazy_loading}),
        );
        capabilities_map.insert(
            "org.matrix.e2e_cross_signing".to_string(),
            serde_json::json!({"enabled": capabilities.e2e_cross_signing}),
        );
        capabilities_map.insert(
            "org.matrix.spaces".to_string(),
            serde_json::json!({"enabled": capabilities.spaces}),
        );
        capabilities_map.insert(
            "org.matrix.threading".to_string(),
            serde_json::json!({"enabled": capabilities.threading}),
        );

        // Add custom capabilities
        for (key, value) in capabilities.custom_capabilities {
            capabilities_map.insert(key, value);
        }

        // Add unstable features
        for (feature, enabled) in unstable_features {
            capabilities_map.insert(feature, serde_json::json!({"enabled": enabled}));
        }

        Ok(CapabilitiesResponse { capabilities: capabilities_map })
    }

    pub async fn handle_sync_request(
        &self,
        user_id: &str,
        since: Option<&str>,
        filter: Option<&Filter>,
    ) -> Result<SyncResponse, RepositoryError> {
        if let Some(since_token) = since {
            self.sync_repo
                .get_incremental_sync_data(user_id, since_token, filter)
                .await
        } else {
            // Convert initial sync to sync response format
            let initial_sync = self.sync_repo.get_initial_sync_data(user_id, filter).await?;

            let mut join_rooms = HashMap::new();
            for room in initial_sync.rooms {
                let joined_room = crate::repository::sync::JoinedRoomSync {
                    state: crate::repository::sync::StateSync {
                        events: room.state,
                    },
                    timeline: room.timeline,
                    ephemeral: room.ephemeral,
                    account_data: crate::repository::sync::AccountDataSync {
                        events: room.account_data,
                    },
                    unread_notifications: room.unread_notifications,
                    summary: room.summary,
                };
                join_rooms.insert(room.room_id, joined_room);
            }

            Ok(SyncResponse {
                next_batch: initial_sync.next_batch,
                rooms: crate::repository::sync::RoomsSyncData {
                    join: join_rooms,
                    invite: HashMap::new(),
                    leave: HashMap::new(),
                    knock: HashMap::new(),
                },
                presence: Some(initial_sync.presence),
                account_data: Some(initial_sync.account_data),
                to_device: None,
                device_lists: None,
                device_one_time_keys_count: Some(HashMap::new()),
                device_unused_fallback_key_types: Some(vec![]),
            })
        }
    }

    pub async fn handle_device_list_request(
        &self,
        user_id: &str,
    ) -> Result<DeviceListResponse, RepositoryError> {
        let devices = self.device_repo.get_user_devices_list(user_id).await?;

        let device_infos = devices
            .into_iter()
            .map(|device| {
                DeviceInfo {
                    device_id: device.device_id,
                    display_name: device.display_name,
                    last_seen_ip: device.last_seen_ip,
                    last_seen_ts: device.last_seen_ts.map(|ts| ts as u64),
                    user_id: user_id.to_string(),
                    created_ts: device.created_at.timestamp() as u64,
                    device_keys: device.device_keys,
                    trust_level: "unverified".to_string(), // Default trust level
                    is_deleted: device.hidden.unwrap_or(false),
                }
            })
            .collect();

        Ok(DeviceListResponse { devices: device_infos })
    }

    pub async fn validate_client_request(
        &self,
        user_id: &str,
        request_type: &str,
    ) -> Result<bool, RepositoryError> {
        // Get user capabilities to validate if they can perform this request type
        let user_capabilities = self.capabilities_repo.get_user_capabilities(user_id).await?;

        let allowed = match request_type {
            "search" => true,        // All authenticated users can search
            "notifications" => true, // All authenticated users can get notifications
            "sync" => true,          // All authenticated users can sync
            "device_list" => true,   // All authenticated users can list their devices
            "capabilities" => true,  // All users can check capabilities
            "create_room" => user_capabilities.can_create_rooms,
            "invite_users" => user_capabilities.can_invite_users,
            "join_public_rooms" => user_capabilities.can_join_public_rooms,
            "change_password" => user_capabilities.can_change_password,
            "set_displayname" => user_capabilities.can_set_displayname,
            "set_avatar_url" => user_capabilities.can_set_avatar_url,
            _ => false, // Unknown request types are not allowed
        };

        Ok(allowed)
    }
}
