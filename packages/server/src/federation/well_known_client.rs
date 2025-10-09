//! Module contains intentional library code not yet fully integrated
#![allow(dead_code)]

use moka::future::Cache;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tracing::{debug, error, info, warn};

/// Well-known matrix server response
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WellKnownResponse {
    #[serde(rename = "m.server")]
    pub server: String,
}

/// Cached well-known response with metadata
#[derive(Debug, Clone)]
pub struct CachedWellKnown {
    pub response: Option<WellKnownResponse>,
    pub cached_at: SystemTime,
    pub expires_at: SystemTime,
    pub is_error: bool,
}

/// Well-known client errors
#[derive(Debug, thiserror::Error)]
pub enum WellKnownError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("JSON parsing failed: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Too many redirects")]
    TooManyRedirects,

    #[error("Cache error: {0}")]
    CacheError(String),
}

pub type WellKnownResult<T> = Result<T, WellKnownError>;

/// Well-known client with Matrix-compliant caching
pub struct WellKnownClient {
    http_client: Arc<Client>,
    cache: Cache<String, CachedWellKnown>,
    max_redirects: u32,
    use_https: bool,
}

impl WellKnownClient {
    pub fn new(http_client: Arc<Client>, use_https: bool) -> Self {
        Self {
            http_client,
            cache: Cache::builder()
                .max_capacity(1000)
                .time_to_live(Duration::from_secs(3600))
                .build(),
            max_redirects: 5,
            use_https,
        }
    }

    /// Fetch well-known server information with caching
    pub async fn get_well_known(
        &self,
        hostname: &str,
    ) -> WellKnownResult<Option<WellKnownResponse>> {
        // Check cache first
        if let Some(cached) = self.cache.get(hostname).await {
            if SystemTime::now() < cached.expires_at {
                debug!("Well-known cache hit for {}", hostname);
                return Ok(cached.response);
            }
            // Cache expired, remove entry
            self.cache.remove(hostname).await;
        }

        debug!("Fetching well-known for {} from network", hostname);

        // Fetch from network
        match self.fetch_well_known_from_network(hostname).await {
            Ok((response, ttl)) => {
                let cached = CachedWellKnown {
                    response: Some(response.clone()),
                    cached_at: SystemTime::now(),
                    expires_at: SystemTime::now() + ttl,
                    is_error: false,
                };

                self.cache.insert(hostname.to_string(), cached).await;
                info!(
                    "Successfully fetched and cached well-known for {} (TTL: {:?})",
                    hostname, ttl
                );
                Ok(Some(response))
            },
            Err(e) => {
                // Cache errors for 1 hour
                let error_ttl = Duration::from_secs(60 * 60);
                let cached = CachedWellKnown {
                    response: None,
                    cached_at: SystemTime::now(),
                    expires_at: SystemTime::now() + error_ttl,
                    is_error: true,
                };

                self.cache.insert(hostname.to_string(), cached).await;
                warn!(
                    "Failed to fetch well-known for {}, cached error for 1 hour: {}",
                    hostname, e
                );
                Ok(None) // Return None for errors to allow fallback discovery
            },
        }
    }

    /// Fetch well-known from network with redirect handling
    async fn fetch_well_known_from_network(
        &self,
        hostname: &str,
    ) -> WellKnownResult<(WellKnownResponse, Duration)> {
        let protocol = if self.use_https { "https" } else { "http" };
        let url = format!("{}://{}/.well-known/matrix/server", protocol, hostname);
        let mut current_url = url;
        let mut redirect_count = 0;

        loop {
            debug!("Requesting well-known from: {}", current_url);

            let response = self
                .http_client
                .get(&current_url)
                .header("User-Agent", "matryx-server/1.0")
                // NOTE: .well-known discovery requests are exempt from X-Matrix signing
                // per Matrix Specification RFC 8615 "Well-Known URI" method and Matrix
                // Server-Server API documentation. These endpoints are used for initial
                // server discovery before federation establishment and are explicitly
                // listed as "Authentication Exempt Endpoints" in the Matrix specification.
                .send()
                .await?;

            let status = response.status();

            // Handle redirects
            if status.is_redirection() {
                if redirect_count >= self.max_redirects {
                    return Err(WellKnownError::TooManyRedirects);
                }

                if let Some(location) = response.headers().get("location") {
                    current_url = location
                        .to_str()
                        .map_err(|_| {
                            WellKnownError::InvalidResponse("Invalid redirect location".to_string())
                        })?
                        .to_string();
                    redirect_count += 1;
                    debug!("Following redirect {} to: {}", redirect_count, current_url);
                    continue;
                } else {
                    return Err(WellKnownError::InvalidResponse(
                        "Redirect without location header".to_string(),
                    ));
                }
            }

            // Handle non-200 responses
            if !status.is_success() {
                return Err(WellKnownError::InvalidResponse(format!("HTTP {}", status)));
            }

            // Calculate cache TTL from headers before consuming response body
            let cache_ttl = self.calculate_cache_ttl_from_response(&response);

            // Parse response
            let text = response.text().await?;

            // Try to parse as JSON, but handle gracefully if it fails
            match serde_json::from_str::<WellKnownResponse>(&text) {
                Ok(well_known) => {
                    // Validate m.server field
                    if well_known.server.is_empty() {
                        return Err(WellKnownError::InvalidResponse(
                            "Empty m.server field".to_string(),
                        ));
                    }
                    return Ok((well_known, cache_ttl));
                },
                Err(e) => {
                    debug!("Failed to parse well-known JSON for {}: {}", hostname, e);
                    return Err(WellKnownError::JsonError(e));
                },
            }
        }
    }

    /// Calculate cache TTL from HTTP response headers
    fn calculate_cache_ttl_from_response(&self, response: &reqwest::Response) -> Duration {
        // Parse Cache-Control max-age directive
        if let Some(cache_control) = response.headers().get("cache-control")
            && let Ok(cache_control_str) = cache_control.to_str()
            && let Some(max_age) = self.parse_max_age(cache_control_str)
        {
            let max_age_duration = Duration::from_secs(max_age);
            let max_allowed = Duration::from_secs(48 * 60 * 60); // 48 hours
            return max_age_duration.min(max_allowed);
        }

        // Parse Expires header using httpdate
        if let Some(expires) = response.headers().get("expires")
            && let Ok(expires_str) = expires.to_str()
            && let Ok(expires_time) = httpdate::parse_http_date(expires_str)
            && let Ok(duration) = expires_time.duration_since(SystemTime::UNIX_EPOCH)
        {
            let now = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
                Ok(d) => d,
                Err(_) => Duration::from_secs(0),
            };
            if duration > now {
                let ttl = Duration::from_secs(duration.as_secs() - now.as_secs());
                let max_allowed = Duration::from_secs(48 * 60 * 60);
                return ttl.min(max_allowed);
            }
        }

        // Default to 24 hours per Matrix spec
        Duration::from_secs(24 * 60 * 60)
    }

    /// Parse max-age directive from Cache-Control header
    fn parse_max_age(&self, cache_control: &str) -> Option<u64> {
        for directive in cache_control.split(',') {
            let directive = directive.trim();
            if let Some(max_age_str) = directive.strip_prefix("max-age=")
                && let Ok(max_age) = max_age_str.parse::<u64>()
            {
                return Some(max_age);
            }
        }
        None
    }

    /// Clear cache entry for a hostname
    pub async fn invalidate_cache(&self, hostname: &str) {
        self.cache.remove(hostname).await;
        debug!("Invalidated well-known cache for {}", hostname);
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> (u64, u64) {
        (self.cache.entry_count(), self.cache.weighted_size())
    }
}
