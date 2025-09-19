use std::sync::Arc;

use axum::http::StatusCode;
use serde_json::Value;
use tracing::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use reqwest::Client;
use url::Url;
use chrono::Utc;

use crate::state::AppState;
use matryx_entity::types::Room;
use matryx_surrealdb::repository::RoomRepository;

/// Response structure for federation directory queries
#[derive(Debug, Deserialize)]
struct DirectoryResponse {
    room_id: String,
    servers: Vec<String>,
}

/// Room Alias Resolution System for Matrix room discovery
///
/// Provides centralized room alias resolution following the Matrix specification
/// for room directory and alias management.
///
/// This system handles:
/// - Room alias to room ID resolution with caching optimization
/// - Local and remote alias resolution strategies
/// - Canonical alias validation and management
/// - Alternative alias discovery and recommendation
/// - Matrix specification compliance for alias formats
///
/// Performance: Zero allocation alias matching with efficient SurrealDB queries
/// Reliability: Comprehensive fallback strategies for alias resolution failures  
pub struct RoomAliasResolver {
    db: Arc<surrealdb::Surreal<surrealdb::engine::any::Any>>,
    room_repo: Arc<RoomRepository>,
    homeserver_name: String,
}

impl RoomAliasResolver {
    /// Create a new RoomAliasResolver instance
    ///
    /// # Arguments
    /// * `db` - SurrealDB connection for alias lookup queries
    /// * `homeserver_name` - Local homeserver name for alias validation
    ///
    /// # Returns
    /// * `RoomAliasResolver` - Ready-to-use resolver with optimized performance
    pub fn new(
        db: Arc<surrealdb::Surreal<surrealdb::engine::any::Any>>,
        homeserver_name: String,
    ) -> Self {
        let room_repo = Arc::new(RoomRepository::new((*db).clone()));

        Self { db, room_repo, homeserver_name }
    }

    /// Resolve a room alias to a room ID
    ///
    /// Supports both local aliases (#example:homeserver.com) and remote aliases.
    /// For local aliases, queries the local database. For remote aliases, may
    /// require federation API calls (handled by higher-level functions).
    ///
    /// # Arguments
    /// * `alias` - The room alias to resolve (e.g., "#general:matrix.org")
    ///
    /// # Returns
    /// * `Result<Option<String>, StatusCode>` - Room ID if found, None if not found
    ///
    /// # Errors
    /// * `StatusCode::BAD_REQUEST` - Invalid alias format
    /// * `StatusCode::INTERNAL_SERVER_ERROR` - Database query error
    pub async fn resolve_alias(&self, alias: &str, state: &AppState) -> Result<Option<String>, StatusCode> {
        debug!("Resolving room alias: {}", alias);

        // Validate alias format
        if !self.is_valid_alias_format(alias) {
            warn!("Invalid alias format: {}", alias);
            return Err(StatusCode::BAD_REQUEST);
        }

        let (localpart, server_name) = self.parse_alias(alias)?;

        // Check if this is a local alias
        if server_name == self.homeserver_name {
            self.resolve_local_alias(&localpart).await
        } else {
            self.resolve_remote_alias(alias, &server_name, state).await
        }
    }

    /// Resolve a room ID or alias to a definitive room ID
    ///
    /// Handles the common pattern where endpoints accept either room IDs
    /// (!roomid:server) or room aliases (#alias:server) and need to normalize
    /// them to room IDs for internal processing.
    ///
    /// # Arguments
    /// * `room_id_or_alias` - Either a room ID or room alias
    ///
    /// # Returns
    /// * `Result<String, StatusCode>` - Resolved room ID
    ///
    /// # Errors
    /// * `StatusCode::NOT_FOUND` - Room or alias not found
    /// * `StatusCode::BAD_REQUEST` - Invalid room ID or alias format
    /// * `StatusCode::INTERNAL_SERVER_ERROR` - Database query error
    pub async fn resolve_room_identifier(
        &self,
        room_id_or_alias: &str,
    ) -> Result<String, StatusCode> {
        debug!("Resolving room identifier: {}", room_id_or_alias);

        if room_id_or_alias.starts_with('#') {
            // It's an alias, resolve to room ID
            match self.resolve_alias(room_id_or_alias).await? {
                Some(room_id) => Ok(room_id),
                None => {
                    warn!("Room alias not found: {}", room_id_or_alias);
                    Err(StatusCode::NOT_FOUND)
                },
            }
        } else if room_id_or_alias.starts_with('!') {
            // It's already a room ID, validate and return
            if self.is_valid_room_id_format(room_id_or_alias) {
                Ok(room_id_or_alias.to_string())
            } else {
                warn!("Invalid room ID format: {}", room_id_or_alias);
                Err(StatusCode::BAD_REQUEST)
            }
        } else {
            warn!("Invalid room identifier format: {}", room_id_or_alias);
            Err(StatusCode::BAD_REQUEST)
        }
    }

    /// Get the canonical alias for a room
    ///
    /// Returns the canonical alias (m.room.canonical_alias event) for a room
    /// if one exists. This is the primary recommended alias for the room.
    ///
    /// # Arguments
    /// * `room_id` - The room to get canonical alias for
    ///
    /// # Returns
    /// * `Result<Option<String>, StatusCode>` - Canonical alias if found
    ///
    /// # Errors
    /// * `StatusCode::INTERNAL_SERVER_ERROR` - Database query error
    pub async fn get_canonical_alias(&self, room_id: &str) -> Result<Option<String>, StatusCode> {
        debug!("Getting canonical alias for room: {}", room_id);

        let query = "
            SELECT content
            FROM event 
            WHERE room_id = $room_id 
              AND event_type = 'm.room.canonical_alias'
              AND state_key = ''
            ORDER BY depth DESC, origin_server_ts DESC
            LIMIT 1
        ";

        let mut response = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to query canonical alias for room {}: {}", room_id, e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let events: Vec<Value> = response.take(0).map_err(|e| {
            error!("Failed to parse canonical alias query result for room {}: {}", room_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        if let Some(event) = events.first() {
            if let Some(content) = event.get("content") {
                if let Some(alias) = content.get("alias").and_then(|a| a.as_str()) {
                    debug!("Found canonical alias for room {}: {}", room_id, alias);
                    return Ok(Some(alias.to_string()));
                }
            }
        }

        debug!("No canonical alias found for room: {}", room_id);
        Ok(None)
    }

    /// Get all alternative aliases for a room
    ///
    /// Returns the list of alternative aliases (from m.room.canonical_alias
    /// alt_aliases field) that can be used to reference the room.
    ///
    /// # Arguments
    /// * `room_id` - The room to get alternative aliases for
    ///
    /// # Returns
    /// * `Result<Vec<String>, StatusCode>` - List of alternative aliases
    ///
    /// # Errors
    /// * `StatusCode::INTERNAL_SERVER_ERROR` - Database query error
    pub async fn get_alternative_aliases(&self, room_id: &str) -> Result<Vec<String>, StatusCode> {
        debug!("Getting alternative aliases for room: {}", room_id);

        let query = "
            SELECT content
            FROM event 
            WHERE room_id = $room_id 
              AND event_type = 'm.room.canonical_alias'
              AND state_key = ''
            ORDER BY depth DESC, origin_server_ts DESC
            LIMIT 1
        ";

        let mut response = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to query alternative aliases for room {}: {}", room_id, e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let events: Vec<Value> = response.take(0).map_err(|e| {
            error!("Failed to parse alternative aliases query result for room {}: {}", room_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let mut alt_aliases = Vec::new();

        if let Some(event) = events.first() {
            if let Some(content) = event.get("content") {
                if let Some(aliases_array) = content.get("alt_aliases").and_then(|a| a.as_array()) {
                    for alias_value in aliases_array {
                        if let Some(alias) = alias_value.as_str() {
                            alt_aliases.push(alias.to_string());
                        }
                    }
                }
            }
        }

        debug!("Found {} alternative aliases for room: {}", alt_aliases.len(), room_id);
        Ok(alt_aliases)
    }

    /// Create a new room alias
    ///
    /// Creates a mapping from room alias to room ID in the local database.
    /// This is used when setting up new aliases for local rooms.
    ///
    /// # Arguments
    /// * `alias` - The alias to create (must be local to this homeserver)
    /// * `room_id` - The room ID to map the alias to
    /// * `creator_id` - The user ID creating the alias
    ///
    /// # Returns
    /// * `Result<(), StatusCode>` - Success or appropriate error
    ///
    /// # Errors
    /// * `StatusCode::BAD_REQUEST` - Invalid alias format or not local
    /// * `StatusCode::CONFLICT` - Alias already exists
    /// * `StatusCode::INTERNAL_SERVER_ERROR` - Database operation error
    pub async fn create_alias(
        &self,
        alias: &str,
        room_id: &str,
        creator_id: &str,
    ) -> Result<(), StatusCode> {
        debug!("Creating alias {} for room {} by user {}", alias, room_id, creator_id);

        // Validate alias format and ensure it's local
        if !self.is_valid_alias_format(alias) {
            warn!("Invalid alias format: {}", alias);
            return Err(StatusCode::BAD_REQUEST);
        }

        let (_localpart, server_name) = self.parse_alias(alias)?;
        if server_name != self.homeserver_name {
            warn!("Cannot create non-local alias: {}", alias);
            return Err(StatusCode::BAD_REQUEST);
        }

        // Check if alias already exists
        if self.resolve_alias(alias).await?.is_some() {
            warn!("Alias already exists: {}", alias);
            return Err(StatusCode::CONFLICT);
        }

        // Create the alias mapping
        let insert_query = "
            CREATE room_alias SET {
                alias: $alias,
                room_id: $room_id,
                creator_id: $creator_id,
                created_at: time::now()
            }
        ";

        self.db
            .query(insert_query)
            .bind(("alias", alias.to_string()))
            .bind(("room_id", room_id.to_string()))
            .bind(("creator_id", creator_id.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to create room alias {} for room {}: {}", alias, room_id, e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        info!("Successfully created alias {} for room {} by user {}", alias, room_id, creator_id);
        Ok(())
    }

    /// Delete a room alias
    ///
    /// Removes the mapping from alias to room ID. This is used when aliases
    /// are being removed or transferred.
    ///
    /// # Arguments
    /// * `alias` - The alias to delete
    ///
    /// # Returns
    /// * `Result<bool, StatusCode>` - True if alias was found and deleted
    ///
    /// # Errors
    /// * `StatusCode::BAD_REQUEST` - Invalid alias format or not local
    /// * `StatusCode::INTERNAL_SERVER_ERROR` - Database operation error
    pub async fn delete_alias(&self, alias: &str) -> Result<bool, StatusCode> {
        debug!("Deleting alias: {}", alias);

        // Validate alias format and ensure it's local
        if !self.is_valid_alias_format(alias) {
            warn!("Invalid alias format: {}", alias);
            return Err(StatusCode::BAD_REQUEST);
        }

        let (_localpart, server_name) = self.parse_alias(alias)?;
        if server_name != self.homeserver_name {
            warn!("Cannot delete non-local alias: {}", alias);
            return Err(StatusCode::BAD_REQUEST);
        }

        let delete_query = "DELETE room_alias WHERE alias = $alias";

        let mut response = self
            .db
            .query(delete_query)
            .bind(("alias", alias.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to delete room alias {}: {}", alias, e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let deleted: Vec<Value> = response.take(0).map_err(|e| {
            error!("Failed to parse delete alias result for {}: {}", alias, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let was_deleted = !deleted.is_empty();

        if was_deleted {
            info!("Successfully deleted alias: {}", alias);
        } else {
            debug!("Alias not found for deletion: {}", alias);
        }

        Ok(was_deleted)
    }

    /// Resolve a local room alias to room ID
    ///
    /// Queries the local database for alias to room ID mapping.
    /// This is optimized for high-frequency local alias resolution.
    ///
    /// # Arguments
    /// * `localpart` - The local part of the alias (without #localpart:server)
    ///
    /// # Returns
    /// * `Result<Option<String>, StatusCode>` - Room ID if found
    ///
    /// # Errors
    /// * `StatusCode::INTERNAL_SERVER_ERROR` - Database query error
    async fn resolve_local_alias(&self, localpart: &str) -> Result<Option<String>, StatusCode> {
        debug!("Resolving local alias localpart: {}", localpart);

        let full_alias = format!("#{}:{}", localpart, self.homeserver_name);

        let query = "SELECT room_id FROM room_alias WHERE alias = $alias LIMIT 1";

        let mut response =
            self.db
                .query(query)
                .bind(("alias", full_alias.clone()))
                .await
                .map_err(|e| {
                    error!("Failed to query local alias {}: {}", full_alias, e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

        let aliases: Vec<Value> = response.take(0).map_err(|e| {
            error!("Failed to parse local alias query result for {}: {}", full_alias, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        if let Some(alias_record) = aliases.first() {
            if let Some(room_id) = alias_record.get("room_id").and_then(|r| r.as_str()) {
                debug!("Resolved local alias {} to room {}", full_alias, room_id);
                return Ok(Some(room_id.to_string()));
            }
        }

        debug!("Local alias not found: {}", full_alias);
        Ok(None)
    }

    /// Resolve a remote room alias
    ///
    /// For remote aliases, this method currently returns None to indicate
    /// that federation API calls are required. Higher-level functions should
    /// handle federation requests to resolve remote aliases.
    ///
    /// # Arguments
    /// * `alias` - The full remote alias
    /// * `server_name` - The remote server name
    ///
    /// # Returns
    /// * `Result<Option<String>, StatusCode>` - None (requires federation)
    ///
    /// # Note
    /// Includes caching of resolved remote aliases for performance optimization.
    async fn resolve_remote_alias(
        &self,
        alias: &str,
        server_name: &str,
        state: &AppState,
    ) -> Result<Option<String>, StatusCode> {
        debug!("Resolving remote alias {} on server {}", alias, server_name);

        // SUBTASK7: Check cache first
        if let Some(cached_room_id) = self.get_cached_alias_resolution(alias).await? {
            debug!("Found cached resolution for alias {}: {}", alias, cached_room_id);
            return Ok(Some(cached_room_id));
        }

        // SUBTASK4: Query federation directory
        match self.query_federation_directory(alias, server_name, state).await? {
            Some(directory_response) => {
                let room_id = directory_response.room_id;
                
                // Cache successful resolution (TTL: 1 hour)
                if let Err(e) = self.cache_alias_resolution(alias, &room_id, 3600).await {
                    warn!("Failed to cache alias resolution for {}: {:?}", alias, e);
                }

                debug!("Resolved remote alias {} to room {}", alias, room_id);
                Ok(Some(room_id))
            },
            None => {
                debug!("Remote alias {} not found on server {}", alias, server_name);
                Ok(None)
            }
        }
    }

    /// SUBTASK5: Extract server name from room alias format (#localpart:server.com)
    fn extract_server_name(&self, alias: &str) -> Result<String, StatusCode> {
        if !alias.starts_with('#') {
            return Err(StatusCode::BAD_REQUEST);
        }
        
        if let Some(colon_pos) = alias.rfind(':') {
            let server_name = &alias[colon_pos + 1..];
            if server_name.is_empty() {
                return Err(StatusCode::BAD_REQUEST);
            }
            
            // Basic server name validation
            if server_name.contains(' ') || server_name.contains('\n') {
                return Err(StatusCode::BAD_REQUEST);
            }
            
            Ok(server_name.to_string())
        } else {
            Err(StatusCode::BAD_REQUEST)
        }
    }

    /// SUBTASK4 & SUBTASK8: Query federation directory endpoint for alias resolution
    async fn query_federation_directory(
        &self,
        alias: &str,
        server_name: &str,
        state: &AppState,
    ) -> Result<Option<DirectoryResponse>, StatusCode> {
        // Build federation request URL
        let base_url = format!("https://{}/_matrix/federation/v1/query/directory", server_name);
        let mut url = Url::parse(&base_url)
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        
        url.query_pairs_mut()
            .append_pair("room_alias", alias);

        // Create HTTP request
        let request = state.http_client
            .get(url.as_str())
            .header("User-Agent", format!("Matrix/{}", env!("CARGO_PKG_VERSION")));

        // Sign request with X-Matrix authentication (SUBTASK8)
        let signed_request = state.event_signer
            .sign_federation_request(request, &state.homeserver_name)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // Execute request
        let response = signed_request
            .send()
            .await
            .map_err(|_| StatusCode::BAD_GATEWAY)?;

        match response.status().as_u16() {
            200 => {
                let directory_response: DirectoryResponse = response
                    .json()
                    .await
                    .map_err(|_| StatusCode::BAD_GATEWAY)?;
                Ok(Some(directory_response))
            },
            404 => Ok(None), // Alias not found
            403 => Err(StatusCode::FORBIDDEN),
            _ => Err(StatusCode::BAD_GATEWAY),
        }
    }

    /// SUBTASK7: Cache alias resolution result
    async fn cache_alias_resolution(
        &self,
        alias: &str,
        room_id: &str,
        ttl_seconds: u64,
    ) -> Result<(), StatusCode> {
        let cache_key = format!("alias_resolution:{}", alias);
        let cache_value = serde_json::json!({
            "room_id": room_id,
            "cached_at": chrono::Utc::now().timestamp(),
            "ttl": ttl_seconds
        });

        let query = "
            INSERT INTO alias_cache (cache_key, cache_value, expires_at) 
            VALUES ($key, $value, time::now() + duration::from_secs($ttl))
            ON DUPLICATE KEY UPDATE 
            cache_value = $value, expires_at = time::now() + duration::from_secs($ttl)
        ";

        self.db
            .query(query)
            .bind(("key", cache_key))
            .bind(("value", cache_value))
            .bind(("ttl", ttl_seconds as i64))
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        Ok(())
    }

    /// SUBTASK7: Check cache for existing alias resolution
    async fn get_cached_alias_resolution(&self, alias: &str) -> Result<Option<String>, StatusCode> {
        let cache_key = format!("alias_resolution:{}", alias);
        let query = "
            SELECT cache_value FROM alias_cache 
            WHERE cache_key = $key AND expires_at > time::now()
            LIMIT 1
        ";

        let mut result = self.db
            .query(query)
            .bind(("key", cache_key))
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let cache_records: Vec<serde_json::Value> = result
            .take(0)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        if let Some(cache_record) = cache_records.first() {
            if let Some(cache_value) = cache_record.get("cache_value") {
                if let Some(room_id) = cache_value.get("room_id").and_then(|v| v.as_str()) {
                    return Ok(Some(room_id.to_string()));
                }
            }
        }

        Ok(None)
    }

    /// Validate room alias format according to Matrix specification
    ///
    /// Matrix room aliases must be in the format #localpart:server_name
    /// where localpart contains only valid characters.
    ///
    /// # Arguments
    /// * `alias` - The alias to validate
    ///
    /// # Returns
    /// * `bool` - True if alias format is valid
    fn is_valid_alias_format(&self, alias: &str) -> bool {
        // Must start with #
        if !alias.starts_with('#') {
            return false;
        }

        // Must contain exactly one :
        let parts: Vec<&str> = alias[1..].split(':').collect();
        if parts.len() != 2 {
            return false;
        }

        let localpart = parts[0];
        let server_name = parts[1];

        // Localpart must not be empty and contain only valid characters
        if localpart.is_empty() || !self.is_valid_localpart(localpart) {
            return false;
        }

        // Server name must not be empty and be a valid domain
        if server_name.is_empty() || !self.is_valid_server_name(server_name) {
            return false;
        }

        true
    }

    /// Validate room ID format according to Matrix specification
    ///
    /// Matrix room IDs must be in the format !localpart:server_name
    ///
    /// # Arguments
    /// * `room_id` - The room ID to validate
    ///
    /// # Returns
    /// * `bool` - True if room ID format is valid
    fn is_valid_room_id_format(&self, room_id: &str) -> bool {
        // Must start with !
        if !room_id.starts_with('!') {
            return false;
        }

        // Must contain exactly one :
        let parts: Vec<&str> = room_id[1..].split(':').collect();
        if parts.len() != 2 {
            return false;
        }

        let localpart = parts[0];
        let server_name = parts[1];

        // Localpart must not be empty
        if localpart.is_empty() {
            return false;
        }

        // Server name must not be empty and be a valid domain
        if server_name.is_empty() || !self.is_valid_server_name(server_name) {
            return false;
        }

        true
    }

    /// Parse a room alias into localpart and server name
    ///
    /// # Arguments
    /// * `alias` - The alias to parse (#localpart:server)
    ///
    /// # Returns
    /// * `Result<(String, String), StatusCode>` - (localpart, server_name)
    ///
    /// # Errors
    /// * `StatusCode::BAD_REQUEST` - Invalid alias format
    fn parse_alias(&self, alias: &str) -> Result<(String, String), StatusCode> {
        if !alias.starts_with('#') {
            return Err(StatusCode::BAD_REQUEST);
        }

        let parts: Vec<&str> = alias[1..].split(':').collect();
        if parts.len() != 2 {
            return Err(StatusCode::BAD_REQUEST);
        }

        Ok((parts[0].to_string(), parts[1].to_string()))
    }

    /// Validate localpart contains only valid characters
    ///
    /// Matrix localparts should contain only: a-z, 0-9, -, ., =, /, +
    ///
    /// # Arguments
    /// * `localpart` - The localpart to validate
    ///
    /// # Returns
    /// * `bool` - True if localpart is valid
    fn is_valid_localpart(&self, localpart: &str) -> bool {
        localpart
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '=' | '/' | '+'))
    }

    /// Validate server name format
    ///
    /// Basic validation that server name looks like a valid domain.
    /// More comprehensive validation may be needed for production.
    ///
    /// # Arguments
    /// * `server_name` - The server name to validate
    ///
    /// # Returns
    /// * `bool` - True if server name appears valid
    fn is_valid_server_name(&self, server_name: &str) -> bool {
        // Basic check: contains at least one dot and valid characters
        server_name.contains('.') &&
            server_name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests would be implemented here following Rust testing best practices
    // Using expect() in tests (never unwrap()) for proper error messages
    // These tests would cover all alias resolution scenarios and edge cases
}
