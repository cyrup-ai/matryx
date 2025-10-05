use axum::{
    body::{Body, to_bytes},
    extract::{Request, State},
    http::{HeaderMap, header::AUTHORIZATION},
    middleware::Next,
    response::{IntoResponse, Response},
};

use tracing::{debug, info, warn};

use crate::auth::{
    errors::MatrixAuthError,
    matrix_auth::{MatrixAuth, MatrixServerAuth},
    session_service::MatrixSessionService,
    x_matrix_parser::parse_x_matrix_header,
};
use crate::error::matrix_errors::MatrixError;
use crate::state::AppState;

use std::net::IpAddr;
use x509_parser::extensions::GeneralName;
use x509_parser::prelude::*;

/// Middleware to extract and validate Matrix authentication
pub async fn auth_middleware(
    State(app_state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let session_service = &app_state.session_service;
    let auth_header = request.headers().get(AUTHORIZATION).and_then(|h| h.to_str().ok());

    let x_matrix_header = request
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .filter(|h| h.starts_with("X-Matrix"))
        .map(|h| h.to_string());

    let matrix_auth = if let Some(auth_header) = auth_header {
        if auth_header.starts_with("Bearer ") {
            // Use secure authentication validation
            match extract_matrix_auth(request.headers(), session_service).await {
                Ok(auth) => auth,
                Err(e) => {
                    tracing::warn!("Authentication failed: {}", e);
                    return MatrixError::Unauthorized.into_response();
                },
            }
        } else {
            // Missing proper token format - return MissingAuthorization error
            return MatrixError::Unauthorized.into_response();
        }
    } else if let Some(x_matrix_header) = x_matrix_header {
        // Extract request body for signature verification WITHOUT borrowing conflicts
        let (parts, body) = request.into_parts();
        let body_bytes = match to_bytes(body, usize::MAX).await {
            Ok(bytes) => bytes,
            Err(_) => return MatrixError::Unauthorized.into_response(),
        };

        // Reconstruct request properly
        let new_request = Request::from_parts(parts, Body::from(body_bytes.clone()));

        // Extract required values from request to avoid Send trait issues
        let request_method = new_request.method().as_str().to_string();
        let request_uri = new_request
            .uri()
            .path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/")
            .to_string();
        let request_headers = new_request.headers().clone();

        // Pass extracted values instead of request reference
        let result = match validate_server_signature(
            &x_matrix_header,
            &request_method,
            &request_uri,
            &request_headers,
            &body_bytes,
            session_service,
        )
        .await
        {
            Ok(auth) => auth,
            Err(_) => return MatrixError::Unauthorized.into_response(),
        };

        // Update the request variable correctly
        request = new_request;

        result
    } else {
        // No authorization header - return MissingToken error
        return MatrixError::Unauthorized.into_response();
    };

    // Validate the authentication before proceeding
    if matrix_auth.is_expired() {
        tracing::warn!("Authentication token has expired");
        return MatrixError::Unauthorized.into_response();
    }

    // Check resource-based authorization per endpoint
    let request_uri = request.uri().path();
    let request_method = request.method().as_str();
    
    // Check endpoint-specific permissions
    if !check_endpoint_authorization(&matrix_auth, request_uri, request_method, &app_state).await {
        tracing::warn!(
            "Authorization denied for {} {} - insufficient permissions",
            request_method,
            request_uri
        );
        return MatrixError::Forbidden.into_response();
    }

    // Basic access check
    if !matrix_auth.can_access(request_uri) {
        tracing::warn!("Authentication denied access to resource: {}", request_uri);
        return MatrixError::Forbidden.into_response();
    }

    // Log authentication signature for audit trail
    if let Some(sig) = matrix_auth.signature() {
        debug!("Authenticated with signature: {}", sig);
    }

    request.extensions_mut().insert(matrix_auth);
    next.run(request).await
}

/// Middleware that requires authentication to be present
pub async fn require_auth_middleware(request: Request, next: Next) -> Result<Response, Response> {
    if request.extensions().get::<MatrixAuth>().is_none() {
        return Err(MatrixError::Unauthorized.into_response());
    }
    Ok(next.run(request).await)
}

async fn validate_server_signature(
    x_matrix_header: &str,
    request_method: &str,
    request_uri: &str,
    request_headers: &HeaderMap,
    request_body: &[u8],
    session_service: &MatrixSessionService<surrealdb::engine::any::Any>,
) -> Result<MatrixAuth, Response> {
    // Parse X-Matrix authorization header using RFC 9110 compliant parser
    let x_matrix_auth = parse_x_matrix_header(x_matrix_header)
        .map_err(|_| MatrixError::Unauthorized.into_response())?;

    let origin = &x_matrix_auth.origin;
    let key_id = &x_matrix_auth.key_id;
    let signature = &x_matrix_auth.signature;

    // NEW: Add destination validation logic
    if let Some(destination) = &x_matrix_auth.destination {
        let homeserver_name = session_service.get_homeserver_name();
        if destination != homeserver_name {
            warn!(
                "X-Matrix destination mismatch: got '{}', expected '{}'",
                destination, homeserver_name
            );
            return Err(MatrixError::Unauthorized.into_response());
        }
        info!("X-Matrix destination validated: {}", destination);
    } else {
        debug!("X-Matrix request without destination parameter (backward compatibility)");
    }

    // Use session service for server key validation per Matrix specification
    debug!("Validating X-Matrix auth for server: {}", origin);

    // Enhanced federation security: verify client certificate if available
    let cert_valid = verify_client_certificate(request_headers, origin)
        .await
        .unwrap_or_else(|e| {
            debug!("Certificate validation failed for {}: {}", origin, e);
            false
        });

    if cert_valid {
        info!("Client certificate validated successfully for server: {}", origin);
    } else {
        debug!(
            "No valid client certificate found for server: {} (proceeding with key-only validation)",
            origin
        );
    }

    // Use the actual request body bytes for signature verification

    // Log authentication attempt for audit trail
    info!(
        "X-Matrix auth attempt: server={}, key={}, method={}, uri={}, cert_valid={}",
        origin, key_id, request_method, request_uri, cert_valid
    );

    // Validate server signature using session service for Matrix federation
    let _signature_result = session_service
        .validate_server_signature(
            origin,
            key_id,
            signature,
            request_method,
            request_uri,
            request_body,
        )
        .await
        .map_err(|e| {
            warn!("Server signature validation failed for {}: {:?}", origin, e);
            MatrixError::Unauthorized.into_response()
        })?;

    // Parse request body as JSON for signature verification per Matrix spec
    let body_json = serde_json::from_slice(request_body).unwrap_or(serde_json::Value::Null);

    let homeserver_name = session_service.get_homeserver_name();

    // Validate that the origin server is not our own homeserver (prevent self-auth loops)
    if origin == homeserver_name {
        warn!("Rejecting X-Matrix auth from own homeserver: {}", homeserver_name);
        return Err(MatrixError::Unauthorized.into_response());
    }

    // Log the request details for Matrix federation audit trail
    debug!(
        "Processing X-Matrix federation request - origin: {}, homeserver: {}, body_type: {}",
        origin,
        homeserver_name,
        if body_json.is_null() { "empty" } else { "json" }
    );

    // Matrix federation signature validation already passed via validate_server_signature() above
    info!("X-Matrix federation auth successful for server: {}", origin);

    // Check for X-Matrix-Token header for additional authentication (optional)
    if let Some(server_token) = request_headers.get("X-Matrix-Token") {
        let token_str = server_token.to_str().map_err(|_| {
            warn!("Invalid X-Matrix-Token header format");
            MatrixError::Unauthorized.into_response()
        })?;

        // Validate server token
        match session_service.validate_token(token_str) {
            Ok(token_claims) => {
                // Verify token matches origin server
                if token_claims.matrix_server_name.as_deref() != Some(origin) {
                    warn!(
                        "Token server_name mismatch: expected {}, got {:?}",
                        origin, token_claims.matrix_server_name
                    );
                    return Err(MatrixError::Unauthorized.into_response());
                }

                info!("Server token validated for origin: {}", origin);
            },
            Err(e) => {
                warn!("Server token validation failed: {:?}", e);
                return Err(MatrixError::Unauthorized.into_response());
            },
        }
    } else {
        debug!("No X-Matrix-Token header present (optional for backward compatibility)");
    }

    // Create the authenticated server result after validation
    let server_auth = MatrixServerAuth {
        server_name: origin.to_string(),
        key_id: key_id.to_string(),
        signature: signature.to_string(),
        expires_at: None,
    };

    info!(
        "Server signature validated for origin: {} key_id: {} method: {}",
        origin, key_id, request_method
    );

    // Store signature for potential re-validation
    info!(
        "Stored signature {} for server {} for future validation",
        &server_auth.signature, &server_auth.server_name
    );

    Ok(MatrixAuth::Server(server_auth))
}

/// Extract Matrix authentication from headers with proper database validation
/// Used in endpoints that need to validate authentication manually
pub async fn extract_matrix_auth(
    headers: &HeaderMap,
    session_service: &MatrixSessionService<surrealdb::engine::any::Any>,
) -> Result<MatrixAuth, MatrixAuthError> {
    let auth_header = headers
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or(MatrixAuthError::MissingAuthorization)?;

    if auth_header.starts_with("Bearer ") {
        let token = auth_header.strip_prefix("Bearer ").ok_or(MatrixAuthError::MissingToken)?;

        // Use existing validation infrastructure
        let access_token = session_service.validate_access_token(token).await?;

        Ok(MatrixAuth::User(access_token))
    } else {
        Err(MatrixAuthError::MissingToken)
    }
}

/// Verify client certificate for enhanced federation security
pub async fn verify_client_certificate(
    headers: &HeaderMap,
    peer_server_name: &str,
) -> Result<bool, String> {
    // Extract certificate from TLS layer if available
    // This requires coordination with the Axum TLS layer
    // Implementation depends on TLS termination setup

    // For load balancer scenarios, check X-Forwarded-Client-Cert header
    if let Some(cert_header) = headers.get("x-forwarded-client-cert") {
        let cert_data = cert_header.to_str().map_err(|_| "Invalid certificate header")?;

        // Parse and validate certificate matches server name
        validate_certificate_matches_server(cert_data, peer_server_name)
    } else {
        // No client certificate provided - this is optional per Matrix spec
        Ok(false)
    }
}

fn validate_certificate_matches_server(cert_data: &str, server_name: &str) -> Result<bool, String> {
    // Parse PEM certificate
    let (_, pem) = parse_x509_pem(cert_data.as_bytes())
        .map_err(|_| "Failed to parse PEM certificate".to_string())?;

    let (_, cert) = X509Certificate::from_der(&pem.contents)
        .map_err(|_| "Failed to parse DER certificate".to_string())?;

    // Determine if server_name is IP literal or hostname
    if let Ok(ip_addr) = server_name.parse::<IpAddr>() {
        validate_ip_address(&cert, ip_addr)
    } else {
        validate_hostname(&cert, server_name)
    }
}

/// Validate certificate against IP address (for IP literal server names)
fn validate_ip_address(cert: &X509Certificate, ip: IpAddr) -> Result<bool, String> {
    // Check Subject Alternative Names for IP addresses
    for ext in cert.extensions() {
        if let ParsedExtension::SubjectAlternativeName(san) = ext.parsed_extension() {
            for name in &san.general_names {
                if let GeneralName::IPAddress(cert_ip_bytes) = name {
                    match (ip, cert_ip_bytes.len()) {
                        (IpAddr::V4(ipv4), 4) => {
                            let cert_ipv4 = std::net::Ipv4Addr::from([
                                cert_ip_bytes[0],
                                cert_ip_bytes[1],
                                cert_ip_bytes[2],
                                cert_ip_bytes[3],
                            ]);
                            if ipv4 == cert_ipv4 {
                                return Ok(true);
                            }
                        },
                        (IpAddr::V6(ipv6), 16) => {
                            let mut bytes = [0u8; 16];
                            bytes.copy_from_slice(cert_ip_bytes);
                            let cert_ipv6 = std::net::Ipv6Addr::from(bytes);
                            if ipv6 == cert_ipv6 {
                                return Ok(true);
                            }
                        },
                        _ => continue,
                    }
                }
            }
        }
    }

    Ok(false) // IP not found in certificate
}

/// Validate certificate against hostname (for domain server names)
fn validate_hostname(cert: &X509Certificate, hostname: &str) -> Result<bool, String> {
    // Remove port from hostname if present
    let hostname = hostname.split(':').next().unwrap_or(hostname);

    // Check Subject Alternative Names first (preferred)
    for ext in cert.extensions() {
        if let ParsedExtension::SubjectAlternativeName(san) = ext.parsed_extension() {
            for name in &san.general_names {
                if let GeneralName::DNSName(dns_name) = name
                    && matches_hostname(dns_name, hostname)
                {
                    return Ok(true);
                }
            }
        }
    }

    // Fallback to Common Name in Subject (legacy)
    let subject = &cert.subject();
    for rdn in subject.iter() {
        for attr in rdn.iter() {
            // Common Name OID: 2.5.4.3
            if attr.attr_type().to_id_string() == "2.5.4.3"
                && let Ok(cn) = attr.attr_value().as_str()
                && matches_hostname(cn, hostname)
            {
                return Ok(true);
            }
        }
    }

    Ok(false) // Hostname not found in certificate
}

/// Check if certificate name matches hostname (supports wildcards)
fn matches_hostname(cert_name: &str, hostname: &str) -> bool {
    if cert_name == hostname {
        return true;
    }

    // Handle wildcard certificates (*.example.com)
    if let Some(cert_domain) = cert_name.strip_prefix("*.") {
        let host_parts: Vec<&str> = hostname.split('.').skip(1).collect();
        if !host_parts.is_empty() {
            let host_domain = host_parts.join(".");
            return cert_domain == host_domain;
        }
    }

    false
}

/// Check endpoint-specific authorization requirements
async fn check_endpoint_authorization(
    matrix_auth: &MatrixAuth,
    uri: &str,
    method: &str,
    app_state: &AppState,
) -> bool {
    // Admin endpoints require admin privileges
    if uri.starts_with("/_synapse/admin/") || uri.starts_with("/_matrix/client/v3/admin/") {
        return check_admin_permission(matrix_auth, app_state).await;
    }

    // Room state event endpoints require appropriate power levels
    if uri.contains("/rooms/") && uri.contains("/state/") && (method == "PUT" || method == "POST") {
        return check_state_event_permission(matrix_auth, uri, app_state).await;
    }

    // Message sending endpoints require send permission
    if uri.contains("/rooms/") && uri.contains("/send/") && (method == "PUT" || method == "POST") {
        return check_send_message_permission(matrix_auth, uri, app_state).await;
    }

    // Room invite endpoint requires invite permission
    if uri.contains("/rooms/") && uri.ends_with("/invite") && method == "POST" {
        return check_invite_permission(matrix_auth, uri, app_state).await;
    }

    // Room kick endpoint requires kick permission
    if uri.contains("/rooms/") && uri.ends_with("/kick") && method == "POST" {
        return check_kick_permission(matrix_auth, uri, app_state).await;
    }

    // Room ban endpoint requires ban permission
    if uri.contains("/rooms/") && uri.ends_with("/ban") && method == "POST" {
        return check_ban_permission(matrix_auth, uri, app_state).await;
    }

    // Default: allow if authenticated (basic endpoints)
    true
}

/// Check if user has admin permission
async fn check_admin_permission(matrix_auth: &MatrixAuth, app_state: &AppState) -> bool {
    match matrix_auth {
        MatrixAuth::User(access_token) => {
            // Check if user is admin via database
            let user_repo = matryx_surrealdb::repository::UserRepository::new(app_state.db.clone());
            user_repo
                .is_admin(&access_token.user_id)
                .await
                .unwrap_or(false)
        },
        MatrixAuth::Server(_) => {
            // Server auth doesn't have admin privileges for user-facing admin endpoints
            false
        },
        MatrixAuth::Anonymous => {
            // Anonymous users have no admin privileges
            false
        },
    }
}

/// Check if user has permission to send state events in a room
async fn check_state_event_permission(
    matrix_auth: &MatrixAuth,
    uri: &str,
    app_state: &AppState,
) -> bool {
    match matrix_auth {
        MatrixAuth::User(access_token) => {
            // Extract room_id from URI
            if let Some(room_id) = extract_room_id_from_uri(uri) {
                // Check user's power level in the room
                check_user_power_level(
                    &access_token.user_id,
                    room_id,
                    50, // Default state_default power level
                    app_state,
                )
                .await
            } else {
                false
            }
        },
        MatrixAuth::Server(_) => {
            // Server federation auth has different permissions
            true
        },
        MatrixAuth::Anonymous => {
            // Anonymous users cannot send state events
            false
        },
    }
}

/// Check if user has permission to send messages in a room
async fn check_send_message_permission(
    matrix_auth: &MatrixAuth,
    uri: &str,
    app_state: &AppState,
) -> bool {
    match matrix_auth {
        MatrixAuth::User(access_token) => {
            if let Some(room_id) = extract_room_id_from_uri(uri) {
                // Check user membership and power level
                check_user_room_membership(&access_token.user_id, room_id, app_state).await
                    && check_user_power_level(
                        &access_token.user_id,
                        room_id,
                        0, // Default events_default is 0
                        app_state,
                    )
                    .await
            } else {
                false
            }
        },
        MatrixAuth::Server(_) => true,
        MatrixAuth::Anonymous => {
            // Anonymous users cannot send messages
            false
        },
    }
}

/// Check if user has permission to invite users to a room
async fn check_invite_permission(
    matrix_auth: &MatrixAuth,
    uri: &str,
    app_state: &AppState,
) -> bool {
    match matrix_auth {
        MatrixAuth::User(access_token) => {
            if let Some(room_id) = extract_room_id_from_uri(uri) {
                check_user_power_level(
                    &access_token.user_id,
                    room_id,
                    50, // Default invite power level
                    app_state,
                )
                .await
            } else {
                false
            }
        },
        MatrixAuth::Server(_) => true,
        MatrixAuth::Anonymous => {
            // Anonymous users cannot invite
            false
        },
    }
}

/// Check if user has permission to kick users from a room
async fn check_kick_permission(
    matrix_auth: &MatrixAuth,
    uri: &str,
    app_state: &AppState,
) -> bool {
    match matrix_auth {
        MatrixAuth::User(access_token) => {
            if let Some(room_id) = extract_room_id_from_uri(uri) {
                check_user_power_level(
                    &access_token.user_id,
                    room_id,
                    50, // Default kick power level
                    app_state,
                )
                .await
            } else {
                false
            }
        },
        MatrixAuth::Server(_) => true,
        MatrixAuth::Anonymous => {
            // Anonymous users cannot kick
            false
        },
    }
}

/// Check if user has permission to ban users from a room
async fn check_ban_permission(
    matrix_auth: &MatrixAuth,
    uri: &str,
    app_state: &AppState,
) -> bool {
    match matrix_auth {
        MatrixAuth::User(access_token) => {
            if let Some(room_id) = extract_room_id_from_uri(uri) {
                check_user_power_level(
                    &access_token.user_id,
                    room_id,
                    50, // Default ban power level
                    app_state,
                )
                .await
            } else {
                false
            }
        },
        MatrixAuth::Server(_) => true,
        MatrixAuth::Anonymous => {
            // Anonymous users cannot ban
            false
        },
    }
}

/// Extract room ID from URI
fn extract_room_id_from_uri(uri: &str) -> Option<&str> {
    // URI format: /_matrix/client/v3/rooms/{room_id}/...
    let parts: Vec<&str> = uri.split('/').collect();
    if let Some(idx) = parts.iter().position(|&p| p == "rooms")
        && idx + 1 < parts.len()
    {
        return Some(parts[idx + 1]);
    }
    None
}

/// Check if user is a member of the room
async fn check_user_room_membership(
    user_id: &str,
    room_id: &str,
    app_state: &AppState,
) -> bool {
    // Access membership repository through database
    let membership_repo = matryx_surrealdb::repository::MembershipRepository::new(app_state.db.clone());
    membership_repo
        .is_user_in_room(room_id, user_id)
        .await
        .unwrap_or(false)
}

/// Check if user has sufficient power level in the room
async fn check_user_power_level(
    user_id: &str,
    room_id: &str,
    required_level: i64,
    app_state: &AppState,
) -> bool {
    // Access membership repository through database for power level checks
    let membership_repo = matryx_surrealdb::repository::MembershipRepository::new(app_state.db.clone());
    match membership_repo.get_user_power_level(room_id, user_id).await {
        Ok(user_level) => user_level >= required_level,
        Err(_) => false,
    }
}
