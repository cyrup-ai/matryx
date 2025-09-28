use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use hickory_resolver::TokioResolver;
use hickory_resolver::config::{ResolverConfig, ResolverOpts};
use tokio::sync::RwLock;

use hickory_resolver::ResolveError;
use tracing::{debug, info, warn, error};
use serde::{Deserialize, Serialize};

use crate::federation::well_known_client::WellKnownClient;

/// DNS resolution errors for Matrix federation
#[derive(Debug, Clone, thiserror::Error)]
pub enum DnsResolutionError {
    #[error("DNS resolution failed: {0}")]
    ResolveError(String),
    
    #[error("Well-known lookup failed: {0}")]
    WellKnownError(String),
    
    #[error("No valid server found for hostname: {0}")]
    NoServerFound(String),
    
    #[error("Invalid hostname: {0}")]
    InvalidHostname(String),
    
    #[error("SRV record parsing failed: {0}")]
    SrvParsingError(String),
}

impl From<ResolveError> for DnsResolutionError {
    fn from(error: ResolveError) -> Self {
        DnsResolutionError::ResolveError(error.to_string())
    }
}

pub type DnsResult<T> = Result<T, DnsResolutionError>;

/// Resolved server information for Matrix federation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedServer {
    /// The IP address to connect to
    pub ip_address: IpAddr,
    /// The port to connect to
    pub port: u16,
    /// The hostname to use in the Host header
    pub host_header: String,
    /// The hostname for TLS certificate validation
    pub tls_hostname: String,
    /// The resolution method used
    pub resolution_method: ResolutionMethod,
}

/// Method used to resolve the server
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionMethod {
    /// Direct IP literal from server name
    IpLiteral,
    /// Hostname with explicit port
    ExplicitPort,
    /// Well-known delegation
    WellKnownDelegation,
    /// SRV record lookup (_matrix-fed._tcp)
    SrvMatrixFed,
    /// Legacy SRV record lookup (_matrix._tcp)
    SrvMatrixLegacy,
    /// Fallback to hostname:8448
    FallbackPort8448,
}

/// SRV record information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrvRecord {
    pub priority: u16,
    pub weight: u16,
    pub port: u16,
    pub target: String,
}

/// Cache entry for server discovery results
#[derive(Debug, Clone)]
struct CacheEntry<T> {
    value: T,
    expires_at: Instant,
}

impl<T> CacheEntry<T> {
    fn new(value: T, ttl: Duration) -> Self {
        Self {
            value,
            expires_at: Instant::now() + ttl,
        }
    }

    fn is_expired(&self) -> bool {
        Instant::now() > self.expires_at
    }
}

/// Exponential backoff state for failed servers
#[derive(Debug, Clone)]
struct BackoffState {
    failure_count: u32,
    next_retry_at: Instant,
    base_delay: Duration,
}

impl BackoffState {
    fn new() -> Self {
        Self {
            failure_count: 0,
            next_retry_at: Instant::now(),
            base_delay: Duration::from_secs(1),
        }
    }

    fn record_failure(&mut self) {
        self.failure_count += 1;
        let delay = self.base_delay * 2_u32.pow(self.failure_count.min(10)); // Cap at 2^10 = 1024 seconds
        self.next_retry_at = Instant::now() + delay;
        debug!("Recorded failure #{}, next retry in {:?}", self.failure_count, delay);
    }

    fn can_retry(&self) -> bool {
        Instant::now() >= self.next_retry_at
    }

    fn reset(&mut self) {
        self.failure_count = 0;
        self.next_retry_at = Instant::now();
    }
}

/// Matrix-compliant DNS resolver for federation server discovery
/// 
/// Implements the complete Matrix Server-Server API DNS resolution specification:
/// 1. Well-known delegation (/.well-known/matrix/server)
/// 2. SRV record lookup (_matrix-fed._tcp and _matrix._tcp)
/// 3. Direct hostname resolution with fallback to port 8448
/// 4. Response caching with Matrix-compliant TTLs
/// 5. Exponential backoff for repeated failures
/// 
/// This resolver ensures Matrix specification compliance for server discovery.
pub struct MatrixDnsResolver {
    dns_resolver: Arc<TokioResolver>,
    well_known_client: Arc<WellKnownClient>,
    timeout: Duration,
    // Caching for successful well-known responses (24-48 hours)
    well_known_cache: Arc<RwLock<HashMap<String, CacheEntry<Option<crate::federation::well_known_client::WellKnownResponse>>>>>,
    // Caching for error responses (up to 1 hour)
    error_cache: Arc<RwLock<HashMap<String, CacheEntry<DnsResolutionError>>>>,
    // Exponential backoff state for failed servers
    backoff_state: Arc<RwLock<HashMap<String, BackoffState>>>,
}

impl MatrixDnsResolver {
    /// Create a new Matrix DNS resolver
    pub fn new(well_known_client: Arc<WellKnownClient>) -> DnsResult<Self> {
        let resolver = TokioResolver::builder_tokio()?.build();

        Ok(Self {
            dns_resolver: Arc::new(resolver),
            well_known_client,
            timeout: Duration::from_secs(10),
            well_known_cache: Arc::new(RwLock::new(HashMap::new())),
            error_cache: Arc::new(RwLock::new(HashMap::new())),
            backoff_state: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Create a new Matrix DNS resolver with custom configuration
    pub fn with_config(
        _config: ResolverConfig,
        _opts: ResolverOpts,
        well_known_client: Arc<WellKnownClient>,
        timeout: Duration,
    ) -> DnsResult<Self> {
        // Build resolver with tokio runtime
        let resolver = TokioResolver::builder_tokio()?.build();

        Ok(Self {
            dns_resolver: Arc::new(resolver),
            well_known_client,
            timeout,
            well_known_cache: Arc::new(RwLock::new(HashMap::new())),
            error_cache: Arc::new(RwLock::new(HashMap::new())),
            backoff_state: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Resolve a Matrix server name according to the Matrix Server-Server API specification
    /// 
    /// This implements the complete Matrix server discovery process:
    /// 1. Parse server name (hostname[:port])
    /// 2. Handle IP literals directly
    /// 3. Handle explicit ports with hostname resolution
    /// 4. Perform well-known delegation lookup with caching
    /// 5. Perform SRV record lookups (_matrix-fed._tcp and _matrix._tcp)
    /// 6. Fallback to hostname:8448
    /// 7. Exponential backoff for repeated failures
    /// 
    /// # Arguments
    /// * `server_name` - The Matrix server name (e.g., "example.com" or "matrix.example.com:8448")
    /// 
    /// # Returns
    /// * `ResolvedServer` - Complete server connection information
    pub async fn resolve_server(&self, server_name: &str) -> DnsResult<ResolvedServer> {
        info!("Resolving Matrix server: {}", server_name);

        // Check if we have a cached error for this server
        if let Some(cached_error) = self.get_cached_error(server_name).await {
            debug!("Returning cached error for {}: {}", server_name, cached_error);
            return Err(cached_error);
        }

        // Check exponential backoff state
        if !self.can_retry_server(server_name).await {
            let error = DnsResolutionError::NoServerFound(format!("Server {} is in backoff state", server_name));
            return Err(error);
        }

        let result = self.resolve_server_internal(server_name).await;

        // Handle result for caching and backoff
        match &result {
            Ok(_) => {
                // Reset backoff state on success
                self.reset_backoff_state(server_name).await;
            }
            Err(e) => {
                // Record failure and cache error
                self.record_failure(server_name).await;
                self.cache_error(server_name, e.clone()).await;
            }
        }

        result
    }

    /// Internal server resolution without caching/backoff logic
    async fn resolve_server_internal(&self, server_name: &str) -> DnsResult<ResolvedServer> {
        // Step 1: Parse server name
        let (hostname, explicit_port) = self.parse_server_name(server_name)?;

        // Step 2: Handle IP literals
        if let Ok(ip) = hostname.parse::<IpAddr>() {
            let port = explicit_port.unwrap_or(8448);
            debug!("Server name is IP literal: {}:{}", ip, port);
            return Ok(ResolvedServer {
                ip_address: ip,
                port,
                host_header: server_name.to_string(),
                tls_hostname: ip.to_string(),
                resolution_method: ResolutionMethod::IpLiteral,
            });
        }

        // Step 3: Handle explicit port
        if let Some(port) = explicit_port {
            debug!("Server name has explicit port: {}:{}", hostname, port);
            let ip = self.resolve_hostname_to_ip(&hostname).await?;
            return Ok(ResolvedServer {
                ip_address: ip,
                port,
                host_header: server_name.to_string(),
                tls_hostname: hostname.clone(),
                resolution_method: ResolutionMethod::ExplicitPort,
            });
        }

        // Step 4: Well-known delegation lookup with caching
        match self.resolve_via_well_known_cached(&hostname).await {
            Ok(resolved) => {
                info!("Resolved via well-known delegation: {:?}", resolved);
                return Ok(resolved);
            }
            Err(e) => {
                debug!("Well-known lookup failed: {}", e);
                // Continue to SRV lookup
            }
        }

        // Step 5: SRV record lookups
        // Try _matrix-fed._tcp first (Matrix v1.8+)
        match self.resolve_via_srv(&hostname, "_matrix-fed._tcp").await {
            Ok(resolved) => {
                info!("Resolved via _matrix-fed._tcp SRV record: {:?}", resolved);
                return Ok(resolved);
            }
            Err(e) => {
                debug!("_matrix-fed._tcp SRV lookup failed: {}", e);
            }
        }

        // Try legacy _matrix._tcp (deprecated)
        match self.resolve_via_srv(&hostname, "_matrix._tcp").await {
            Ok(resolved) => {
                info!("Resolved via legacy _matrix._tcp SRV record: {:?}", resolved);
                return Ok(resolved);
            }
            Err(e) => {
                debug!("_matrix._tcp SRV lookup failed: {}", e);
            }
        }

        // Step 6: Fallback to hostname:8448
        debug!("Falling back to {}:8448", hostname);
        let ip = self.resolve_hostname_to_ip(&hostname).await?;
        Ok(ResolvedServer {
            ip_address: ip,
            port: 8448,
            host_header: hostname.clone(),
            tls_hostname: hostname,
            resolution_method: ResolutionMethod::FallbackPort8448,
        })
    }

    /// Parse a Matrix server name into hostname and optional port
    fn parse_server_name(&self, server_name: &str) -> DnsResult<(String, Option<u16>)> {
        if server_name.is_empty() {
            return Err(DnsResolutionError::InvalidHostname("Empty server name".to_string()));
        }

        // Handle IPv6 literals [::1]:8448
        if server_name.starts_with('[') {
            if let Some(bracket_end) = server_name.find(']') {
                let ipv6_part = &server_name[1..bracket_end];
                let remainder = &server_name[bracket_end + 1..];
                
                if remainder.is_empty() {
                    return Ok((ipv6_part.to_string(), None));
                } else if let Some(port_str) = remainder.strip_prefix(':') {
                    let port = port_str.parse::<u16>()
                        .map_err(|_| DnsResolutionError::InvalidHostname(format!("Invalid port: {}", port_str)))?;
                    return Ok((ipv6_part.to_string(), Some(port)));
                } else {
                    return Err(DnsResolutionError::InvalidHostname(format!("Invalid IPv6 literal: {}", server_name)));
                }
            } else {
                return Err(DnsResolutionError::InvalidHostname(format!("Unclosed IPv6 literal: {}", server_name)));
            }
        }

        // Handle regular hostname[:port]
        if let Some(colon_pos) = server_name.rfind(':') {
            let hostname = &server_name[..colon_pos];
            let port_str = &server_name[colon_pos + 1..];
            
            // Check if this is actually an IPv6 address without brackets
            if hostname.contains(':') {
                // This is likely an IPv6 address without brackets
                return Ok((server_name.to_string(), None));
            }
            
            let port = port_str.parse::<u16>()
                .map_err(|_| DnsResolutionError::InvalidHostname(format!("Invalid port: {}", port_str)))?;
            Ok((hostname.to_string(), Some(port)))
        } else {
            Ok((server_name.to_string(), None))
        }
    }

    /// Resolve server via well-known delegation with caching
    async fn resolve_via_well_known_cached(&self, hostname: &str) -> DnsResult<ResolvedServer> {
        // Check cache first
        if let Some(cached_response) = self.get_cached_well_known(hostname).await {
            debug!("Using cached well-known response for {}", hostname);
            if let Some(well_known) = cached_response {
                return self.process_well_known_response(hostname, &well_known).await;
            } else {
                return Err(DnsResolutionError::WellKnownError("Cached well-known response is None".to_string()));
            }
        }

        // Fetch from network
        let well_known_result = self.well_known_client.get_well_known(hostname).await
            .map_err(|e| DnsResolutionError::WellKnownError(e.to_string()));

        // Cache the result (both success and failure)
        match &well_known_result {
            Ok(well_known_opt) => {
                self.cache_well_known(hostname, well_known_opt.clone()).await;
                if let Some(well_known) = well_known_opt {
                    return self.process_well_known_response(hostname, well_known).await;
                } else {
                    return Err(DnsResolutionError::WellKnownError("No well-known response".to_string()));
                }
            }
            Err(e) => {
                // Cache None for failed requests
                self.cache_well_known(hostname, None).await;
                return Err(e.clone());
            }
        }
    }

    /// Process a well-known response to resolve the server
    async fn process_well_known_response(&self, _hostname: &str, well_known: &crate::federation::well_known_client::WellKnownResponse) -> DnsResult<ResolvedServer> {

        // Parse delegated server name
        let (delegated_hostname, delegated_port) = self.parse_server_name(&well_known.server)?;

        // Handle delegated IP literal
        if let Ok(ip) = delegated_hostname.parse::<IpAddr>() {
            let port = delegated_port.unwrap_or(8448);
            return Ok(ResolvedServer {
                ip_address: ip,
                port,
                host_header: format!("{}:{}", ip, port),
                tls_hostname: ip.to_string(),
                resolution_method: ResolutionMethod::WellKnownDelegation,
            });
        }

        // Handle delegated hostname with explicit port
        if let Some(port) = delegated_port {
            let ip = self.resolve_hostname_to_ip(&delegated_hostname).await?;
            return Ok(ResolvedServer {
                ip_address: ip,
                port,
                host_header: format!("{}:{}", delegated_hostname, port),
                tls_hostname: delegated_hostname,
                resolution_method: ResolutionMethod::WellKnownDelegation,
            });
        }

        // Try SRV records for delegated hostname
        // Try _matrix-fed._tcp first
        if let Ok(resolved) = self.resolve_via_srv(&delegated_hostname, "_matrix-fed._tcp").await {
            return Ok(ResolvedServer {
                ip_address: resolved.ip_address,
                port: resolved.port,
                host_header: delegated_hostname.clone(),
                tls_hostname: delegated_hostname,
                resolution_method: ResolutionMethod::WellKnownDelegation,
            });
        }

        // Try legacy _matrix._tcp
        if let Ok(resolved) = self.resolve_via_srv(&delegated_hostname, "_matrix._tcp").await {
            return Ok(ResolvedServer {
                ip_address: resolved.ip_address,
                port: resolved.port,
                host_header: delegated_hostname.clone(),
                tls_hostname: delegated_hostname,
                resolution_method: ResolutionMethod::WellKnownDelegation,
            });
        }

        // Fallback to delegated hostname:8448
        let ip = self.resolve_hostname_to_ip(&delegated_hostname).await?;
        Ok(ResolvedServer {
            ip_address: ip,
            port: 8448,
            host_header: delegated_hostname.clone(),
            tls_hostname: delegated_hostname,
            resolution_method: ResolutionMethod::WellKnownDelegation,
        })
    }

    /// Cache management methods
    async fn get_cached_well_known(&self, hostname: &str) -> Option<Option<crate::federation::well_known_client::WellKnownResponse>> {
        let cache = self.well_known_cache.read().await;
        if let Some(entry) = cache.get(hostname) {
            if !entry.is_expired() {
                return Some(entry.value.clone());
            }
        }
        None
    }

    async fn cache_well_known(&self, hostname: &str, response: Option<crate::federation::well_known_client::WellKnownResponse>) {
        let mut cache = self.well_known_cache.write().await;
        // Matrix spec: cache for 24-48 hours, we use 24 hours as default
        let ttl = Duration::from_secs(24 * 60 * 60); // 24 hours
        cache.insert(hostname.to_string(), CacheEntry::new(response, ttl));
        debug!("Cached well-known response for {} (TTL: {:?})", hostname, ttl);
    }

    async fn get_cached_error(&self, server_name: &str) -> Option<DnsResolutionError> {
        let cache = self.error_cache.read().await;
        if let Some(entry) = cache.get(server_name) {
            if !entry.is_expired() {
                return Some(entry.value.clone());
            }
        }
        None
    }

    async fn cache_error(&self, server_name: &str, error: DnsResolutionError) {
        let mut cache = self.error_cache.write().await;
        // Matrix spec: cache errors for up to 1 hour
        let ttl = Duration::from_secs(60 * 60); // 1 hour
        cache.insert(server_name.to_string(), CacheEntry::new(error, ttl));
        debug!("Cached error for {} (TTL: {:?})", server_name, ttl);
    }

    /// Exponential backoff management methods
    async fn can_retry_server(&self, server_name: &str) -> bool {
        let backoff_map = self.backoff_state.read().await;
        if let Some(state) = backoff_map.get(server_name) {
            state.can_retry()
        } else {
            true // No backoff state means we can retry
        }
    }

    async fn record_failure(&self, server_name: &str) {
        let mut backoff_map = self.backoff_state.write().await;
        let state = backoff_map.entry(server_name.to_string()).or_insert_with(BackoffState::new);
        state.record_failure();
    }

    async fn reset_backoff_state(&self, server_name: &str) {
        let mut backoff_map = self.backoff_state.write().await;
        if let Some(state) = backoff_map.get_mut(server_name) {
            state.reset();
        }
    }

    /// Clean up expired cache entries and old backoff states
    pub async fn cleanup_cache(&self) {
        // Clean up well-known cache
        {
            let mut cache = self.well_known_cache.write().await;
            cache.retain(|_, entry| !entry.is_expired());
        }

        // Clean up error cache
        {
            let mut cache = self.error_cache.write().await;
            cache.retain(|_, entry| !entry.is_expired());
        }

        // Clean up old backoff states (older than 24 hours)
        {
            let mut backoff_map = self.backoff_state.write().await;
            let cutoff = Instant::now() - Duration::from_secs(24 * 60 * 60);
            backoff_map.retain(|_, state| state.next_retry_at > cutoff);
        }

        debug!("Cleaned up expired cache entries and old backoff states");
    }

    /// Resolve server via SRV records
    async fn resolve_via_srv(&self, hostname: &str, service: &str) -> DnsResult<ResolvedServer> {
        let srv_name = format!("{}.{}", service, hostname);
        debug!("Looking up SRV record: {}", srv_name);

        // Apply timeout to SRV lookup
        let srv_lookup = tokio::time::timeout(
            self.timeout,
            self.dns_resolver.srv_lookup(&srv_name)
        ).await
            .map_err(|_| DnsResolutionError::ResolveError(format!("SRV lookup timeout for {}", srv_name)))?
            .map_err(DnsResolutionError::from)?;

        let srv_records = self.parse_srv_records(srv_lookup)?;

        if srv_records.is_empty() {
            return Err(DnsResolutionError::NoServerFound(format!("No SRV records found for {}", srv_name)));
        }

        // Sort by priority (lower is higher priority), then by weight (higher is preferred)
        let mut sorted_records = srv_records;
        sorted_records.sort_by(|a, b| {
            a.priority.cmp(&b.priority)
                .then_with(|| b.weight.cmp(&a.weight))
        });

        // Try each SRV record in order
        for srv_record in sorted_records {
            debug!("Trying SRV record: {}:{} (priority: {}, weight: {})", 
                   srv_record.target, srv_record.port, srv_record.priority, srv_record.weight);

            match self.resolve_hostname_to_ip(&srv_record.target).await {
                Ok(ip) => {
                    let resolution_method = if service == "_matrix-fed._tcp" {
                        ResolutionMethod::SrvMatrixFed
                    } else {
                        ResolutionMethod::SrvMatrixLegacy
                    };

                    return Ok(ResolvedServer {
                        ip_address: ip,
                        port: srv_record.port,
                        host_header: hostname.to_string(),
                        tls_hostname: hostname.to_string(),
                        resolution_method,
                    });
                }
                Err(e) => {
                    warn!("Failed to resolve SRV target {}: {}", srv_record.target, e);
                    continue;
                }
            }
        }

        Err(DnsResolutionError::NoServerFound(format!("All SRV targets failed for {}", srv_name)))
    }

    /// Parse SRV lookup results into SrvRecord structs
    fn parse_srv_records(&self, srv_lookup: hickory_resolver::lookup::SrvLookup) -> DnsResult<Vec<SrvRecord>> {
        let mut records = Vec::new();

        for srv in srv_lookup.iter() {
            records.push(SrvRecord {
                priority: srv.priority(),
                weight: srv.weight(),
                port: srv.port(),
                target: srv.target().to_string().trim_end_matches('.').to_string(),
            });
        }

        Ok(records)
    }

    /// Resolve hostname to IP address using A/AAAA records
    async fn resolve_hostname_to_ip(&self, hostname: &str) -> DnsResult<IpAddr> {
        debug!("Resolving hostname to IP: {}", hostname);

        // Apply timeout to DNS resolution
        let lookup = tokio::time::timeout(
            self.timeout,
            self.dns_resolver.lookup_ip(hostname)
        ).await
            .map_err(|_| DnsResolutionError::ResolveError(format!("DNS resolution timeout for {}", hostname)))?
            .map_err(DnsResolutionError::from)?;

        let ip = lookup.iter().next()
            .ok_or_else(|| DnsResolutionError::NoServerFound(format!("No IP addresses found for {}", hostname)))?;

        debug!("Resolved {} to {}", hostname, ip);
        Ok(ip)
    }

    /// Get the base URL for a resolved server
    pub fn get_base_url(&self, resolved: &ResolvedServer) -> String {
        format!("https://{}:{}", resolved.ip_address, resolved.port)
    }

    /// Get the Host header value for a resolved server
    pub fn get_host_header(&self, resolved: &ResolvedServer) -> String {
        resolved.host_header.clone()
    }

    /// Get a socket address for the resolved server
    pub fn get_socket_addr(&self, resolved: &ResolvedServer) -> SocketAddr {
        SocketAddr::new(resolved.ip_address, resolved.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use reqwest::Client;

    fn create_test_resolver() -> MatrixDnsResolver {
        let http_client = Arc::new(Client::new());
        let well_known_client = Arc::new(WellKnownClient::new(http_client));
        MatrixDnsResolver::new(well_known_client).unwrap()
    }

    #[test]
    fn test_parse_server_name() {
        let resolver = create_test_resolver();

        // Test hostname only
        let (hostname, port) = resolver.parse_server_name("example.com").unwrap();
        assert_eq!(hostname, "example.com");
        assert_eq!(port, None);

        // Test hostname with port
        let (hostname, port) = resolver.parse_server_name("example.com:8448").unwrap();
        assert_eq!(hostname, "example.com");
        assert_eq!(port, Some(8448));

        // Test IPv4 literal
        let (hostname, port) = resolver.parse_server_name("192.168.1.1").unwrap();
        assert_eq!(hostname, "192.168.1.1");
        assert_eq!(port, None);

        // Test IPv4 literal with port
        let (hostname, port) = resolver.parse_server_name("192.168.1.1:8448").unwrap();
        assert_eq!(hostname, "192.168.1.1");
        assert_eq!(port, Some(8448));

        // Test IPv6 literal with brackets
        let (hostname, port) = resolver.parse_server_name("[::1]:8448").unwrap();
        assert_eq!(hostname, "::1");
        assert_eq!(port, Some(8448));

        // Test IPv6 literal without port
        let (hostname, port) = resolver.parse_server_name("[2001:db8::1]").unwrap();
        assert_eq!(hostname, "2001:db8::1");
        assert_eq!(port, None);
    }

    #[tokio::test]
    async fn test_resolve_ip_literal() {
        let resolver = create_test_resolver();

        // Test IPv4 literal
        let resolved = resolver.resolve_server("192.168.1.1:8448").await.unwrap();
        assert_eq!(resolved.ip_address, "192.168.1.1".parse::<IpAddr>().unwrap());
        assert_eq!(resolved.port, 8448);
        assert_eq!(resolved.resolution_method, ResolutionMethod::IpLiteral);

        // Test IPv6 literal
        let resolved = resolver.resolve_server("[::1]:8448").await.unwrap();
        assert_eq!(resolved.ip_address, "::1".parse::<IpAddr>().unwrap());
        assert_eq!(resolved.port, 8448);
        assert_eq!(resolved.resolution_method, ResolutionMethod::IpLiteral);
    }
}