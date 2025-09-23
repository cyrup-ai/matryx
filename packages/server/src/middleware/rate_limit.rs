use crate::{auth::MatrixAuth, error::MatrixError};
use axum::{
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use governor::{
    Quota,
    RateLimiter,
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    num::NonZeroU32,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;

/// Rate limiting service using governor crate with proper per-IP and per-user limiting
pub struct RateLimitService {
    // Per-IP rate limiters (one for each IP address) with last usage tracking
    ip_limiters:
        Arc<RwLock<HashMap<IpAddr, (RateLimiter<NotKeyed, InMemoryState, DefaultClock>, Instant)>>>,
    // Per-user limiters (stored by user ID) with last usage tracking
    user_limiters:
        Arc<RwLock<HashMap<String, (RateLimiter<NotKeyed, InMemoryState, DefaultClock>, Instant)>>>,
    // Configuration
    requests_per_minute: u32,
}

impl RateLimitService {
    pub fn new(requests_per_minute: Option<u32>) -> Result<Self, String> {
        let rpm = requests_per_minute.unwrap_or(100);

        // Validate rate limit value with proper bounds checking
        if rpm == 0 {
            return Err("Rate limit must be greater than 0".to_string());
        }
        if rpm > 10000 {
            return Err("Rate limit must not exceed 10000 requests per minute".to_string());
        }

        Ok(Self {
            ip_limiters: Arc::new(RwLock::new(HashMap::new())),
            user_limiters: Arc::new(RwLock::new(HashMap::new())),
            requests_per_minute: rpm,
        })
    }

    /// Create a new rate limiter with proper error handling
    fn create_rate_limiter(
        &self,
    ) -> Result<RateLimiter<NotKeyed, InMemoryState, DefaultClock>, String> {
        let quota = NonZeroU32::new(self.requests_per_minute)
            .ok_or("Invalid rate limit value: must be greater than 0")?;
        Ok(RateLimiter::direct(Quota::per_minute(quota)))
    }

    /// Check rate limit for IP address with proper per-IP limiting
    pub async fn check_ip_rate_limit(&self, ip: IpAddr) -> Result<(), MatrixError> {
        let mut limiters = self.ip_limiters.write().await;
        let now = Instant::now();

        // Get or create limiter for this IP
        if !limiters.contains_key(&ip) {
            match self.create_rate_limiter() {
                Ok(limiter) => {
                    limiters.insert(ip, (limiter, now));
                },
                Err(_) => return Err(MatrixError::Unknown),
            }
        }

        let (limiter, last_used) = limiters.get_mut(&ip).ok_or(MatrixError::Unknown)?;
        *last_used = now; // Update last usage timestamp

        match limiter.check() {
            Ok(_) => Ok(()),
            Err(_) => {
                // Calculate retry after in milliseconds
                let retry_after_ms = Some(60000 / self.requests_per_minute as u64);
                Err(MatrixError::LimitExceeded { retry_after_ms })
            },
        }
    }

    /// Check rate limit for authenticated user
    pub async fn check_user_rate_limit(&self, user_id: &str) -> Result<(), MatrixError> {
        let mut limiters = self.user_limiters.write().await;
        let now = Instant::now();

        // Get or create limiter for this user
        if !limiters.contains_key(user_id) {
            match self.create_rate_limiter() {
                Ok(limiter) => {
                    limiters.insert(user_id.to_string(), (limiter, now));
                },
                Err(_) => return Err(MatrixError::Unknown),
            }
        }

        let (limiter, last_used) = limiters.get_mut(user_id).ok_or(MatrixError::Unknown)?;
        *last_used = now; // Update last usage timestamp

        match limiter.check() {
            Ok(_) => Ok(()),
            Err(_) => {
                let retry_after_ms = Some(60000 / self.requests_per_minute as u64);
                Err(MatrixError::LimitExceeded { retry_after_ms })
            },
        }
    }

    /// Clean up old rate limiters to prevent memory leaks
    pub async fn cleanup_unused_limiters(&self) {
        let cutoff = Instant::now() - Duration::from_secs(3600); // 1 hour cutoff

        // Remove IP limiters not used in the last hour
        let mut ip_limiters = self.ip_limiters.write().await;
        ip_limiters.retain(|_, (_, last_used)| *last_used > cutoff);

        // Remove user limiters not used in the last hour
        let mut user_limiters = self.user_limiters.write().await;
        user_limiters.retain(|_, (_, last_used)| *last_used > cutoff);
    }
}

/// Rate limiting middleware for Matrix API endpoints with full authentication integration
pub async fn rate_limit_middleware(
    State(rate_limit_service): State<Arc<RateLimitService>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract IP address
    let ip = addr.ip();

    // Check IP-based rate limit first
    if let Err(matrix_error) = rate_limit_service.check_ip_rate_limit(ip).await {
        return Ok(matrix_error.into_response());
    }

    // Check user-based rate limit if user is authenticated
    if let Some(auth) = request.extensions().get::<MatrixAuth>() {
        match auth {
            MatrixAuth::User(user_token) => {
                if let Err(matrix_error) =
                    rate_limit_service.check_user_rate_limit(&user_token.user_id).await
                {
                    return Ok(matrix_error.into_response());
                }
            },
            MatrixAuth::Server(_) => {
                // Server-to-server authentication might have different rate limits
                // For now, we skip user-based rate limiting for server auth
            },
            MatrixAuth::Anonymous => {
                // No user authentication - only IP-based rate limiting applies
            },
        }
    }

    Ok(next.run(request).await)
}

/// Configuration for rate limiting
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub requests_per_minute: u32,
    pub burst_size: u32,
    pub enabled: bool,
}

impl RateLimitConfig {
    pub fn from_env() -> Self {
        // Parse and validate requests_per_minute with proper bounds
        let requests_per_minute = std::env::var("RATE_LIMIT_PER_MINUTE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(100)
            .clamp(1, 10000); // Ensure value is within valid bounds

        // Parse and validate burst_size with proper bounds
        let burst_size = std::env::var("RATE_LIMIT_BURST")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10)
            .clamp(1, 1000); // Ensure value is within valid bounds

        // Parse enabled flag with proper default
        let enabled = std::env::var("RATE_LIMIT_ENABLED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(true);

        Self { requests_per_minute, burst_size, enabled }
    }
}
