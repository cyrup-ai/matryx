//! Module contains intentional library code not yet fully integrated
#![allow(dead_code)]

//! Matrix Federation Server Discovery Orchestrator
//!
//! Implements the complete Matrix server discovery process according to the
//! Matrix Server-Server API specification. This orchestrator coordinates
//! all resolution methods in the correct priority order.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;
use tracing::{error, info};

use crate::federation::dns_resolver::{DnsResolutionError, MatrixDnsResolver, ResolvedServer};

/// Server discovery errors
#[derive(Debug, Error)]
pub enum ServerDiscoveryError {
    #[error("DNS resolution failed: {0}")]
    DnsError(#[from] DnsResolutionError),

    #[error("Invalid server name: {0}")]
    InvalidServerName(String),

    #[error("No valid server found for: {0}")]
    NoServerFound(String),

    #[error("All resolution methods failed for: {0}")]
    AllMethodsFailed(String),
}

pub type DiscoveryResult<T> = Result<T, ServerDiscoveryError>;

/// Certificate validation requirements based on resolution method
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CertificateValidation {
    /// Validate against the original server name
    ServerName(String),
    /// Validate against the resolved hostname
    Hostname(String),
    /// Skip hostname validation for IP literals (still validate certificate chain)
    IpLiteral,
}

/// Federation connection information
#[derive(Debug, Clone)]
pub struct FederationConnection {
    /// Socket address to connect to
    pub socket_addr: SocketAddr,
    /// Base URL for HTTP requests (http://ip:port or https://ip:port)
    pub base_url: String,
    /// Host header value for HTTP requests
    pub host_header: String,
    /// Certificate validation requirements
    pub cert_validation: CertificateValidation,
    /// Resolution method used
    pub resolution_method: String,
}

/// Matrix Federation Server Discovery Orchestrator
///
/// Coordinates the complete Matrix server discovery process following
/// the Matrix Server-Server API specification priority order:
/// 1. IP literal handling
/// 2. Explicit port handling  
/// 3. Well-known delegation lookup
/// 4. SRV record resolution (_matrix-fed._tcp per Matrix v1.8, fallback to deprecated _matrix._tcp)
/// 5. Fallback to hostname:8448
pub struct ServerDiscoveryOrchestrator {
    dns_resolver: Arc<MatrixDnsResolver>,
    #[allow(dead_code)]
    timeout: Duration,
}

impl ServerDiscoveryOrchestrator {
    /// Create a new server discovery orchestrator
    pub fn new(dns_resolver: Arc<MatrixDnsResolver>) -> Self {
        Self { dns_resolver, timeout: Duration::from_secs(30) }
    }

    /// Create orchestrator with custom timeout
    pub fn with_timeout(dns_resolver: Arc<MatrixDnsResolver>, timeout: Duration) -> Self {
        Self { dns_resolver, timeout }
    }

    /// Discover Matrix server connection information
    ///
    /// Implements the complete Matrix server discovery process in the correct
    /// priority order as specified by the Matrix Server-Server API.
    ///
    /// # Arguments
    /// * `server_name` - The Matrix server name to resolve
    ///
    /// # Returns
    /// * `FederationConnection` - Complete connection information for federation
    pub async fn discover_server(
        &self,
        server_name: &str,
    ) -> DiscoveryResult<FederationConnection> {
        info!("Starting Matrix server discovery for: {}", server_name);

        // Use the existing DNS resolver which already implements the full discovery chain
        let resolved = self.dns_resolver.resolve_server(server_name).await?;

        // Convert ResolvedServer to FederationConnection
        let connection = self.create_federation_connection(&resolved, server_name);

        info!(
            "Server discovery completed for {}: {} via {}",
            server_name, connection.socket_addr, connection.resolution_method
        );

        Ok(connection)
    }

    /// Create federation connection from resolved server information
    fn create_federation_connection(
        &self,
        resolved: &ResolvedServer,
        original_server_name: &str,
    ) -> FederationConnection {
        let socket_addr = SocketAddr::new(resolved.ip_address, resolved.port);
        let base_url = self.dns_resolver.get_base_url(resolved);
        let host_header = self.dns_resolver.get_host_header(resolved);

        // Determine certificate validation requirements based on resolution method
        let cert_validation = match &resolved.resolution_method {
            crate::federation::dns_resolver::ResolutionMethod::IpLiteral => {
                CertificateValidation::IpLiteral
            },
            crate::federation::dns_resolver::ResolutionMethod::ExplicitPort => {
                CertificateValidation::ServerName(original_server_name.to_string())
            },
            crate::federation::dns_resolver::ResolutionMethod::WellKnownDelegation => {
                CertificateValidation::Hostname(resolved.tls_hostname.clone())
            },
            crate::federation::dns_resolver::ResolutionMethod::SrvMatrixFed
            | crate::federation::dns_resolver::ResolutionMethod::SrvMatrixLegacy => {
                CertificateValidation::Hostname(resolved.tls_hostname.clone())
            },
            crate::federation::dns_resolver::ResolutionMethod::FallbackPort8448 => {
                CertificateValidation::ServerName(original_server_name.to_string())
            },
        };

        FederationConnection {
            socket_addr,
            base_url,
            host_header,
            cert_validation,
            resolution_method: format!("{:?}", resolved.resolution_method),
        }
    }

    /// Get the underlying DNS resolver
    pub fn dns_resolver(&self) -> &Arc<MatrixDnsResolver> {
        &self.dns_resolver
    }

    /// Validate server name format
    pub fn validate_server_name(&self, server_name: &str) -> DiscoveryResult<()> {
        if server_name.is_empty() {
            return Err(ServerDiscoveryError::InvalidServerName("Empty server name".to_string()));
        }

        if server_name.contains("://") {
            return Err(ServerDiscoveryError::InvalidServerName(
                "Server name should not contain protocol scheme".to_string(),
            ));
        }

        if server_name.starts_with('.') || server_name.ends_with('.') {
            return Err(ServerDiscoveryError::InvalidServerName(
                "Server name cannot start or end with dot".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn create_test_orchestrator() -> ServerDiscoveryOrchestrator {
        // Create proper test setup with mock DNS resolver
        let http_client = Arc::new(reqwest::Client::new());
        let well_known_client =
            Arc::new(crate::federation::well_known_client::WellKnownClient::new(http_client));
        let dns_resolver = Arc::new(
            crate::federation::dns_resolver::MatrixDnsResolver::new(well_known_client)
                .expect("Failed to create DNS resolver"),
        );
        ServerDiscoveryOrchestrator::new(dns_resolver)
    }

    #[test]
    fn test_validate_server_name() {
        let orchestrator = create_test_orchestrator();

        // Valid server names
        assert!(orchestrator.validate_server_name("example.com").is_ok());
        assert!(orchestrator.validate_server_name("matrix.example.com:8448").is_ok());
        assert!(orchestrator.validate_server_name("192.168.1.1").is_ok());
        assert!(orchestrator.validate_server_name("[::1]:8448").is_ok());

        // Invalid server names
        assert!(orchestrator.validate_server_name("").is_err());
        assert!(orchestrator.validate_server_name("https://example.com").is_err());
        assert!(orchestrator.validate_server_name(".example.com").is_err());
        assert!(orchestrator.validate_server_name("example.com.").is_err());
    }
}
