use crate::repository::error::RepositoryError;
use crate::repository::{FederationRepository, MembershipRepository, MediaRepository, PublicRoomsRepository, RoomRepository, EventRepository};
use crate::repository::federation::{JoinResult, LeaveResult, KnockResult, StateIdsResponse, BackfillResponse, ThirdPartyInvite};
use crate::repository::media::MediaInfo;
use crate::repository::public_rooms::{PublicRoomsResponse, PublicRoomInfo};
use matryx_entity::types::Event;
use serde::{Deserialize, Serialize};
use surrealdb::{Connection, Surreal};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MakeJoinResponse {
    pub event: Event,
    pub room_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendJoinResponse {
    pub state: Vec<Event>,
    pub auth_chain: Vec<Event>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MakeLeaveResponse {
    pub event: Event,
    pub room_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendLeaveResponse {
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MakeKnockResponse {
    pub event: Event,
    pub room_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendKnockResponse {
    pub knock_state: Vec<Event>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteResponse {
    pub event: Event,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingEventsResponse {
    pub events: Vec<Event>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaResponse {
    pub content: Vec<u8>,
    pub content_type: String,
    pub content_length: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThumbnailResponse {
    pub thumbnail: Vec<u8>,
    pub content_type: String,
}

pub struct FederationManagementService {
    federation_repo: FederationRepository,
    membership_repo: MembershipRepository,
    media_repo: MediaRepository,
    public_rooms_repo: PublicRoomsRepository,
    room_repo: RoomRepository,
    event_repo: EventRepository,
}

impl FederationManagementService {
    pub fn new(
        federation_repo: FederationRepository,
        membership_repo: MembershipRepository,
        media_repo: MediaRepository,
        public_rooms_repo: PublicRoomsRepository,
        room_repo: RoomRepository,
        event_repo: EventRepository,
    ) -> Self {
        Self {
            federation_repo,
            membership_repo,
            media_repo,
            public_rooms_repo,
            room_repo,
            event_repo,
        }
    }

    /// Handle make join request
    pub async fn handle_make_join(&self, room_id: &str, user_id: &str, room_version: &str) -> Result<MakeJoinResponse, RepositoryError> {
        // Validate room exists
        if self.room_repo.get_room_by_id(room_id).await?.is_none() {
            return Err(RepositoryError::NotFound {
                entity_type: "Room".to_string(),
                id: room_id.to_string(),
            });
        }

        // Validate room version
        let actual_room_version = self.room_repo.get_room_version(room_id).await?;
        
        // Create join event template
        let event = self.federation_repo.make_join_event(room_id, user_id, &actual_room_version).await?;

        Ok(MakeJoinResponse {
            event,
            room_version: actual_room_version,
        })
    }

    /// Handle send join request
    pub async fn handle_send_join(&self, room_id: &str, event_id: &str, event: &Event, origin: &str) -> Result<SendJoinResponse, RepositoryError> {
        // Validate and process the join event
        let join_result = self.federation_repo.process_join_event(room_id, event, origin).await?;

        // Process membership change
        if let Some(target_user) = &event.state_key {
            self.membership_repo.process_federation_join(room_id, target_user, event, origin).await?;
        }

        // Store the event
        self.event_repo.store_event_with_hash(event).await?;

        Ok(SendJoinResponse {
            state: join_result.state,
            auth_chain: join_result.auth_chain,
        })
    }

    /// Handle make leave request
    pub async fn handle_make_leave(&self, room_id: &str, user_id: &str, room_version: &str) -> Result<MakeLeaveResponse, RepositoryError> {
        // Validate room exists
        if self.room_repo.get_room_by_id(room_id).await?.is_none() {
            return Err(RepositoryError::NotFound {
                entity_type: "Room".to_string(),
                id: room_id.to_string(),
            });
        }

        // Validate user is in room
        if !self.membership_repo.is_user_in_room(room_id, user_id).await? {
            return Err(RepositoryError::Validation {
                field: "user_id".to_string(),
                message: "User is not in room".to_string(),
            });
        }

        let actual_room_version = self.room_repo.get_room_version(room_id).await?;
        let event = self.federation_repo.make_leave_event(room_id, user_id, &actual_room_version).await?;

        Ok(MakeLeaveResponse {
            event,
            room_version: actual_room_version,
        })
    }

    /// Handle send leave request
    pub async fn handle_send_leave(&self, room_id: &str, event_id: &str, event: &Event, origin: &str) -> Result<SendLeaveResponse, RepositoryError> {
        // Validate and process the leave event
        let _leave_result = self.federation_repo.process_leave_event(room_id, event, origin).await?;

        // Process membership change
        if let Some(target_user) = &event.state_key {
            self.membership_repo.process_federation_leave(room_id, target_user, event, origin).await?;
        }

        // Store the event
        self.event_repo.store_event_with_hash(event).await?;

        Ok(SendLeaveResponse {
            success: true,
        })
    }

    /// Handle make knock request
    pub async fn handle_make_knock(&self, room_id: &str, user_id: &str, room_version: &str) -> Result<MakeKnockResponse, RepositoryError> {
        // Validate room exists
        if self.room_repo.get_room_by_id(room_id).await?.is_none() {
            return Err(RepositoryError::NotFound {
                entity_type: "Room".to_string(),
                id: room_id.to_string(),
            });
        }

        // Check if room allows knocking
        let join_rules = self.room_repo.get_room_join_rules(room_id).await?;
        if !matches!(join_rules, crate::repository::room::JoinRules::Knock) {
            return Err(RepositoryError::Validation {
                field: "join_rules".to_string(),
                message: "Room does not allow knocking".to_string(),
            });
        }

        let actual_room_version = self.room_repo.get_room_version(room_id).await?;
        let event = self.federation_repo.make_knock_event(room_id, user_id, &actual_room_version).await?;

        Ok(MakeKnockResponse {
            event,
            room_version: actual_room_version,
        })
    }

    /// Handle send knock request
    pub async fn handle_send_knock(&self, room_id: &str, event_id: &str, event: &Event, origin: &str) -> Result<SendKnockResponse, RepositoryError> {
        // Validate and process the knock event
        let knock_result = self.federation_repo.process_knock_event(room_id, event, origin).await?;

        // Process membership change
        if let Some(target_user) = &event.state_key {
            self.membership_repo.process_federation_knock(room_id, target_user, event, origin).await?;
        }

        // Store the event
        self.event_repo.store_event_with_hash(event).await?;

        Ok(SendKnockResponse {
            knock_state: knock_result.knock_state,
        })
    }

    /// Handle invite request
    pub async fn handle_invite(&self, room_id: &str, event_id: &str, event: &Event, origin: &str) -> Result<InviteResponse, RepositoryError> {
        // Validate the invite event
        let validation = self.federation_repo.validate_pdu(event, origin).await?;
        if !validation.valid {
            return Err(RepositoryError::Validation {
                field: "event".to_string(),
                message: validation.reason.unwrap_or("Invalid invite event".to_string()),
            });
        }

        // Process membership change
        if let Some(target_user) = &event.state_key {
            self.membership_repo.process_federation_invite(room_id, target_user, event, origin).await?;
        }

        // Store the event
        self.event_repo.store_event_with_hash(event).await?;

        Ok(InviteResponse {
            event: event.clone(),
        })
    }

    /// Handle state IDs request
    pub async fn handle_state_ids(&self, room_id: &str, event_id: &str) -> Result<StateIdsResponse, RepositoryError> {
        // Validate room exists
        if self.room_repo.get_room_by_id(room_id).await?.is_none() {
            return Err(RepositoryError::NotFound {
                entity_type: "Room".to_string(),
                id: room_id.to_string(),
            });
        }

        self.federation_repo.get_room_state_ids_at_event(room_id, event_id).await
    }

    /// Handle backfill request
    pub async fn handle_backfill(&self, room_id: &str, event_ids: &[String], limit: u32) -> Result<BackfillResponse, RepositoryError> {
        // Validate room exists
        if self.room_repo.get_room_by_id(room_id).await?.is_none() {
            return Err(RepositoryError::NotFound {
                entity_type: "Room".to_string(),
                id: room_id.to_string(),
            });
        }

        self.federation_repo.backfill_events(room_id, event_ids, limit).await
    }

    /// Handle get missing events request
    pub async fn handle_get_missing_events(&self, room_id: &str, earliest: &[String], latest: &[String], limit: u32) -> Result<MissingEventsResponse, RepositoryError> {
        // Validate room exists
        if self.room_repo.get_room_by_id(room_id).await?.is_none() {
            return Err(RepositoryError::NotFound {
                entity_type: "Room".to_string(),
                id: room_id.to_string(),
            });
        }

        let events = self.federation_repo.get_missing_events(room_id, earliest, latest, limit).await?;

        Ok(MissingEventsResponse { events })
    }

    /// Handle public rooms request
    pub async fn handle_public_rooms(&self, server_name: Option<&str>, limit: Option<u32>, since: Option<&str>) -> Result<PublicRoomsResponse, RepositoryError> {
        self.public_rooms_repo.get_public_rooms(server_name, limit, since).await
    }

    /// Handle media download request
    pub async fn handle_media_download(&self, media_id: &str, server_name: &str) -> Result<MediaResponse, RepositoryError> {
        // Validate media access
        if !self.media_repo.validate_media_access(media_id, server_name, "localhost").await? {
            return Err(RepositoryError::Unauthorized {
                reason: "Access denied to media".to_string(),
            });
        }

        // Get media info
        let media_info = self.media_repo.get_media_info(media_id, server_name).await?
            .ok_or_else(|| RepositoryError::NotFound {
                entity_type: "Media".to_string(),
                id: format!("{}:{}", server_name, media_id),
            })?;

        // Get media content
        let content = self.media_repo.get_media_content(media_id, server_name).await?
            .ok_or_else(|| RepositoryError::NotFound {
                entity_type: "Media content".to_string(),
                id: format!("{}:{}", server_name, media_id),
            })?;

        Ok(MediaResponse {
            content,
            content_type: media_info.content_type,
            content_length: media_info.content_length,
        })
    }

    /// Handle media thumbnail request
    pub async fn handle_media_thumbnail(&self, media_id: &str, server_name: &str, width: u32, height: u32, method: &str) -> Result<ThumbnailResponse, RepositoryError> {
        // Validate media access
        if !self.media_repo.validate_media_access(media_id, server_name, "localhost").await? {
            return Err(RepositoryError::Unauthorized {
                reason: "Access denied to media".to_string(),
            });
        }

        // Get thumbnail
        let thumbnail = self.media_repo.get_media_thumbnail(media_id, server_name, width, height, method).await?
            .ok_or_else(|| RepositoryError::NotFound {
                entity_type: "Media thumbnail".to_string(),
                id: format!("{}:{}:{}x{}:{}", server_name, media_id, width, height, method),
            })?;

        Ok(ThumbnailResponse {
            thumbnail,
            content_type: "image/jpeg".to_string(), // Would determine from actual thumbnail
        })
    }

    /// Handle third-party invite exchange
    pub async fn handle_exchange_third_party_invite(&self, room_id: &str, invite: &ThirdPartyInvite) -> Result<Event, RepositoryError> {
        // Validate room exists
        if self.room_repo.get_room_by_id(room_id).await?.is_none() {
            return Err(RepositoryError::NotFound {
                entity_type: "Room".to_string(),
                id: room_id.to_string(),
            });
        }

        // Exchange the invite
        let event = self.federation_repo.exchange_third_party_invite(room_id, invite).await?;

        // Store the event
        self.event_repo.store_event_with_hash(&event).await?;

        Ok(event)
    }

    /// Handle query request
    pub async fn handle_query(&self, query_type: &str, query_params: &serde_json::Value) -> Result<serde_json::Value, RepositoryError> {
        match query_type {
            "profile" => {
                // Handle user profile query
                if let Some(user_id) = query_params.get("user_id").and_then(|v| v.as_str()) {
                    // Would query user profile from user repository
                    Ok(serde_json::json!({
                        "user_id": user_id,
                        "displayname": null,
                        "avatar_url": null
                    }))
                } else {
                    Err(RepositoryError::Validation {
                        field: "user_id".to_string(),
                        message: "Missing user_id parameter".to_string(),
                    })
                }
            },
            "directory" => {
                // Handle room directory query
                if let Some(room_alias) = query_params.get("room_alias").and_then(|v| v.as_str()) {
                    // Would resolve room alias
                    Ok(serde_json::json!({
                        "room_id": null,
                        "servers": []
                    }))
                } else {
                    Err(RepositoryError::Validation {
                        field: "room_alias".to_string(),
                        message: "Missing room_alias parameter".to_string(),
                    })
                }
            },
            _ => {
                Err(RepositoryError::Validation {
                    field: "query_type".to_string(),
                    message: format!("Unsupported query type: {}", query_type),
                })
            }
        }
    }

    /// Handle event request
    pub async fn handle_get_event(&self, event_id: &str) -> Result<Event, RepositoryError> {
        let query = "SELECT * FROM event WHERE event_id = $event_id LIMIT 1";
        let mut result = self.federation_repo.db
            .query(query)
            .bind(("event_id", event_id.to_string()))
            .await?;
        let events: Vec<Event> = result.take(0)?;

        events.into_iter().next().ok_or_else(|| {
            RepositoryError::NotFound {
                entity_type: "Event".to_string(),
                id: event_id.to_string(),
            }
        })
    }

    /// Validate federation request
    pub async fn validate_federation_request(&self, origin: &str, destination: &str, request_id: &str) -> Result<bool, RepositoryError> {
        self.federation_repo.validate_federation_request(origin, destination, request_id).await
    }

    /// Check room federation ACL
    pub async fn check_room_federation_acl(&self, room_id: &str, server_name: &str) -> Result<bool, RepositoryError> {
        self.federation_repo.check_federation_acl(room_id, server_name).await
    }
}