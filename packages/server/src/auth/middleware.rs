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

use x509_parser::prelude::*;
use x509_parser::extensions::GeneralName;
use std::net::IpAddr;

/// Middleware to extract and validate Matrix authentication
pub async fn auth_middleware(
    State(session_service): State<MatrixSessionService<surrealdb::engine::any::Any>>,
    mut request: Request,
    next: Next,
) -> Result<Response, Response> {
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
            match extract_matrix_auth(request.headers(), &session_service).await {
                Ok(auth) => auth,
                Err(e) => {
                    tracing::warn!("Authentication failed: {}", e);
                    return Err(MatrixError::Unauthorized.into_response());
                },
            }
        } else {
            // Missing proper token format - return MissingAuthorization error
            return Err(MatrixError::Unauthorized.into_response());
        }
    } else if let Some(x_matrix_header) = x_matrix_header {
        // Extract request body for signature verification WITHOUT borrowing conflicts
        let (parts, body) = request.into_parts();
        let body_bytes = to_bytes(body, usize::MAX).await
            .map_err(|_| MatrixError::Unauthorized.into_response())?;

        // Reconstruct request properly
        let new_request = Request::from_parts(parts, Body::from(body_bytes.clone()));

        // Pass the correct reference to validation function
        let result = validate_server_signature(&x_matrix_header, &new_request, &body_bytes, &session_service).await?;

        // Update the request variable correctly
        request = new_request;

        result
    } else {
        // No authorization header - return MissingToken error
        return Err(MatrixError::Unauthorized.into_response());
    };

    request.extensions_mut().insert(matrix_auth);
    Ok(next.run(request).await)
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
    request: &Request,
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
            warn!("X-Matrix destination mismatch: got '{}', expected '{}'",
                  destination, homeserver_name);
            return Err(MatrixError::Unauthorized.into_response());
        }
        info!("X-Matrix destination validated: {}", destination);
    } else {
        debug!("X-Matrix request without destination parameter (backward compatibility)");
    }

    // Use session service for server key validation per Matrix specification
    debug!("Validating X-Matrix auth for server: {}", origin);
    
    // Extract request details for signature verification
    let request_method = request.method().as_str();
    let request_uri = request.uri().path_and_query().map(|pq| pq.as_str()).unwrap_or("/");

    // Use the actual request body bytes for signature verification

    // Log authentication attempt for audit trail
    info!("X-Matrix auth attempt: server={}, key={}, method={}, uri={}", 
          origin, key_id, request_method, request_uri);

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
    debug!("Processing X-Matrix federation request - origin: {}, homeserver: {}, body_type: {}",
           origin, homeserver_name, if body_json.is_null() { "empty" } else { "json" });

    // Matrix federation signature validation already passed via validate_server_signature() above
    info!("X-Matrix federation auth successful for server: {}", origin);



    // Create the authenticated server result after validation
    let server_auth = MatrixServerAuth {
        server_name: origin.to_string(),
        key_id: key_id.to_string(),
        signature: signature.to_string(),
        expires_at: None,
    };

    info!("Server signature validated for origin: {} key_id: {} method: {}",
          origin, key_id, request_method);

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
        let cert_data = cert_header.to_str()
            .map_err(|_| "Invalid certificate header")?;

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
                                cert_ip_bytes[0], cert_ip_bytes[1],
                                cert_ip_bytes[2], cert_ip_bytes[3]
                            ]);
                            if ipv4 == cert_ipv4 {
                                return Ok(true);
                            }
                        }
                        (IpAddr::V6(ipv6), 16) => {
                            let mut bytes = [0u8; 16];
                            bytes.copy_from_slice(cert_ip_bytes);
                            let cert_ipv6 = std::net::Ipv6Addr::from(bytes);
                            if ipv6 == cert_ipv6 {
                                return Ok(true);
                            }
                        }
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
                if let GeneralName::DNSName(dns_name) = name {
                    if matches_hostname(dns_name, hostname) {
                        return Ok(true);
                    }
                }
            }
        }
    }

    // Fallback to Common Name in Subject (legacy)
    let subject = &cert.subject();
    for rdn in subject.iter() {
        for attr in rdn.iter() {
            // Common Name OID: 2.5.4.3
            if attr.attr_type().to_id_string() == "2.5.4.3" {
                if let Ok(cn) = attr.attr_value().as_str() {
                    if matches_hostname(cn, hostname) {
                        return Ok(true);
                    }
                }
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
    if cert_name.starts_with("*.") {
        let cert_domain = &cert_name[2..];
        let host_parts: Vec<&str> = hostname.split('.').skip(1).collect();
        if !host_parts.is_empty() {
            let host_domain = host_parts.join(".");
            return cert_domain == host_domain;
        }
    }

    false
}


