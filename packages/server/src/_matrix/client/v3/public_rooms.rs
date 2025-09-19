use axum::extract::ConnectInfo;
use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::net::SocketAddr;
use tracing::{error, info};

use crate::auth::{MatrixAuthError, authenticate_user};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct PublicRoomsFilter {
    pub limit: Option<u64>,
    pub since: Option<String>,
    pub filter: Option<RoomFilter>,
    pub include_all_known_networks: Option<bool>,
    pub third_party_instance_id: Option<String>,
}

#[derive(Deserialize)]
pub struct RoomFilter {
    pub generic_search_term: Option<String>,
    pub room_types: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct PublicRoomsResponse {
    pub chunk: Vec<PublicRoom>,
    pub next_batch: Option<String>,
    pub prev_batch: Option<String>,
    pub total_room_count_estimate: Option<u64>,
}

#[derive(Serialize)]
pub struct PublicRoom {
    pub room_id: String,
    pub name: Option<String>,
    pub topic: Option<String>,
    pub canonical_alias: Option<String>,
    pub num_joined_members: u64,
    pub avatar_url: Option<String>,
    pub world_readable: bool,
    pub guest_can_join: bool,
    pub join_rule: Option<String>,
    pub room_type: Option<String>,
}

/// GET /_matrix/client/v3/publicRooms
pub async fn get(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<PublicRoomsResponse>, StatusCode> {
    // Authentication is optional for public rooms directory
    let _user_id = match authenticate_user(&state, &headers).await {
        Ok(user_id) => Some(user_id),
        Err(_) => None, // Allow anonymous access to public rooms
    };

    info!("Public rooms request from {}", addr);

    let limit = params
        .get("limit")
        .and_then(|l| l.parse::<u64>().ok())
        .unwrap_or(10)
        .min(100); // Cap at 100

    let since = params.get("since").cloned();

    // Query public rooms from database
    let query = r#"
        SELECT room_id, name, topic, canonical_alias, num_joined_members, 
               avatar_url, world_readable, guest_can_join, join_rule, room_type
        FROM public_rooms
        ORDER BY num_joined_members DESC
        LIMIT $limit
    "#;

    let public_rooms = match state.db.query(query).bind(("limit", limit)).await {
        Ok(mut result) => {
            match result.take::<Vec<(
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                i64,
                Option<String>,
                bool,
                bool,
                String,
                Option<String>,
            )>>(0)
            {
                Ok(rooms) => rooms,
                Err(e) => {
                    error!("Failed to parse public rooms: {}", e);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                },
            }
        },
        Err(e) => {
            error!("Failed to query public rooms: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        },
    };

    // Convert to response format
    let chunk: Vec<PublicRoom> = public_rooms
        .into_iter()
        .map(
            |(
                room_id,
                name,
                topic,
                canonical_alias,
                num_joined_members,
                avatar_url,
                world_readable,
                guest_can_join,
                join_rule,
                room_type,
            )| {
                PublicRoom {
                    room_id,
                    name,
                    topic,
                    canonical_alias,
                    num_joined_members: num_joined_members.max(0) as u64,
                    avatar_url,
                    world_readable,
                    guest_can_join,
                    join_rule: Some(join_rule),
                    room_type,
                }
            },
        )
        .collect();

    // Get total count estimate
    let count_query = "SELECT count() FROM public_rooms GROUP ALL";
    let total_count = match state.db.query(count_query).await {
        Ok(mut result) => {
            match result.take::<Vec<i64>>(0) {
                Ok(counts) => counts.into_iter().next().unwrap_or(0) as u64,
                Err(_) => chunk.len() as u64,
            }
        },
        Err(_) => chunk.len() as u64,
    };

    Ok(Json(PublicRoomsResponse {
        chunk,
        next_batch: None, // Implement pagination later
        prev_batch: since,
        total_room_count_estimate: Some(total_count),
    }))
}

/// POST /_matrix/client/v3/publicRooms
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(filter): Json<PublicRoomsFilter>,
) -> Result<Json<PublicRoomsResponse>, StatusCode> {
    // Authentication is optional for public rooms directory
    let _user_id = match authenticate_user(&state, &headers).await {
        Ok(user_id) => Some(user_id),
        Err(_) => None, // Allow anonymous access to public rooms
    };

    info!("Public rooms search from {} with filter", addr);

    let limit = filter.limit.unwrap_or(10).min(100); // Cap at 100

    let mut query = String::from(
        r#"
        SELECT room_id, name, topic, canonical_alias, num_joined_members, 
               avatar_url, world_readable, guest_can_join, join_rule, room_type
        FROM public_rooms
    "#,
    );

    let mut conditions = Vec::new();
    let mut bindings = Vec::new();

    // Apply search filter
    if let Some(room_filter) = &filter.filter {
        if let Some(search_term) = &room_filter.generic_search_term {
            conditions.push("(name CONTAINS $search_term OR topic CONTAINS $search_term)");
            bindings.push(("search_term", search_term.as_str()));
        }

        if let Some(room_types) = &room_filter.room_types {
            if !room_types.is_empty() {
                conditions.push("room_type IN $room_types");
                bindings.push(("room_types", room_types));
            }
        }
    }

    if !conditions.is_empty() {
        query.push_str(" WHERE ");
        query.push_str(&conditions.join(" AND "));
    }

    query.push_str(" ORDER BY num_joined_members DESC LIMIT $limit");

    let mut db_query = state.db.query(&query);
    for (key, value) in bindings {
        db_query = db_query.bind((key, value));
    }
    db_query = db_query.bind(("limit", limit));

    let public_rooms = match db_query.await {
        Ok(mut result) => {
            match result.take::<Vec<(
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                i64,
                Option<String>,
                bool,
                bool,
                String,
                Option<String>,
            )>>(0)
            {
                Ok(rooms) => rooms,
                Err(e) => {
                    error!("Failed to parse filtered public rooms: {}", e);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                },
            }
        },
        Err(e) => {
            error!("Failed to query filtered public rooms: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        },
    };

    // Convert to response format
    let chunk: Vec<PublicRoom> = public_rooms
        .into_iter()
        .map(
            |(
                room_id,
                name,
                topic,
                canonical_alias,
                num_joined_members,
                avatar_url,
                world_readable,
                guest_can_join,
                join_rule,
                room_type,
            )| {
                PublicRoom {
                    room_id,
                    name,
                    topic,
                    canonical_alias,
                    num_joined_members: num_joined_members.max(0) as u64,
                    avatar_url,
                    world_readable,
                    guest_can_join,
                    join_rule: Some(join_rule),
                    room_type,
                }
            },
        )
        .collect();

    Ok(Json(PublicRoomsResponse {
        chunk,
        next_batch: None, // Implement pagination later
        prev_batch: filter.since,
        total_room_count_estimate: None, // Could be expensive to calculate with filters
    }))
}
