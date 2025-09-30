//! RFC 9110 Compliant X-Matrix Authorization Header Parser
//!
//! Implements proper parsing of Matrix server-to-server authentication headers
//! according to RFC 9110 Section 11.4 format, replacing basic split-based parsers.

use std::collections::HashMap;

/// X-Matrix authentication parameters extracted from Authorization header
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XMatrixAuth {
    pub origin: String,
    pub destination: Option<String>,  // v1.3+ required
    pub key_id: String,
    pub signature: String,
}

/// Errors that can occur during X-Matrix header parsing
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XMatrixParseError {
    /// Header does not start with "X-Matrix "
    InvalidScheme,
    /// Malformed parameter syntax
    InvalidParameterSyntax,
    /// Invalid token characters
    InvalidToken,
    /// Unterminated quoted string
    UnterminatedQuotedString,
    /// Invalid escape sequence in quoted string
    #[allow(dead_code)]
    InvalidEscapeSequence,
    /// Missing required parameter
    MissingRequiredParameter(String),
    /// Invalid parameter value format
    InvalidParameterValue(String),
}

impl std::fmt::Display for XMatrixParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XMatrixParseError::InvalidScheme => {
                write!(f, "Authorization header must start with 'X-Matrix '")
            }
            XMatrixParseError::InvalidParameterSyntax => {
                write!(f, "Invalid parameter syntax in X-Matrix header")
            }
            XMatrixParseError::InvalidToken => {
                write!(f, "Invalid token characters in parameter")
            }
            XMatrixParseError::UnterminatedQuotedString => {
                write!(f, "Unterminated quoted string in parameter value")
            }
            XMatrixParseError::InvalidEscapeSequence => {
                write!(f, "Invalid escape sequence in quoted string")
            }
            XMatrixParseError::MissingRequiredParameter(param) => {
                write!(f, "Missing required parameter: {}", param)
            }
            XMatrixParseError::InvalidParameterValue(param) => {
                write!(f, "Invalid value for parameter: {}", param)
            }
        }
    }
}

impl std::error::Error for XMatrixParseError {}

/// Parse state for RFC 9110 compliant parameter parsing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseState {
    ExpectingParamName,
    ExpectingEquals,
    ExpectingParamValue,
    InQuotedString,
    AfterBackslash,
    ExpectingCommaOrEnd,
}

/// Parse X-Matrix Authorization header according to RFC 9110
///
/// # Arguments
/// * `auth_header` - Full Authorization header value (including "X-Matrix " scheme)
///
/// # Returns
/// * `Ok(XMatrixAuth)` - Successfully parsed authentication parameters
/// * `Err(XMatrixParseError)` - Parsing error with details
///
/// # Example
/// ```
/// let header = "X-Matrix origin=example.com,key=ed25519:abc123,sig=def456";
/// let auth = parse_x_matrix_header(header)?;
/// assert_eq!(auth.origin, "example.com");
/// ```
pub fn parse_x_matrix_header(auth_header: &str) -> Result<XMatrixAuth, XMatrixParseError> {
    // Validate X-Matrix scheme
    let params_str = auth_header
        .strip_prefix("X-Matrix ")
        .ok_or(XMatrixParseError::InvalidScheme)?;

    // Parse parameters using RFC 9110 compliant logic
    let params = parse_auth_params(params_str)?;

    // Extract required Matrix parameters
    let origin = params
        .get("origin")
        .ok_or_else(|| XMatrixParseError::MissingRequiredParameter("origin".to_string()))?
        .clone();

    let key_param = params
        .get("key")
        .ok_or_else(|| XMatrixParseError::MissingRequiredParameter("key".to_string()))?;

    // Parse key parameter - must be in format "ed25519:keyid"  
    let key_id = if let Some(key_id) = key_param.strip_prefix("ed25519:") {
        key_id.to_string()
    } else {
        return Err(XMatrixParseError::InvalidParameterValue("key".to_string()));
    };

    let signature = params
        .get("sig")
        .ok_or_else(|| XMatrixParseError::MissingRequiredParameter("sig".to_string()))?
        .clone();

    // Destination parameter is optional for backward compatibility
    let destination = params.get("destination").cloned();

    Ok(XMatrixAuth {
        origin,
        destination,
        key_id,
        signature,
    })
}/// Parse authentication parameters according to RFC 9110 Section 11.4
///
/// Handles comma-separated name=value pairs with proper quoted-string support
/// and Matrix compatibility mode for unquoted colons.
fn parse_auth_params(params_str: &str) -> Result<HashMap<String, String>, XMatrixParseError> {
    let mut params = HashMap::new();
    let mut chars = params_str.chars().peekable();
    let mut state = ParseState::ExpectingParamName;
    let mut current_name = String::new();
    let mut current_value = String::new();

    // Skip initial whitespace
    while chars.peek() == Some(&' ') || chars.peek() == Some(&'\t') {
        chars.next();
    }

    while let Some(ch) = chars.next() {
        match state {
            ParseState::ExpectingParamName => {
                if is_valid_token_char_with_matrix_compat(ch) {
                    current_name.push(ch);
                } else if ch == '=' {
                    if current_name.is_empty() {
                        return Err(XMatrixParseError::InvalidParameterSyntax);
                    }
                    state = ParseState::ExpectingParamValue;
                } else if ch == ' ' || ch == '\t' {
                    if !current_name.is_empty() {
                        state = ParseState::ExpectingEquals;
                    }
                    // Skip whitespace
                } else {
                    return Err(XMatrixParseError::InvalidToken);
                }
            }

            ParseState::ExpectingEquals => {
                if ch == '=' {
                    state = ParseState::ExpectingParamValue;
                } else if ch == ' ' || ch == '\t' {
                    // Skip whitespace before equals
                } else {
                    return Err(XMatrixParseError::InvalidParameterSyntax);
                }
            }

            ParseState::ExpectingParamValue => {
                if ch == '"' {
                    // Start of quoted string
                    state = ParseState::InQuotedString;
                } else if ch == ' ' || ch == '\t' {
                    // Skip whitespace before value
                } else if is_valid_token_char_with_matrix_compat(ch) {
                    // Unquoted token value
                    current_value.push(ch);
                    state = ParseState::ExpectingCommaOrEnd;
                } else {
                    return Err(XMatrixParseError::InvalidParameterSyntax);
                }
            }

            ParseState::InQuotedString => {
                if ch == '"' {
                    // End of quoted string
                    state = ParseState::ExpectingCommaOrEnd;
                } else if ch == '\\' {
                    state = ParseState::AfterBackslash;
                } else {
                    // Regular character in quoted string
                    current_value.push(ch);
                }
            }

            ParseState::AfterBackslash => {
                // RFC 9110: any character can be escaped in quoted-string
                current_value.push(ch);
                state = ParseState::InQuotedString;
            }

            ParseState::ExpectingCommaOrEnd => {
                if ch == ',' {
                    // Complete current parameter
                    params.insert(current_name.trim().to_lowercase(), current_value.clone());
                    current_name.clear();
                    current_value.clear();
                    state = ParseState::ExpectingParamName;
                    
                    // Skip whitespace after comma
                    while chars.peek() == Some(&' ') || chars.peek() == Some(&'\t') {
                        chars.next();
                    }
                } else if ch == ' ' || ch == '\t' {
                    // Skip trailing whitespace
                } else if is_valid_token_char_with_matrix_compat(ch) {
                    // Continue building unquoted token value
                    current_value.push(ch);
                } else {
                    return Err(XMatrixParseError::InvalidParameterSyntax);
                }
            }
        }
    }

    // Handle end of input
    match state {
        ParseState::InQuotedString | ParseState::AfterBackslash => {
            return Err(XMatrixParseError::UnterminatedQuotedString);
        }
        ParseState::ExpectingParamValue => {
            return Err(XMatrixParseError::InvalidParameterSyntax);
        }
        ParseState::ExpectingCommaOrEnd | ParseState::ExpectingEquals => {
            if !current_name.is_empty() {
                params.insert(current_name.trim().to_lowercase(), current_value.clone());
            }
        }
        ParseState::ExpectingParamName => {
            // Empty parameters string is valid
        }
    }

    Ok(params)
}

/// Check if character is valid in token with Matrix compatibility
///
/// Implements RFC 9110 Section 5.6.2 tchar definition plus Matrix compatibility
/// for colons in unquoted values (for older server implementations).
fn is_valid_token_char_with_matrix_compat(ch: char) -> bool {
    match ch {
        // RFC 9110 tchar characters
        '!' | '#' | '$' | '%' | '&' | '\'' | '*' | '+' | '-' | '.' |
        '^' | '_' | '`' | '|' | '~' => true,
        // Matrix compatibility: allow colons in unquoted values
        ':' => true,
        // ALPHA / DIGIT
        c if c.is_ascii_alphanumeric() => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_x_matrix_parsing() {
        let header = "X-Matrix origin=example.com,key=ed25519:abc123,sig=def456";
        let auth = parse_x_matrix_header(header).unwrap();
        
        assert_eq!(auth.origin, "example.com");
        assert_eq!(auth.key_id, "abc123");
        assert_eq!(auth.signature, "def456");
        assert_eq!(auth.destination, None);
    }

    #[test]
    fn test_quoted_values() {
        let header = r#"X-Matrix origin="example.com",key="ed25519:abc123",sig="def,456""#;
        let auth = parse_x_matrix_header(header).unwrap();
        
        assert_eq!(auth.origin, "example.com");
        assert_eq!(auth.key_id, "abc123");
        assert_eq!(auth.signature, "def,456");
    }

    #[test]
    fn test_escaped_quotes() {
        let header = r#"X-Matrix origin=example.com,key=ed25519:abc123,sig="def\"456""#;
        let auth = parse_x_matrix_header(header).unwrap();
        
        assert_eq!(auth.signature, "def\"456");
    }

    #[test]
    fn test_matrix_compatibility_colons() {
        let header = "X-Matrix origin=matrix.example.com:8448,key=ed25519:abc123,sig=def456";
        let auth = parse_x_matrix_header(header).unwrap();
        
        assert_eq!(auth.origin, "matrix.example.com:8448");
    }

    #[test]
    fn test_destination_parameter() {
        let header = "X-Matrix origin=example.com,destination=target.com,key=ed25519:abc123,sig=def456";
        let auth = parse_x_matrix_header(header).unwrap();
        
        assert_eq!(auth.destination, Some("target.com".to_string()));
    }

    #[test]
    fn test_invalid_scheme() {
        let header = "Bearer token123";
        let result = parse_x_matrix_header(header);
        
        assert!(matches!(result, Err(XMatrixParseError::InvalidScheme)));
    }

    #[test]
    fn test_missing_required_parameter() {
        let header = "X-Matrix origin=example.com,key=ed25519:abc123";
        let result = parse_x_matrix_header(header);
        
        assert!(matches!(result, Err(XMatrixParseError::MissingRequiredParameter(_))));
    }

    #[test]
    fn test_invalid_key_format() {
        let header = "X-Matrix origin=example.com,key=rsa:abc123,sig=def456";
        let result = parse_x_matrix_header(header);
        
        assert!(matches!(result, Err(XMatrixParseError::InvalidParameterValue(_))));
    }
}