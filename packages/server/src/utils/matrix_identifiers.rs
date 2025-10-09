use crate::config::ServerConfig;
use std::net::{Ipv4Addr, Ipv6Addr};
use uuid::Uuid;

/// Parsed Matrix server name with hostname, port, and IP literal detection
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedServerName {
    /// The hostname or IP address part
    pub hostname: String,
    /// The port number (defaults to 8448 if not specified)
    pub port: u16,
    /// Whether the hostname is an IP literal (affects certificate validation)
    pub is_ip_literal: bool,
    /// The original server name as provided
    pub original: String,
}

impl ParsedServerName {
    /// Get the hostname for certificate validation
    #[allow(dead_code)]
    pub fn cert_hostname(&self) -> &str {
        &self.hostname
    }

    /// Get the Host header value for HTTP requests
    #[allow(dead_code)]
    pub fn host_header(&self) -> String {
        if self.port == 8448 {
            self.hostname.clone()
        } else {
            format!("{}:{}", self.hostname, self.port)
        }
    }

    /// Get the server name for Matrix protocol usage
    #[allow(dead_code)]
    pub fn server_name(&self) -> String {
        if self.port == 8448 {
            self.hostname.clone()
        } else {
            format!("{}:{}", self.hostname, self.port)
        }
    }
}

/// Get the configured server name from ServerConfig
pub fn get_server_name() -> &'static str {
    match ServerConfig::get() {
        Ok(config) => &config.homeserver_name,
        Err(_) => {
            tracing::error!("ServerConfig not initialized, falling back to localhost");
            "localhost"
        },
    }
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
#[allow(dead_code)] // Utility function for Matrix ID formatting
pub fn format_user_id(localpart: &str) -> String {
    format!("@{}:{}", localpart, get_server_name())
}

/// Format the system user ID for server-generated events
///
/// # Returns
/// * System user ID: `@system:server.name`
#[allow(dead_code)] // Utility function for system user ID formatting
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
#[allow(dead_code)] // Utility function for Matrix event ID formatting
pub fn format_event_id(localpart: &str) -> String {
    format!("${}:{}", localpart, get_server_name())
}

/// Generate a new Matrix event ID with UUID localpart
///
/// # Returns
/// * New Matrix event ID: `${uuid}:server.name`
#[allow(dead_code)] // Utility function for generating unique event IDs
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

    // Use the comprehensive parser for validation
    parse_server_name(server_name).is_ok()
}

/// Parse a Matrix server name according to specification
///
/// Handles IPv4, IPv6 (with brackets), and hostname formats with optional ports.
/// Defaults to port 8448 when not specified.
///
/// # Arguments
/// * `server_name` - The server name to parse (e.g., "example.com:8448", "[::1]:8080")
///
/// # Returns
/// * `Ok(ParsedServerName)` if parsing succeeds
/// * `Err(String)` with error description if parsing fails
pub fn parse_server_name(server_name: &str) -> Result<ParsedServerName, String> {
    if server_name.is_empty() {
        return Err("Server name cannot be empty".to_string());
    }

    let original = server_name.to_string();

    // Handle IPv6 literals with brackets: [::1]:8080 or [::1]
    if server_name.starts_with('[') {
        return parse_ipv6_server_name(server_name, original);
    }

    // Handle IPv4 and hostname formats: example.com:8080 or 192.168.1.1:8080
    parse_hostname_or_ipv4_server_name(server_name, original)
}

/// Parse IPv6 server name with brackets
fn parse_ipv6_server_name(server_name: &str, original: String) -> Result<ParsedServerName, String> {
    if !server_name.starts_with('[') {
        return Err("IPv6 address must start with '['".to_string());
    }

    // Find the closing bracket
    let bracket_end = server_name
        .find(']')
        .ok_or_else(|| "IPv6 address missing closing ']'".to_string())?;

    if bracket_end == 1 {
        return Err("IPv6 address cannot be empty".to_string());
    }

    // Extract IPv6 address (skip opening '[' and take up to closing ']')
    let ipv6_part = server_name
        .strip_prefix('[')
        .and_then(|s| s.get(..bracket_end - 1))
        .ok_or_else(|| "Failed to extract IPv6 address".to_string())?;

    // Get remainder after closing ']'
    let remainder = server_name
        .get(bracket_end + 1..)
        .ok_or_else(|| "Failed to get remainder after IPv6 address".to_string())?;

    // Validate IPv6 address
    let _ipv6: Ipv6Addr = ipv6_part
        .parse()
        .map_err(|_| format!("Invalid IPv6 address: {}", ipv6_part))?;

    // Parse port if present
    let port = if remainder.is_empty() {
        8448
    } else if let Some(port_str) = remainder.strip_prefix(':') {
        if port_str.is_empty() {
            return Err("Port cannot be empty after ':'".to_string());
        }
        port_str
            .parse::<u16>()
            .map_err(|_| format!("Invalid port number: {}", port_str))?
    } else {
        return Err("Invalid characters after IPv6 address".to_string());
    };

    Ok(ParsedServerName {
        hostname: format!("[{}]", ipv6_part),
        port,
        is_ip_literal: true,
        original,
    })
}

/// Parse hostname or IPv4 server name
fn parse_hostname_or_ipv4_server_name(
    server_name: &str,
    original: String,
) -> Result<ParsedServerName, String> {
    // Split on the last colon to handle IPv4 addresses correctly
    let (hostname, port) = if let Some(colon_pos) = server_name.rfind(':') {
        let hostname_part = server_name
            .get(..colon_pos)
            .ok_or_else(|| "Failed to extract hostname part".to_string())?;
        let port_part = server_name
            .get(colon_pos + 1..)
            .ok_or_else(|| "Failed to extract port part".to_string())?;

        if port_part.is_empty() {
            return Err("Port cannot be empty after ':'".to_string());
        }

        let port = port_part
            .parse::<u16>()
            .map_err(|_| format!("Invalid port number: {}", port_part))?;

        (hostname_part, port)
    } else {
        (server_name, 8448)
    };

    if hostname.is_empty() {
        return Err("Hostname cannot be empty".to_string());
    }

    // Check if it's an IPv4 literal
    let is_ip_literal = hostname.parse::<Ipv4Addr>().is_ok();

    // Basic hostname validation for non-IP literals
    if !is_ip_literal && !is_valid_hostname(hostname) {
        return Err(format!("Invalid hostname: {}", hostname));
    }

    Ok(ParsedServerName {
        hostname: hostname.to_string(),
        port,
        is_ip_literal,
        original,
    })
}

/// Validate hostname according to basic DNS rules
fn is_valid_hostname(hostname: &str) -> bool {
    if hostname.is_empty() || hostname.len() > 253 {
        return false;
    }

    // Must contain at least one dot for domain names
    if !hostname.contains('.') {
        return false;
    }

    // Basic character validation
    hostname.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        && !hostname.starts_with('-')
        && !hostname.ends_with('-')
        && !hostname.starts_with('.')
        && !hostname.ends_with('.')
}

/// Detect if a string represents an IP literal (IPv4 or IPv6)
///
/// # Arguments
/// * `hostname` - The hostname to check
///
/// # Returns
/// * `true` if the hostname is an IP literal, `false` otherwise
#[allow(dead_code)] // Utility function for IP literal detection
pub fn is_ip_literal(hostname: &str) -> bool {
    // Check for IPv6 with brackets
    if hostname.starts_with('[') && hostname.ends_with(']') {
        let ipv6_part = &hostname[1..hostname.len() - 1];
        return ipv6_part.parse::<Ipv6Addr>().is_ok();
    }

    // Check for IPv4
    hostname.parse::<Ipv4Addr>().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hostname_with_port() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = parse_server_name("example.com:8448")?;
        assert_eq!(parsed.hostname, "example.com");
        assert_eq!(parsed.port, 8448);
        assert!(!parsed.is_ip_literal);
        Ok(())
    }

    #[test]
    fn test_parse_hostname_without_port() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = parse_server_name("example.com")?;
        assert_eq!(parsed.hostname, "example.com");
        assert_eq!(parsed.port, 8448);
        assert!(!parsed.is_ip_literal);
        Ok(())
    }

    #[test]
    fn test_parse_ipv4_with_port() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = parse_server_name("192.168.1.1:8080")?;
        assert_eq!(parsed.hostname, "192.168.1.1");
        assert_eq!(parsed.port, 8080);
        assert!(parsed.is_ip_literal);
        Ok(())
    }

    #[test]
    fn test_parse_ipv6_with_port() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = parse_server_name("[::1]:8080")?;
        assert_eq!(parsed.hostname, "[::1]");
        assert_eq!(parsed.port, 8080);
        assert!(parsed.is_ip_literal);
        Ok(())
    }

    #[test]
    fn test_parse_ipv6_without_port() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = parse_server_name("[2001:db8::1]")?;
        assert_eq!(parsed.hostname, "[2001:db8::1]");
        assert_eq!(parsed.port, 8448);
        assert!(parsed.is_ip_literal);
        Ok(())
    }

    #[test]
    fn test_invalid_server_names() {
        assert!(parse_server_name("").is_err());
        assert!(parse_server_name(":8080").is_err());
        assert!(parse_server_name("example.com:").is_err());
        assert!(parse_server_name("[::1").is_err());
        assert!(parse_server_name("::1]").is_err());
        assert!(parse_server_name("invalid_hostname").is_err());
    }

    #[test]
    fn test_is_ip_literal() {
        assert!(is_ip_literal("192.168.1.1"));
        assert!(is_ip_literal("[::1]"));
        assert!(is_ip_literal("[2001:db8::1]"));
        assert!(!is_ip_literal("example.com"));
        assert!(!is_ip_literal("localhost"));
    }

    #[test]
    fn test_enhanced_server_name_validation() {
        assert!(is_valid_server_name("example.com"));
        assert!(is_valid_server_name("matrix.example.org"));
        assert!(is_valid_server_name("192.168.1.1:8448"));
        assert!(is_valid_server_name("[::1]:8080"));
        assert!(!is_valid_server_name("localhost"));
        assert!(!is_valid_server_name(""));
        assert!(!is_valid_server_name("invalid"));
    }

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
}
