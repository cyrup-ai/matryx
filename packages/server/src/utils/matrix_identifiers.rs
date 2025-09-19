use crate::config::ServerConfig;
use uuid::Uuid;

/// Get the configured server name from ServerConfig
pub fn get_server_name() -> &'static str {
    &ServerConfig::get().homeserver_name
}

/// Format a Matrix room ID with the configured server name
/// 
/// # Arguments
/// * `localpart` - The local part of the room ID (without ! prefix)
/// 
/// # Returns
/// * Properly formatted Matrix room ID: `!localpart:server.name`
pub fn format_room_id(localpart: &str) -> String {
    format!("!{}:{}", localpart, get_server_name())
}

/// Generate a new Matrix room ID with UUID localpart
/// 
/// # Returns
/// * New Matrix room ID: `!{uuid}:server.name`
pub fn generate_room_id() -> String {
    format_room_id(&Uuid::new_v4().to_string())
}

/// Format a Matrix user ID with the configured server name
/// 
/// # Arguments
/// * `localpart` - The local part of the user ID (without @ prefix)
/// 
/// # Returns
/// * Properly formatted Matrix user ID: `@localpart:server.name`
pub fn format_user_id(localpart: &str) -> String {
    format!("@{}:{}", localpart, get_server_name())
}

/// Format the system user ID for server-generated events
/// 
/// # Returns
/// * System user ID: `@system:server.name`
pub fn format_system_user_id() -> String {
    format_user_id("system")
}

/// Format a Matrix event ID with the configured server name
/// 
/// # Arguments
/// * `localpart` - The local part of the event ID (without $ prefix)
/// 
/// # Returns
/// * Properly formatted Matrix event ID: `$localpart:server.name`
pub fn format_event_id(localpart: &str) -> String {
    format!("${}:{}", localpart, get_server_name())
}

/// Generate a new Matrix event ID with UUID localpart
/// 
/// # Returns
/// * New Matrix event ID: `${uuid}:server.name`
pub fn generate_event_id() -> String {
    format_event_id(&Uuid::new_v4().to_string())
}

/// Validate Matrix server name format according to specification
/// 
/// # Arguments
/// * `server_name` - Server name to validate
/// 
/// # Returns
/// * `true` if server name is valid, `false` otherwise
pub fn is_valid_server_name(server_name: &str) -> bool {
    if server_name.is_empty() || server_name == "localhost" {
        return false;
    }
    
    // Basic validation: must contain at least one dot (domain) or be IP:port
    server_name.contains('.') || server_name.parse::<std::net::IpAddr>().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_room_id_generation() {
        let room_id = format_room_id("test123");
        assert!(room_id.starts_with("!test123:"));
        assert!(!room_id.contains("localhost"));
    }

    #[test]
    fn test_system_user_id() {
        let system_id = format_system_user_id();
        assert!(system_id.starts_with("@system:"));
        assert!(!system_id.contains("localhost"));
    }

    #[test]
    fn test_server_name_validation() {
        assert!(is_valid_server_name("example.com"));
        assert!(is_valid_server_name("matrix.example.org"));
        assert!(is_valid_server_name("192.168.1.1:8448"));
        assert!(!is_valid_server_name("localhost"));
        assert!(!is_valid_server_name(""));
        assert!(!is_valid_server_name("invalid"));
    }
}