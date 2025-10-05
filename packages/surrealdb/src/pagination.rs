use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose};
use chrono::Utc;

/// Direction for pagination
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Direction {
    #[serde(rename = "f")]
    Forward,
    #[serde(rename = "b")]
    Backward,
}

/// Opaque pagination token with validation and expiration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationToken {
    /// Cursor position (timestamp or event index)
    pub position: i64,
    
    /// Pagination direction
    pub direction: Direction,
    
    /// Room context for validation
    pub room_id: String,
    
    /// Token creation timestamp (for expiration check)
    pub created_at: i64,
    
    /// Optional event ID for precise positioning
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
}

/// Token expiration time in hours
const TOKEN_TTL_HOURS: i64 = 24;

/// Pagination errors
#[derive(Debug, thiserror::Error)]
pub enum PaginationError {
    #[error("Invalid token format: {0}")]
    InvalidFormat(String),
    
    #[error("Token has expired")]
    Expired,
    
    #[error("Token room_id mismatch: expected {expected}, got {got}")]
    RoomMismatch { expected: String, got: String },
    
    #[error("Base64 decode error: {0}")]
    DecodeError(#[from] base64::DecodeError),
    
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

impl PaginationToken {
    /// Create a new pagination token
    pub fn new(
        position: i64,
        direction: Direction,
        room_id: String,
        event_id: Option<String>,
    ) -> Self {
        Self {
            position,
            direction,
            room_id,
            created_at: Utc::now().timestamp(),
            event_id,
        }
    }
    
    /// Encode token to opaque base64url string
    pub fn encode(&self) -> Result<String, PaginationError> {
        let json = serde_json::to_string(self)?;
        // Use URL_SAFE_NO_PAD for Matrix spec compliance (URL-safe tokens)
        Ok(general_purpose::URL_SAFE_NO_PAD.encode(json.as_bytes()))
    }
    
    /// Decode token from base64url string
    pub fn decode(token: &str) -> Result<Self, PaginationError> {
        let bytes = general_purpose::URL_SAFE_NO_PAD
            .decode(token.as_bytes())?;
        let json = String::from_utf8(bytes)
            .map_err(|e| PaginationError::InvalidFormat(e.to_string()))?;
        Ok(serde_json::from_str(&json)?)
    }
    
    /// Validate token (expiration and room context)
    pub fn validate(&self, expected_room_id: &str) -> Result<(), PaginationError> {
        // Check expiration
        let now = Utc::now().timestamp();
        let age_hours = (now - self.created_at) / 3600;
        
        if age_hours > TOKEN_TTL_HOURS {
            return Err(PaginationError::Expired);
        }
        
        // Check room context
        if self.room_id != expected_room_id {
            return Err(PaginationError::RoomMismatch {
                expected: expected_room_id.to_string(),
                got: self.room_id.clone(),
            });
        }
        
        Ok(())
    }
    
    /// Check if token is valid (convenience method)
    pub fn is_valid(&self, room_id: &str) -> bool {
        self.validate(room_id).is_ok()
    }
}

/// Helper to generate start/end tokens from event list
pub fn generate_timeline_tokens(
    events: &[matryx_entity::types::Event],
    room_id: &str,
) -> (Option<String>, Option<String>) {
    if events.is_empty() {
        return (None, None);
    }
    
    let start = events.first().and_then(|e| {
        let token = PaginationToken::new(
            e.origin_server_ts,
            Direction::Backward,
            room_id.to_string(),
            Some(e.event_id.clone()),
        );
        token.encode().ok()
    });
    
    let end = events.last().and_then(|e| {
        let token = PaginationToken::new(
            e.origin_server_ts,
            Direction::Forward,
            room_id.to_string(),
            Some(e.event_id.clone()),
        );
        token.encode().ok()
    });
    
    (start, end)
}

/// Helper to generate next_batch token (forward pagination)
pub fn generate_next_batch(
    events: &[matryx_entity::types::Event],
    room_id: &str,
    limit: usize,
) -> Option<String> {
    if events.len() >= limit {
        events.last().and_then(|e| {
            let token = PaginationToken::new(
                e.origin_server_ts,
                Direction::Forward,
                room_id.to_string(),
                Some(e.event_id.clone()),
            );
            token.encode().ok()
        })
    } else {
        None
    }
}

/// Helper to generate prev_batch token (backward pagination)
pub fn generate_prev_batch(
    events: &[matryx_entity::types::Event],
    room_id: &str,
    limit: usize,
) -> Option<String> {
    if events.len() >= limit {
        events.first().and_then(|e| {
            let token = PaginationToken::new(
                e.origin_server_ts,
                Direction::Backward,
                room_id.to_string(),
                Some(e.event_id.clone()),
            );
            token.encode().ok()
        })
    } else {
        None
    }
}
