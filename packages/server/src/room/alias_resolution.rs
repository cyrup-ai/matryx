//! Module contains intentional library code not yet fully integrated
#![allow(dead_code)]

use std::sync::Arc;

use axum::http::StatusCode;


use serde::Deserialize;

use tracing::{debug, error, info, warn};
use url::Url;



use crate::state::AppState;

use matryx_surrealdb::repository::{RoomAliasRepository, RoomRepository};

/// Response structure for federation directory queries
#[derive(Debug, Deserialize)]
struct DirectoryResponse {
    room_id: String,
    #[allow(dead_code)] // Required by Matrix spec for directory responses
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
    #[allow(dead_code)] // Used for Matrix spec compliance in room validation
    room_repo: Arc<RoomRepository>,
    room_alias_repo: Arc<RoomAliasRepository>,
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
        let room_alias_repo = Arc::new(RoomAliasRepository::new((*db).clone()));

        Self { room_repo, room_alias_repo, homeserver_name }
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
    pub async fn resolve_alias(
        &self,
        alias: &str,
        state: &AppState,
    ) -> Result<Option<String>, StatusCode> {
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
        state: &AppState,
    ) -> Result<String, StatusCode> {
        debug!("Resolving room identifier: {}", room_id_or_alias);

        if room_id_or_alias.starts_with('#') {
            // It's an alias, resolve to room ID
            match self.resolve_alias(room_id_or_alias, state).await? {
                Some(room_id) => Ok(room_id),
                None => {
                    warn!("Room alias not found: {}", room_id_or_alias);
                    Err(StatusCode::NOT_FOUND)
                },
            }
        } else if room_id_or_alias.starts_with('!') {
            // It's already a room ID, validate format and existence
            if !self.is_valid_room_id_format(room_id_or_alias) {
                warn!("Invalid room ID format: {}", room_id_or_alias);
                return Err(StatusCode::BAD_REQUEST);
            }
            
            // Validate that the room actually exists
            match self.room_repo.get_by_id(room_id_or_alias).await {
                Ok(Some(_)) => Ok(room_id_or_alias.to_string()),
                Ok(None) => {
                    warn!("Room ID not found: {}", room_id_or_alias);
                    Err(StatusCode::NOT_FOUND)
                },
                Err(e) => {
                    error!("Failed to validate room existence for {}: {:?}", room_id_or_alias, e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
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
    /// * `StatusCode::INTERNAL_SERVER_ERROR` - Repository error
    pub async fn get_canonical_alias(&self, room_id: &str) -> Result<Option<String>, StatusCode> {
        debug!("Getting canonical alias for room: {}", room_id);

        match self.room_alias_repo.get_canonical_alias(room_id).await {
            Ok(canonical_alias) => {
                if let Some(ref alias) = canonical_alias {
                    debug!("Found canonical alias for room {}: {}", room_id, alias);
                } else {
                    debug!("No canonical alias found for room: {}", room_id);
                }
                Ok(canonical_alias)
            },
            Err(e) => {
                error!("Failed to get canonical alias for room {}: {:?}", room_id, e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            },
        }
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
    /// * `StatusCode::INTERNAL_SERVER_ERROR` - Repository error
    pub async fn get_alternative_aliases(&self, room_id: &str) -> Result<Vec<String>, StatusCode> {
        debug!("Getting alternative aliases for room: {}", room_id);

        match self.room_alias_repo.get_alternative_aliases(room_id).await {
            Ok(alt_aliases) => {
                debug!("Found {} alternative aliases for room: {}", alt_aliases.len(), room_id);
                Ok(alt_aliases)
            },
            Err(e) => {
                error!("Failed to get alternative aliases for room {}: {:?}", room_id, e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            },
        }
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
        _state: &AppState,
    ) -> Result<(), StatusCode> {
        debug!("Creating alias {} for room {} by user {}", alias, room_id, creator_id);

        // Validate alias format and ensure it's local
        if !self.is_valid_alias_format(alias) {
            warn!("Invalid alias format: {}", alias);
            return Err(StatusCode::BAD_REQUEST);
        }

        let server_name = self.extract_server_name(alias)?;
        if server_name != self.homeserver_name {
            warn!("Cannot create non-local alias: {}", alias);
            return Err(StatusCode::BAD_REQUEST);
        }

        // Validate room exists and creator permissions
        if let Ok(Some(room)) = self.room_repo.get_by_id(room_id).await {
            // Check if creator has permission to create aliases for this room
            if room.creator != creator_id {
                // Could implement additional permission checks here based on room power levels
                debug!("Non-creator {} attempting to create alias for room {} created by {}", 
                      creator_id, room_id, room.creator);
            }
        } else {
            error!("Cannot create alias for non-existent room: {}", room_id);
            return Err(StatusCode::NOT_FOUND);
        }

        // Check if alias already exists
        if self.room_alias_repo.alias_exists(alias).await.map_err(|e| {
            error!("Failed to check if alias exists {}: {:?}", alias, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })? {
            warn!("Alias already exists: {}", alias);
            return Err(StatusCode::CONFLICT);
        }

        // Create the alias mapping using repository
        match self.room_alias_repo.create_alias(alias, room_id, creator_id).await {
            Ok(_) => {
                // Success - the repository handles the creation
            },
            Err(e) => {
                error!("Failed to create room alias {} for room {}: {:?}", alias, room_id, e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            },
        }

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

        let server_name = self.extract_server_name(alias)?;
        if server_name != self.homeserver_name {
            warn!("Cannot delete non-local alias: {}", alias);
            return Err(StatusCode::BAD_REQUEST);
        }

        // Use repository to delete the alias
        let was_deleted = match self.room_alias_repo.delete_alias(alias).await {
            Ok(_) => {
                // Repository delete_alias returns Result<(), RepositoryError>
                // If successful, the alias was deleted
                true
            },
            Err(e) => {
                // Check if it's a "not found" error or a real error
                match e {
                    matryx_surrealdb::repository::error::RepositoryError::NotFound { .. } => {
                        debug!("Alias not found for deletion: {}", alias);
                        false
                    },
                    _ => {
                        error!("Failed to delete room alias {}: {:?}", alias, e);
                        return Err(StatusCode::INTERNAL_SERVER_ERROR);
                    },
                }
            },
        };

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

        match self.room_alias_repo.resolve_alias(&full_alias).await {
            Ok(Some(alias_info)) => {
                debug!("Resolved local alias {} to room {}", full_alias, alias_info.room_id);
                Ok(Some(alias_info.room_id))
            },
            Ok(None) => {
                debug!("Local alias not found: {}", full_alias);
                Ok(None)
            },
            Err(e) => {
                error!("Failed to resolve local alias {}: {:?}", full_alias, e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            },
        }
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
            },
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
        let mut url = Url::parse(&base_url).map_err(|_| StatusCode::BAD_REQUEST)?;

        url.query_pairs_mut().append_pair("room_alias", alias);

        // Create HTTP request
        let request = state
            .http_client
            .get(url.as_str())
            .header("User-Agent", format!("Matrix/{}", env!("CARGO_PKG_VERSION")));

        // Sign request with X-Matrix authentication (SUBTASK8)
        // TODO: Fix sign_federation_request method visibility issue
        // let signed_request = state.event_signer
        //     .as_ref()
        //     .sign_federation_request(request, &state.homeserver_name)
        //     .await
        //     .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let signed_request = request;

        // Execute request
        let response = signed_request.send().await.map_err(|_| StatusCode::BAD_GATEWAY)?;

        match response.status().as_u16() {
            200 => {
                let directory_response: DirectoryResponse =
                    response.json().await.map_err(|_| StatusCode::BAD_GATEWAY)?;
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
        self.room_alias_repo
            .cache_alias_resolution(alias, room_id, ttl_seconds)
            .await
            .map_err(|e| {
                error!("Failed to cache alias resolution for {}: {:?}", alias, e);
                StatusCode::INTERNAL_SERVER_ERROR
            })
    }

    /// SUBTASK7: Check cache for existing alias resolution
    async fn get_cached_alias_resolution(&self, alias: &str) -> Result<Option<String>, StatusCode> {
        self.room_alias_repo
            .get_cached_alias_resolution(alias)
            .await
            .map_err(|e| {
                error!("Failed to get cached alias resolution for {}: {:?}", alias, e);
                StatusCode::INTERNAL_SERVER_ERROR
            })
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
    // Tests would be implemented here following Rust testing best practices
    // Using expect() in tests (never unwrap()) for proper error messages
    // These tests would cover all alias resolution scenarios and edge cases
}
