use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use surrealdb::{Surreal, engine::any::Any};

use crate::repository::RepositoryError;

// Type aliases for complex tuple types to satisfy clippy::type_complexity
type SearchResultTuple = (String, String, String, String, Option<String>, String, Option<f64>);
type RoomDataTuple = (String, Option<String>, Option<String>, Option<String>, Option<u64>, Option<bool>, Option<bool>, Option<String>, Option<String>);
type UserDataTuple = (String, Option<String>, Option<String>, Option<bool>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub search_categories: SearchCriteria,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub search_categories: SearchResults,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDirectoryResponse {
    pub results: Vec<UserDirectoryResult>,
    pub limited: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDirectoryResult {
    pub user_id: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchCriteria {
    pub search_term: String,
    pub room_events: Option<RoomEventsCriteria>,
    pub order_by: Option<SearchOrderBy>,
    pub event_context: Option<EventContext>,
    pub include_state: bool,
    pub groupings: Option<SearchGroupings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEventsCriteria {
    pub search_term: String,
    pub keys: Option<Vec<String>>,
    pub filter: Option<RoomEventFilter>,
    pub order_by: Option<String>,
    pub event_context: Option<EventContext>,
    pub include_state: Option<bool>,
    pub groupings: Option<SearchGroupings>,
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
pub struct SearchGroupings {
    pub group_by: Option<Vec<GroupBy>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupBy {
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchOrderBy {
    pub field: String,
    pub direction: SearchDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResults {
    pub search_categories: SearchCategories,
    pub next_batch: Option<String>,
    pub highlights: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchCategories {
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
pub struct RoomSearchResult {
    pub room_id: String,
    pub name: Option<String>,
    pub topic: Option<String>,
    pub canonical_alias: Option<String>,
    pub num_joined_members: u64,
    pub world_readable: bool,
    pub guest_can_join: bool,
    pub join_rule: String,
    pub room_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSearchResult {
    pub user_id: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchIndexResult {
    pub indexed_events: u64,
    pub duration_ms: u64,
}

pub struct SearchRepository {
    db: Surreal<Any>,
}

impl SearchRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    pub async fn search_events(
        &self,
        user_id: &str,
        search_criteria: &SearchCriteria,
    ) -> Result<SearchResults, RepositoryError> {
        // Get user's accessible rooms first
        let accessible_rooms = self.get_user_accessible_rooms(user_id).await?;

        if accessible_rooms.is_empty() {
            return Ok(SearchResults {
                search_categories: SearchCategories {
                    room_events: Some(RoomEventsResults {
                        results: vec![],
                        count: Some(0),
                        highlights: vec![],
                        next_batch: None,
                        groups: None,
                    }),
                },
                next_batch: None,
                highlights: vec![search_criteria.search_term.clone()],
            });
        }

        // Build search query based on criteria
        let mut query = String::from(
            "SELECT event_id, room_id, sender, event_type, content_body, origin_server_ts, rank
             FROM search_index 
             WHERE room_id IN $rooms AND content_body CONTAINS $search_term",
        );

        // Apply additional filters if present
        if let Some(room_events) = &search_criteria.room_events
            && let Some(filter) = &room_events.filter {
                if filter.types.is_some() {
                    query.push_str(" AND event_type IN $types");
                }
                if filter.not_types.is_some() {
                    query.push_str(" AND event_type NOT IN $not_types");
                }
                if filter.senders.is_some() {
                    query.push_str(" AND sender IN $senders");
                }
                if filter.not_senders.is_some() {
                    query.push_str(" AND sender NOT IN $not_senders");
                }
                if filter.rooms.is_some() {
                    query.push_str(" AND room_id IN $filter_rooms");
                }
            }

        // Add ordering
        query.push_str(" ORDER BY origin_server_ts DESC");

        // Add limit
        query.push_str(" LIMIT 50");

        let mut db_query = self
            .db
            .query(&query)
            .bind(("rooms", accessible_rooms.clone()))
            .bind(("search_term", search_criteria.search_term.clone()));

        // Bind additional filter parameters
        if let Some(room_events) = &search_criteria.room_events
            && let Some(filter) = &room_events.filter {
                if let Some(types) = &filter.types {
                    db_query = db_query.bind(("types", types.clone()));
                }
                if let Some(not_types) = &filter.not_types {
                    db_query = db_query.bind(("not_types", not_types.clone()));
                }
                if let Some(senders) = &filter.senders {
                    db_query = db_query.bind(("senders", senders.clone()));
                }
                if let Some(not_senders) = &filter.not_senders {
                    db_query = db_query.bind(("not_senders", not_senders.clone()));
                }
                if let Some(rooms) = &filter.rooms {
                    db_query = db_query.bind(("filter_rooms", rooms.clone()));
                }
            }

        let mut response = db_query.await.map_err(RepositoryError::Database)?;

        let search_rows: Vec<SearchResultTuple> = response.take(0).map_err(RepositoryError::Database)?;

        // Convert to search results format
        let mut results = Vec::new();
        for (event_id, _room_id, _sender, _event_type, _content_body, _origin_server_ts, rank) in
            search_rows
        {
            // Get the full event from events table
            let event = self.get_full_event(&event_id).await?;
            if let Some(event_data) = event {
                results.push(SearchResult {
                    rank: rank.unwrap_or(1.0),
                    result: event_data,
                    context: None, // Context can be added later if needed
                });
            }
        }

        let highlights = vec![search_criteria.search_term.clone()];

        Ok(SearchResults {
            search_categories: SearchCategories {
                room_events: Some(RoomEventsResults {
                    count: Some(results.len() as u64),
                    results,
                    highlights: highlights.clone(),
                    next_batch: None,
                    groups: None,
                }),
            },
            next_batch: None,
            highlights,
        })
    }

    pub async fn search_rooms(
        &self,
        user_id: &str,
        query: &str,
        limit: Option<u32>,
    ) -> Result<Vec<RoomSearchResult>, RepositoryError> {
        let search_query = r#"
            SELECT room_id, name, topic, canonical_alias, num_joined_members, 
                   world_readable, guest_can_join, join_rule, room_type
            FROM rooms 
            WHERE (name CONTAINS $query OR topic CONTAINS $query OR canonical_alias CONTAINS $query)
            AND (visibility = 'public' OR room_id IN (
                SELECT room_id FROM membership 
                WHERE user_id = $user_id AND membership IN ['join', 'invite']
            ))
            LIMIT $limit
        "#;

        let mut response = self
            .db
            .query(search_query)
            .bind(("query", query.to_string()))
            .bind(("user_id", user_id.to_string()))
            .bind(("limit", limit.unwrap_or(50)))
            .await
            .map_err(RepositoryError::Database)?;

        let rooms: Vec<RoomDataTuple> = response.take(0).map_err(RepositoryError::Database)?;

        Ok(rooms
            .into_iter()
            .map(
                |(
                    room_id,
                    name,
                    topic,
                    canonical_alias,
                    num_joined_members,
                    world_readable,
                    guest_can_join,
                    join_rule,
                    room_type,
                )| {
                    RoomSearchResult {
                        room_id,
                        name,
                        topic,
                        canonical_alias,
                        num_joined_members: num_joined_members.unwrap_or(0),
                        world_readable: world_readable.unwrap_or(false),
                        guest_can_join: guest_can_join.unwrap_or(false),
                        join_rule: join_rule.unwrap_or_else(|| "invite".to_string()),
                        room_type,
                    }
                },
            )
            .collect())
    }

    pub async fn search_users(
        &self,
        user_id: &str,
        query: &str,
        limit: Option<u32>,
    ) -> Result<Vec<UserSearchResult>, RepositoryError> {
        let search_query = r#"
            SELECT user_id, display_name, avatar_url
            FROM users 
            WHERE (user_id CONTAINS $query OR display_name CONTAINS $query)
            AND active = true
            AND user_id != $requesting_user_id
            LIMIT $limit
        "#;

        let mut response = self
            .db
            .query(search_query)
            .bind(("query", query.to_string()))
            .bind(("requesting_user_id", user_id.to_string()))
            .bind(("limit", limit.unwrap_or(50)))
            .await
            .map_err(RepositoryError::Database)?;

        let users: Vec<(String, Option<String>, Option<String>)> =
            response.take(0).map_err(RepositoryError::Database)?;

        Ok(users
            .into_iter()
            .map(|(user_id, display_name, avatar_url)| {
                UserSearchResult { user_id, display_name, avatar_url }
            })
            .collect())
    }

    pub async fn index_event_for_search(
        &self,
        event: &Value,
        room_id: &str,
    ) -> Result<(), RepositoryError> {
        let event_id = event.get("event_id").and_then(|v| v.as_str()).ok_or_else(|| {
            RepositoryError::Validation {
                field: "event_id".to_string(),
                message: "Missing event_id in event".to_string(),
            }
        })?;

        let sender = event.get("sender").and_then(|v| v.as_str()).unwrap_or("");

        let event_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("");

        let content_body =
            event.get("content").and_then(|c| c.get("body")).and_then(|b| b.as_str());

        let origin_server_ts = event.get("origin_server_ts").and_then(|v| v.as_u64()).unwrap_or(0);

        let insert_query = r#"
            CREATE search_index CONTENT {
                event_id: $event_id,
                room_id: $room_id,
                sender: $sender,
                event_type: $event_type,
                content_body: $content_body,
                origin_server_ts: $origin_server_ts,
                indexed_at: time::now(),
                rank: 1.0
            }
        "#;

        self.db
            .query(insert_query)
            .bind(("event_id", event_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .bind(("sender", sender.to_string()))
            .bind(("event_type", event_type.to_string()))
            .bind(("content_body", content_body.map(|s| s.to_string()).unwrap_or_default()))
            .bind(("origin_server_ts", origin_server_ts))
            .await
            .map_err(RepositoryError::Database)?;

        Ok(())
    }

    pub async fn remove_event_from_search(&self, event_id: &str) -> Result<(), RepositoryError> {
        let delete_query = "DELETE search_index WHERE event_id = $event_id";

        self.db
            .query(delete_query)
            .bind(("event_id", event_id.to_string()))
            .await
            .map_err(RepositoryError::Database)?;

        Ok(())
    }

    pub async fn get_search_suggestions(
        &self,
        user_id: &str,
        partial_query: &str,
    ) -> Result<Vec<String>, RepositoryError> {
        let suggestions_query = r#"
            SELECT DISTINCT content_body
            FROM search_index 
            WHERE content_body CONTAINS $partial_query
            AND room_id IN (
                SELECT room_id FROM membership 
                WHERE user_id = $user_id AND membership IN ['join', 'invite']
            )
            LIMIT 10
        "#;

        let mut response = self
            .db
            .query(suggestions_query)
            .bind(("partial_query", partial_query.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await
            .map_err(RepositoryError::Database)?;

        let suggestions: Vec<(Option<String>,)> =
            response.take(0).map_err(RepositoryError::Database)?;

        Ok(suggestions.into_iter().filter_map(|(content,)| content).collect())
    }

    pub async fn update_search_index(
        &self,
        room_id: &str,
    ) -> Result<SearchIndexResult, RepositoryError> {
        let start_time = std::time::Instant::now();

        // Get all events for the room
        let events_query = "SELECT * FROM events WHERE room_id = $room_id";

        let mut response = self
            .db
            .query(events_query)
            .bind(("room_id", room_id.to_string()))
            .await
            .map_err(RepositoryError::Database)?;

        let events: Vec<Value> = response.take(0).map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "update_search_index_parse_events".to_string(),
            }
        })?;

        // Index each event
        let mut indexed_count = 0u64;
        for event in events {
            if (self.index_event_for_search(&event, room_id).await).is_err() {
                // Continue indexing other events even if one fails
                continue;
            }
            indexed_count += 1;
        }

        let duration = start_time.elapsed();

        Ok(SearchIndexResult {
            indexed_events: indexed_count,
            duration_ms: duration.as_millis() as u64,
        })
    }

    pub async fn cleanup_search_index(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<u64, RepositoryError> {
        let cleanup_query = "DELETE search_index WHERE indexed_at < $cutoff";

        let _response = self
            .db
            .query(cleanup_query)
            .bind(("cutoff", cutoff))
            .await
            .map_err(RepositoryError::Database)?;

        // Note: SurrealDB doesn't return count of deleted records by default
        // This is a simplified implementation
        Ok(0)
    }

    // Helper methods

    async fn get_user_accessible_rooms(
        &self,
        user_id: &str,
    ) -> Result<Vec<String>, RepositoryError> {
        let rooms_query = r#"
            SELECT room_id FROM room_members 
            WHERE user_id = $user_id AND membership = 'join'
        "#;

        let mut response = self
            .db
            .query(rooms_query)
            .bind(("user_id", user_id.to_string()))
            .await
            .map_err(RepositoryError::Database)?;

        let rooms: Vec<(String,)> = response.take(0).map_err(RepositoryError::Database)?;

        Ok(rooms.into_iter().map(|(room_id,)| room_id).collect())
    }

    async fn get_full_event(&self, event_id: &str) -> Result<Option<Value>, RepositoryError> {
        let event_query = "SELECT * FROM events WHERE event_id = $event_id";

        let mut response = self
            .db
            .query(event_query)
            .bind(("event_id", event_id.to_string()))
            .await
            .map_err(RepositoryError::Database)?;

        let events: Vec<Value> = response.take(0).map_err(RepositoryError::Database)?;

        Ok(events.into_iter().next())
    }

    /// Get user directory for searching users
    pub async fn get_user_directory(
        &self,
        search_term: &str,
        limit: Option<u32>,
    ) -> Result<matryx_entity::types::UserDirectoryResponse, RepositoryError> {
        let query = r#"
            SELECT user_id, display_name, avatar_url, is_guest
            FROM users
            WHERE display_name CONTAINS $search_term
            OR user_id CONTAINS $search_term
            ORDER BY display_name
            LIMIT $limit
        "#;

        let mut response = self
            .db
            .query(query)
            .bind(("search_term", search_term.to_string()))
            .bind(("limit", limit.unwrap_or(20)))
            .await
            .map_err(RepositoryError::Database)?;

        let users: Vec<UserDataTuple> =
            response.take(0).map_err(RepositoryError::Database)?;

        let user_count = users.len();
        let results = users
            .into_iter()
            .map(|(user_id, display_name, avatar_url, is_guest)| {
                matryx_entity::types::UserDirectoryEntry {
                    user_id,
                    display_name,
                    avatar_url,
                    is_guest: is_guest.unwrap_or(false),
                }
            })
            .collect();

        Ok(matryx_entity::types::UserDirectoryResponse {
            results,
            limited: user_count >= limit.unwrap_or(20) as usize,
        })
    }
}
