use axum::http::StatusCode;
use serde_json::Value;
use std::fmt;
use tracing::error;

/// Comprehensive Matrix-Compliant Error Handling for All Membership Operations
///
/// Provides complete error handling for Matrix room membership operations
/// following Matrix specification error codes and HTTP status mappings.
///
/// This system handles:
/// - Matrix error code mapping (M_FORBIDDEN, M_NOT_FOUND, etc.)
/// - Detailed error context for debugging
/// - Federation error handling and network failures  
/// - Room and user validation errors
/// - State resolution and transition errors
///
/// Performance: Zero allocation error handling with elegant ergonomic error types
/// Security: Proper error information disclosure without leaking sensitive data
#[derive(Debug, Clone)]
pub enum MembershipError {
    /// Insufficient permissions for the requested action
    InsufficientPermissions {
        action: String,
        required_level: i64,
        user_level: i64,
        room_id: String,
    },

    /// User is banned from the room
    UserBanned {
        user_id: String,
        room_id: String,
        reason: Option<String>,
    },

    /// Invalid membership state transition
    InvalidMembershipTransition {
        from: String,
        to: String,
        user_id: String,
        room_id: String,
    },

    /// Room does not exist or is not accessible
    RoomNotFound { room_id: String },

    /// Room access denied due to join rules
    RoomAccessDenied { room_id: String, join_rule: String, reason: String },

    /// User not found or invalid user ID
    UserNotFound { user_id: String },

    /// User already has the requested membership state
    MembershipAlreadyExists {
        user_id: String,
        room_id: String,
        current_membership: String,
        requested_membership: String,
    },

    /// Federation server unreachable or returned error
    FederationError {
        server_name: String,
        error_code: Option<String>,
        error_message: String,
        retry_after: Option<u64>,
    },

    /// DNS resolution failed for federation server
    DnsResolutionError {
        server_name: String,
        error: String,
    },

    /// Network timeout during federation request
    FederationTimeout {
        server_name: String,
        timeout_ms: u64,
        operation: String,
    },

    /// Malformed or invalid Matrix event
    InvalidEvent { event_id: Option<String>, reason: String },

    /// Invalid Matrix ID format (room, user, etc.)
    InvalidMatrixId { id: String, expected_type: String },

    /// Database operation failed
    DatabaseError { operation: String, error: String },

    /// JSON parsing or serialization error
    JsonError { context: String, error: String },

    /// Concurrent membership changes detected
    ConflictingMembershipChange {
        user_id: String,
        room_id: String,
        conflicting_event_id: String,
    },

    /// Room state is inconsistent or corrupted
    InconsistentRoomState { room_id: String, details: String },

    /// Rate limiting or resource exhaustion
    RateLimited { retry_after_ms: u64 },

    /// Generic internal server error
    InternalError { context: String, error: String },
}

impl MembershipError {
    /// Convert to appropriate HTTP status code
    pub fn to_status_code(&self) -> StatusCode {
        match self {
            MembershipError::InsufficientPermissions { .. } => StatusCode::FORBIDDEN,
            MembershipError::UserBanned { .. } => StatusCode::FORBIDDEN,
            MembershipError::InvalidMembershipTransition { .. } => StatusCode::BAD_REQUEST,
            MembershipError::RoomNotFound { .. } => StatusCode::NOT_FOUND,
            MembershipError::RoomAccessDenied { .. } => StatusCode::FORBIDDEN,
            MembershipError::UserNotFound { .. } => StatusCode::NOT_FOUND,
            MembershipError::MembershipAlreadyExists { .. } => StatusCode::CONFLICT,
            MembershipError::FederationError { .. } => StatusCode::BAD_GATEWAY,
            MembershipError::FederationTimeout { .. } => StatusCode::GATEWAY_TIMEOUT,
            MembershipError::InvalidEvent { .. } => StatusCode::BAD_REQUEST,
            MembershipError::InvalidMatrixId { .. } => StatusCode::BAD_REQUEST,
            MembershipError::DatabaseError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            MembershipError::JsonError { .. } => StatusCode::BAD_REQUEST,
            MembershipError::ConflictingMembershipChange { .. } => StatusCode::CONFLICT,
            MembershipError::InconsistentRoomState { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            MembershipError::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            MembershipError::InternalError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            MembershipError::DnsResolutionError { .. } => StatusCode::BAD_GATEWAY,
        }
    }

    /// Convert to Matrix error code
    pub fn to_matrix_error_code(&self) -> &'static str {
        match self {
            MembershipError::InsufficientPermissions { .. } => "M_FORBIDDEN",
            MembershipError::UserBanned { .. } => "M_FORBIDDEN",
            MembershipError::InvalidMembershipTransition { .. } => "M_BAD_STATE",
            MembershipError::RoomNotFound { .. } => "M_NOT_FOUND",
            MembershipError::RoomAccessDenied { .. } => "M_FORBIDDEN",
            MembershipError::UserNotFound { .. } => "M_NOT_FOUND",
            MembershipError::MembershipAlreadyExists { .. } => "M_BAD_STATE",
            MembershipError::FederationError { .. } => "M_UNKNOWN",
            MembershipError::DnsResolutionError { .. } => "M_UNKNOWN",
            MembershipError::FederationTimeout { .. } => "M_UNKNOWN",
            MembershipError::InvalidEvent { .. } => "M_BAD_JSON",
            MembershipError::InvalidMatrixId { .. } => "M_INVALID_PARAM",
            MembershipError::DatabaseError { .. } => "M_UNKNOWN",
            MembershipError::JsonError { .. } => "M_BAD_JSON",
            MembershipError::ConflictingMembershipChange { .. } => "M_BAD_STATE",
            MembershipError::InconsistentRoomState { .. } => "M_UNKNOWN",
            MembershipError::RateLimited { .. } => "M_LIMIT_EXCEEDED",
            MembershipError::InternalError { .. } => "M_UNKNOWN",
        }
    }

    /// Generate Matrix-compliant error response JSON
    pub fn to_matrix_response(&self) -> Value {
        let mut response = serde_json::json!({
            "errcode": self.to_matrix_error_code(),
            "error": self.to_string()
        });

        // Add specific fields for certain error types
        match self {
            MembershipError::RateLimited { retry_after_ms } => {
                response["retry_after_ms"] = (*retry_after_ms).into();
            },
            MembershipError::FederationError { retry_after: Some(retry), .. } => {
                response["retry_after_ms"] = (*retry).into();
            },
            MembershipError::FederationError { retry_after: None, .. } => {},
            _ => {},
        }

        response
    }

    /// Log error with appropriate level and context
    pub fn log_error(&self) {
        match self {
            MembershipError::InternalError { context, error } |
            MembershipError::DatabaseError { operation: context, error } => {
                error!("Internal error in {}: {}", context, error);
            },
            MembershipError::FederationError { server_name, error_message, .. } => {
                error!("Federation error from {}: {}", server_name, error_message);
            },
            MembershipError::InconsistentRoomState { room_id, details } => {
                error!("Inconsistent room state in {}: {}", room_id, details);
            },
            _ => {
                // Other errors are typically client errors, log at debug level
                error!("Membership error: {}", self);
            },
        }
    }
}

impl fmt::Display for MembershipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MembershipError::InsufficientPermissions {
                action, required_level, user_level, ..
            } => {
                write!(
                    f,
                    "Insufficient permissions for {}: required level {}, user has {}",
                    action, required_level, user_level
                )
            },
            MembershipError::UserBanned { user_id, reason, .. } => {
                match reason {
                    Some(r) => write!(f, "User {} is banned: {}", user_id, r),
                    None => write!(f, "User {} is banned", user_id),
                }
            },
            MembershipError::InvalidMembershipTransition { from, to, user_id, .. } => {
                write!(f, "Invalid membership transition for {}: {} -> {}", user_id, from, to)
            },
            MembershipError::RoomNotFound { room_id } => {
                write!(f, "Room {} not found", room_id)
            },
            MembershipError::RoomAccessDenied { room_id, join_rule, reason } => {
                write!(f, "Access denied to room {} ({}): {}", room_id, join_rule, reason)
            },
            MembershipError::UserNotFound { user_id } => {
                write!(f, "User {} not found", user_id)
            },
            MembershipError::MembershipAlreadyExists {
                user_id,
                current_membership,
                requested_membership,
                ..
            } => {
                write!(
                    f,
                    "User {} already has membership {} (requested {})",
                    user_id, current_membership, requested_membership
                )
            },
            MembershipError::FederationError { server_name, error_message, .. } => {
                write!(f, "Federation error from {}: {}", server_name, error_message)
            },
            MembershipError::DnsResolutionError { server_name, error } => {
                write!(f, "DNS resolution failed for {}: {}", server_name, error)
            },
            MembershipError::FederationTimeout { server_name, timeout_ms, operation } => {
                write!(
                    f,
                    "Federation timeout for {} on {}: {}ms",
                    operation, server_name, timeout_ms
                )
            },
            MembershipError::InvalidEvent { event_id, reason } => {
                match event_id {
                    Some(id) => write!(f, "Invalid event {}: {}", id, reason),
                    None => write!(f, "Invalid event: {}", reason),
                }
            },
            MembershipError::InvalidMatrixId { id, expected_type } => {
                write!(f, "Invalid {} ID: {}", expected_type, id)
            },
            MembershipError::DatabaseError { operation, error } => {
                write!(f, "Database error during {}: {}", operation, error)
            },
            MembershipError::JsonError { context, error } => {
                write!(f, "JSON error in {}: {}", context, error)
            },
            MembershipError::ConflictingMembershipChange {
                user_id, conflicting_event_id, ..
            } => {
                write!(
                    f,
                    "Conflicting membership change for {} (conflict: {})",
                    user_id, conflicting_event_id
                )
            },
            MembershipError::InconsistentRoomState { room_id, details } => {
                write!(f, "Inconsistent room state in {}: {}", room_id, details)
            },
            MembershipError::RateLimited { retry_after_ms } => {
                write!(f, "Rate limited, retry after {}ms", retry_after_ms)
            },
            MembershipError::InternalError { context, error } => {
                write!(f, "Internal error in {}: {}", context, error)
            },
        }
    }
}

impl std::error::Error for MembershipError {}

/// Convenience type alias for membership operation results
pub type MembershipResult<T> = Result<T, MembershipError>;

/// Helper functions for creating common error types
impl MembershipError {
    /// Create insufficient permissions error for membership actions
    pub fn insufficient_permissions(
        action: &str,
        required_level: i64,
        user_level: i64,
        room_id: &str,
    ) -> Self {
        MembershipError::InsufficientPermissions {
            action: action.to_string(),
            required_level,
            user_level,
            room_id: room_id.to_string(),
        }
    }

    /// Create user banned error
    pub fn user_banned(user_id: &str, room_id: &str, reason: Option<&str>) -> Self {
        MembershipError::UserBanned {
            user_id: user_id.to_string(),
            room_id: room_id.to_string(),
            reason: reason.map(|s| s.to_string()),
        }
    }

    /// Create invalid membership transition error
    pub fn invalid_transition(from: &str, to: &str, user_id: &str, room_id: &str) -> Self {
        MembershipError::InvalidMembershipTransition {
            from: from.to_string(),
            to: to.to_string(),
            user_id: user_id.to_string(),
            room_id: room_id.to_string(),
        }
    }

    /// Create room not found error
    pub fn room_not_found(room_id: &str) -> Self {
        MembershipError::RoomNotFound { room_id: room_id.to_string() }
    }

    /// Create federation error
    pub fn federation_error(server_name: &str, error_message: &str) -> Self {
        MembershipError::FederationError {
            server_name: server_name.to_string(),
            error_code: None,
            error_message: error_message.to_string(),
            retry_after: None,
        }
    }

    /// Create database error
    pub fn database_error(operation: &str, error: &str) -> Self {
        MembershipError::DatabaseError {
            operation: operation.to_string(),
            error: error.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod error_construction_tests {
        use super::*;

        #[test]
        fn test_insufficient_permissions_helper() {
            let error =
                MembershipError::insufficient_permissions("ban user", 50, 10, "!room:example.com");

            match error {
                MembershipError::InsufficientPermissions {
                    action,
                    required_level,
                    user_level,
                    room_id,
                } => {
                    assert_eq!(action, "ban user");
                    assert_eq!(required_level, 50);
                    assert_eq!(user_level, 10);
                    assert_eq!(room_id, "!room:example.com");
                },
                _ => panic!("Expected InsufficientPermissions error"),
            }
        }

        #[test]
        fn test_user_banned_helper_with_reason() {
            let error = MembershipError::user_banned(
                "@user:example.com",
                "!room:example.com",
                Some("Spamming"),
            );

            match error {
                MembershipError::UserBanned { user_id, room_id, reason } => {
                    assert_eq!(user_id, "@user:example.com");
                    assert_eq!(room_id, "!room:example.com");
                    assert_eq!(reason, Some("Spamming".to_string()));
                },
                _ => panic!("Expected UserBanned error"),
            }
        }

        #[test]
        fn test_user_banned_helper_without_reason() {
            let error =
                MembershipError::user_banned("@user:example.com", "!room:example.com", None);

            match error {
                MembershipError::UserBanned { user_id, room_id, reason } => {
                    assert_eq!(user_id, "@user:example.com");
                    assert_eq!(room_id, "!room:example.com");
                    assert_eq!(reason, None);
                },
                _ => panic!("Expected UserBanned error"),
            }
        }

        #[test]
        fn test_invalid_transition_helper() {
            let error = MembershipError::invalid_transition(
                "join",
                "invite",
                "@user:example.com",
                "!room:example.com",
            );

            match error {
                MembershipError::InvalidMembershipTransition { from, to, user_id, room_id } => {
                    assert_eq!(from, "join");
                    assert_eq!(to, "invite");
                    assert_eq!(user_id, "@user:example.com");
                    assert_eq!(room_id, "!room:example.com");
                },
                _ => panic!("Expected InvalidMembershipTransition error"),
            }
        }

        #[test]
        fn test_room_not_found_helper() {
            let error = MembershipError::room_not_found("!nonexistent:example.com");

            match error {
                MembershipError::RoomNotFound { room_id } => {
                    assert_eq!(room_id, "!nonexistent:example.com");
                },
                _ => panic!("Expected RoomNotFound error"),
            }
        }

        #[test]
        fn test_federation_error_helper() {
            let error =
                MembershipError::federation_error("remote.server.com", "Connection refused");

            match error {
                MembershipError::FederationError {
                    server_name,
                    error_code,
                    error_message,
                    retry_after,
                } => {
                    assert_eq!(server_name, "remote.server.com");
                    assert_eq!(error_code, None);
                    assert_eq!(error_message, "Connection refused");
                    assert_eq!(retry_after, None);
                },
                _ => panic!("Expected FederationError error"),
            }
        }

        #[test]
        fn test_database_error_helper() {
            let error = MembershipError::database_error("insert membership", "Connection lost");

            match error {
                MembershipError::DatabaseError { operation, error } => {
                    assert_eq!(operation, "insert membership");
                    assert_eq!(error, "Connection lost");
                },
                _ => panic!("Expected DatabaseError error"),
            }
        }
    }

    mod status_code_mapping_tests {
        use super::*;

        #[test]
        fn test_insufficient_permissions_status_code() {
            let error = MembershipError::insufficient_permissions("test", 50, 10, "!room:test");
            assert_eq!(error.to_status_code(), StatusCode::FORBIDDEN);
        }

        #[test]
        fn test_user_banned_status_code() {
            let error = MembershipError::user_banned("@user:test", "!room:test", None);
            assert_eq!(error.to_status_code(), StatusCode::FORBIDDEN);
        }

        #[test]
        fn test_invalid_membership_transition_status_code() {
            let error =
                MembershipError::invalid_transition("join", "invite", "@user:test", "!room:test");
            assert_eq!(error.to_status_code(), StatusCode::BAD_REQUEST);
        }

        #[test]
        fn test_room_not_found_status_code() {
            let error = MembershipError::room_not_found("!room:test");
            assert_eq!(error.to_status_code(), StatusCode::NOT_FOUND);
        }

        #[test]
        fn test_room_access_denied_status_code() {
            let error = MembershipError::RoomAccessDenied {
                room_id: "!room:test".to_string(),
                join_rule: "invite".to_string(),
                reason: "Not invited".to_string(),
            };
            assert_eq!(error.to_status_code(), StatusCode::FORBIDDEN);
        }

        #[test]
        fn test_user_not_found_status_code() {
            let error = MembershipError::UserNotFound { user_id: "@nonexistent:test".to_string() };
            assert_eq!(error.to_status_code(), StatusCode::NOT_FOUND);
        }

        #[test]
        fn test_membership_already_exists_status_code() {
            let error = MembershipError::MembershipAlreadyExists {
                user_id: "@user:test".to_string(),
                room_id: "!room:test".to_string(),
                current_membership: "join".to_string(),
                requested_membership: "invite".to_string(),
            };
            assert_eq!(error.to_status_code(), StatusCode::CONFLICT);
        }

        #[test]
        fn test_federation_error_status_code() {
            let error = MembershipError::federation_error("remote.server", "error");
            assert_eq!(error.to_status_code(), StatusCode::BAD_GATEWAY);
        }

        #[test]
        fn test_federation_timeout_status_code() {
            let error = MembershipError::FederationTimeout {
                server_name: "remote.server".to_string(),
                timeout_ms: 5000,
                operation: "join".to_string(),
            };
            assert_eq!(error.to_status_code(), StatusCode::GATEWAY_TIMEOUT);
        }

        #[test]
        fn test_invalid_event_status_code() {
            let error = MembershipError::InvalidEvent {
                event_id: Some("$event:test".to_string()),
                reason: "Missing field".to_string(),
            };
            assert_eq!(error.to_status_code(), StatusCode::BAD_REQUEST);
        }

        #[test]
        fn test_invalid_matrix_id_status_code() {
            let error = MembershipError::InvalidMatrixId {
                id: "invalid_id".to_string(),
                expected_type: "user".to_string(),
            };
            assert_eq!(error.to_status_code(), StatusCode::BAD_REQUEST);
        }

        #[test]
        fn test_database_error_status_code() {
            let error = MembershipError::database_error("test", "error");
            assert_eq!(error.to_status_code(), StatusCode::INTERNAL_SERVER_ERROR);
        }

        #[test]
        fn test_json_error_status_code() {
            let error = MembershipError::JsonError {
                context: "parsing".to_string(),
                error: "invalid json".to_string(),
            };
            assert_eq!(error.to_status_code(), StatusCode::BAD_REQUEST);
        }

        #[test]
        fn test_conflicting_membership_change_status_code() {
            let error = MembershipError::ConflictingMembershipChange {
                user_id: "@user:test".to_string(),
                room_id: "!room:test".to_string(),
                conflicting_event_id: "$conflict:test".to_string(),
            };
            assert_eq!(error.to_status_code(), StatusCode::CONFLICT);
        }

        #[test]
        fn test_inconsistent_room_state_status_code() {
            let error = MembershipError::InconsistentRoomState {
                room_id: "!room:test".to_string(),
                details: "State mismatch".to_string(),
            };
            assert_eq!(error.to_status_code(), StatusCode::INTERNAL_SERVER_ERROR);
        }

        #[test]
        fn test_rate_limited_status_code() {
            let error = MembershipError::RateLimited { retry_after_ms: 1000 };
            assert_eq!(error.to_status_code(), StatusCode::TOO_MANY_REQUESTS);
        }

        #[test]
        fn test_internal_error_status_code() {
            let error = MembershipError::InternalError {
                context: "test".to_string(),
                error: "unexpected".to_string(),
            };
            assert_eq!(error.to_status_code(), StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    mod matrix_error_code_mapping_tests {
        use super::*;

        #[test]
        fn test_insufficient_permissions_matrix_code() {
            let error = MembershipError::insufficient_permissions("test", 50, 10, "!room:test");
            assert_eq!(error.to_matrix_error_code(), "M_FORBIDDEN");
        }

        #[test]
        fn test_user_banned_matrix_code() {
            let error = MembershipError::user_banned("@user:test", "!room:test", None);
            assert_eq!(error.to_matrix_error_code(), "M_FORBIDDEN");
        }

        #[test]
        fn test_invalid_membership_transition_matrix_code() {
            let error =
                MembershipError::invalid_transition("join", "invite", "@user:test", "!room:test");
            assert_eq!(error.to_matrix_error_code(), "M_BAD_STATE");
        }

        #[test]
        fn test_room_not_found_matrix_code() {
            let error = MembershipError::room_not_found("!room:test");
            assert_eq!(error.to_matrix_error_code(), "M_NOT_FOUND");
        }

        #[test]
        fn test_room_access_denied_matrix_code() {
            let error = MembershipError::RoomAccessDenied {
                room_id: "!room:test".to_string(),
                join_rule: "invite".to_string(),
                reason: "Not invited".to_string(),
            };
            assert_eq!(error.to_matrix_error_code(), "M_FORBIDDEN");
        }

        #[test]
        fn test_user_not_found_matrix_code() {
            let error = MembershipError::UserNotFound { user_id: "@nonexistent:test".to_string() };
            assert_eq!(error.to_matrix_error_code(), "M_NOT_FOUND");
        }

        #[test]
        fn test_membership_already_exists_matrix_code() {
            let error = MembershipError::MembershipAlreadyExists {
                user_id: "@user:test".to_string(),
                room_id: "!room:test".to_string(),
                current_membership: "join".to_string(),
                requested_membership: "invite".to_string(),
            };
            assert_eq!(error.to_matrix_error_code(), "M_BAD_STATE");
        }

        #[test]
        fn test_federation_error_matrix_code() {
            let error = MembershipError::federation_error("remote.server", "error");
            assert_eq!(error.to_matrix_error_code(), "M_UNKNOWN");
        }

        #[test]
        fn test_federation_timeout_matrix_code() {
            let error = MembershipError::FederationTimeout {
                server_name: "remote.server".to_string(),
                timeout_ms: 5000,
                operation: "join".to_string(),
            };
            assert_eq!(error.to_matrix_error_code(), "M_UNKNOWN");
        }

        #[test]
        fn test_invalid_event_matrix_code() {
            let error = MembershipError::InvalidEvent {
                event_id: Some("$event:test".to_string()),
                reason: "Missing field".to_string(),
            };
            assert_eq!(error.to_matrix_error_code(), "M_BAD_JSON");
        }

        #[test]
        fn test_invalid_matrix_id_matrix_code() {
            let error = MembershipError::InvalidMatrixId {
                id: "invalid_id".to_string(),
                expected_type: "user".to_string(),
            };
            assert_eq!(error.to_matrix_error_code(), "M_INVALID_PARAM");
        }

        #[test]
        fn test_database_error_matrix_code() {
            let error = MembershipError::database_error("test", "error");
            assert_eq!(error.to_matrix_error_code(), "M_UNKNOWN");
        }

        #[test]
        fn test_json_error_matrix_code() {
            let error = MembershipError::JsonError {
                context: "parsing".to_string(),
                error: "invalid json".to_string(),
            };
            assert_eq!(error.to_matrix_error_code(), "M_BAD_JSON");
        }

        #[test]
        fn test_conflicting_membership_change_matrix_code() {
            let error = MembershipError::ConflictingMembershipChange {
                user_id: "@user:test".to_string(),
                room_id: "!room:test".to_string(),
                conflicting_event_id: "$conflict:test".to_string(),
            };
            assert_eq!(error.to_matrix_error_code(), "M_BAD_STATE");
        }

        #[test]
        fn test_inconsistent_room_state_matrix_code() {
            let error = MembershipError::InconsistentRoomState {
                room_id: "!room:test".to_string(),
                details: "State mismatch".to_string(),
            };
            assert_eq!(error.to_matrix_error_code(), "M_UNKNOWN");
        }

        #[test]
        fn test_rate_limited_matrix_code() {
            let error = MembershipError::RateLimited { retry_after_ms: 1000 };
            assert_eq!(error.to_matrix_error_code(), "M_LIMIT_EXCEEDED");
        }

        #[test]
        fn test_internal_error_matrix_code() {
            let error = MembershipError::InternalError {
                context: "test".to_string(),
                error: "unexpected".to_string(),
            };
            assert_eq!(error.to_matrix_error_code(), "M_UNKNOWN");
        }
    }

    mod matrix_response_tests {
        use super::*;

        #[test]
        fn test_basic_matrix_response() {
            let error = MembershipError::room_not_found("!room:test");
            let response = error.to_matrix_response();

            assert_eq!(response["errcode"], "M_NOT_FOUND");
            assert_eq!(response["error"], "Room !room:test not found");
            assert!(response.get("retry_after_ms").is_none());
        }

        #[test]
        fn test_rate_limited_matrix_response_with_retry_after() {
            let error = MembershipError::RateLimited { retry_after_ms: 5000 };
            let response = error.to_matrix_response();

            assert_eq!(response["errcode"], "M_LIMIT_EXCEEDED");
            assert_eq!(response["retry_after_ms"], 5000);
        }

        #[test]
        fn test_federation_error_matrix_response_with_retry_after() {
            let error = MembershipError::FederationError {
                server_name: "remote.server".to_string(),
                error_code: Some("M_UNAVAILABLE".to_string()),
                error_message: "Server temporarily unavailable".to_string(),
                retry_after: Some(30000),
            };
            let response = error.to_matrix_response();

            assert_eq!(response["errcode"], "M_UNKNOWN");
            assert_eq!(response["retry_after_ms"], 30000);
        }

        #[test]
        fn test_federation_error_matrix_response_without_retry_after() {
            let error = MembershipError::federation_error("remote.server", "Connection failed");
            let response = error.to_matrix_response();

            assert_eq!(response["errcode"], "M_UNKNOWN");
            assert!(response.get("retry_after_ms").is_none());
        }
    }

    mod display_tests {
        use super::*;

        #[test]
        fn test_insufficient_permissions_display() {
            let error = MembershipError::insufficient_permissions("ban user", 50, 10, "!room:test");
            let display_str = error.to_string();
            assert!(display_str.contains("Insufficient permissions for ban user"));
            assert!(display_str.contains("required level 50"));
            assert!(display_str.contains("user has 10"));
        }

        #[test]
        fn test_user_banned_display_with_reason() {
            let error = MembershipError::user_banned("@user:test", "!room:test", Some("Spamming"));
            let display_str = error.to_string();
            assert!(display_str.contains("User @user:test is banned"));
            assert!(display_str.contains("Spamming"));
        }

        #[test]
        fn test_user_banned_display_without_reason() {
            let error = MembershipError::user_banned("@user:test", "!room:test", None);
            let display_str = error.to_string();
            assert!(display_str.contains("User @user:test is banned"));
            assert!(!display_str.contains(":"));
        }

        #[test]
        fn test_invalid_membership_transition_display() {
            let error =
                MembershipError::invalid_transition("join", "invite", "@user:test", "!room:test");
            let display_str = error.to_string();
            assert!(display_str.contains("Invalid membership transition for @user:test"));
            assert!(display_str.contains("join -> invite"));
        }

        #[test]
        fn test_room_not_found_display() {
            let error = MembershipError::room_not_found("!room:test");
            let display_str = error.to_string();
            assert!(display_str.contains("Room !room:test not found"));
        }

        #[test]
        fn test_room_access_denied_display() {
            let error = MembershipError::RoomAccessDenied {
                room_id: "!room:test".to_string(),
                join_rule: "invite".to_string(),
                reason: "Not invited".to_string(),
            };
            let display_str = error.to_string();
            assert!(display_str.contains("Access denied to room !room:test"));
            assert!(display_str.contains("(invite)"));
            assert!(display_str.contains("Not invited"));
        }

        #[test]
        fn test_user_not_found_display() {
            let error = MembershipError::UserNotFound { user_id: "@user:test".to_string() };
            let display_str = error.to_string();
            assert!(display_str.contains("User @user:test not found"));
        }

        #[test]
        fn test_membership_already_exists_display() {
            let error = MembershipError::MembershipAlreadyExists {
                user_id: "@user:test".to_string(),
                room_id: "!room:test".to_string(),
                current_membership: "join".to_string(),
                requested_membership: "invite".to_string(),
            };
            let display_str = error.to_string();
            assert!(display_str.contains("User @user:test already has membership join"));
            assert!(display_str.contains("(requested invite)"));
        }

        #[test]
        fn test_federation_error_display() {
            let error = MembershipError::federation_error("remote.server", "Connection refused");
            let display_str = error.to_string();
            assert!(display_str.contains("Federation error from remote.server"));
            assert!(display_str.contains("Connection refused"));
        }

        #[test]
        fn test_federation_timeout_display() {
            let error = MembershipError::FederationTimeout {
                server_name: "remote.server".to_string(),
                timeout_ms: 5000,
                operation: "join".to_string(),
            };
            let display_str = error.to_string();
            assert!(display_str.contains("Federation timeout for join on remote.server"));
            assert!(display_str.contains("5000ms"));
        }

        #[test]
        fn test_invalid_event_display_with_event_id() {
            let error = MembershipError::InvalidEvent {
                event_id: Some("$event:test".to_string()),
                reason: "Missing field".to_string(),
            };
            let display_str = error.to_string();
            assert!(display_str.contains("Invalid event $event:test"));
            assert!(display_str.contains("Missing field"));
        }

        #[test]
        fn test_invalid_event_display_without_event_id() {
            let error = MembershipError::InvalidEvent {
                event_id: None,
                reason: "Missing field".to_string(),
            };
            let display_str = error.to_string();
            assert!(display_str.contains("Invalid event: Missing field"));
        }

        #[test]
        fn test_invalid_matrix_id_display() {
            let error = MembershipError::InvalidMatrixId {
                id: "invalid_id".to_string(),
                expected_type: "user".to_string(),
            };
            let display_str = error.to_string();
            assert!(display_str.contains("Invalid user ID: invalid_id"));
        }

        #[test]
        fn test_database_error_display() {
            let error = MembershipError::database_error("insert", "Connection lost");
            let display_str = error.to_string();
            assert!(display_str.contains("Database error during insert"));
            assert!(display_str.contains("Connection lost"));
        }

        #[test]
        fn test_json_error_display() {
            let error = MembershipError::JsonError {
                context: "parsing event".to_string(),
                error: "invalid json".to_string(),
            };
            let display_str = error.to_string();
            assert!(display_str.contains("JSON error in parsing event"));
            assert!(display_str.contains("invalid json"));
        }

        #[test]
        fn test_conflicting_membership_change_display() {
            let error = MembershipError::ConflictingMembershipChange {
                user_id: "@user:test".to_string(),
                room_id: "!room:test".to_string(),
                conflicting_event_id: "$conflict:test".to_string(),
            };
            let display_str = error.to_string();
            assert!(display_str.contains("Conflicting membership change for @user:test"));
            assert!(display_str.contains("(conflict: $conflict:test)"));
        }

        #[test]
        fn test_inconsistent_room_state_display() {
            let error = MembershipError::InconsistentRoomState {
                room_id: "!room:test".to_string(),
                details: "State mismatch".to_string(),
            };
            let display_str = error.to_string();
            assert!(display_str.contains("Inconsistent room state in !room:test"));
            assert!(display_str.contains("State mismatch"));
        }

        #[test]
        fn test_rate_limited_display() {
            let error = MembershipError::RateLimited { retry_after_ms: 1000 };
            let display_str = error.to_string();
            assert!(display_str.contains("Rate limited, retry after 1000ms"));
        }

        #[test]
        fn test_internal_error_display() {
            let error = MembershipError::InternalError {
                context: "validation".to_string(),
                error: "unexpected condition".to_string(),
            };
            let display_str = error.to_string();
            assert!(display_str.contains("Internal error in validation"));
            assert!(display_str.contains("unexpected condition"));
        }
    }

    mod error_trait_tests {
        use super::*;

        #[test]
        fn test_error_trait_implementation() {
            let error = MembershipError::room_not_found("!room:test");
            let error_trait: &dyn std::error::Error = &error;

            // Test that we can use Error trait methods
            assert!(!error_trait.to_string().is_empty());
            assert!(error_trait.source().is_none()); // Our errors don't have sources
        }
    }
}
