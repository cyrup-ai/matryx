//! Module contains intentional library code not yet fully integrated
#![allow(dead_code)]

use crate::auth::CaptchaService;
use crate::auth::captcha::CaptchaConfig;
use crate::{auth::MatrixAuth, error::MatrixError};
use axum::{
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use governor::{
    Quota, RateLimiter,
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
};
use matryx_surrealdb::repository::CaptchaRepository;
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    num::NonZeroU32,
    sync::Arc,
    time::{Duration, Instant},
};
use surrealdb::engine::any::Any;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Type alias for a rate limiter with timestamp
type RateLimiterEntry = (RateLimiter<NotKeyed, InMemoryState, DefaultClock>, Instant);

/// Type alias for IP-based rate limiters map
type IpLimitersMap = Arc<RwLock<HashMap<IpAddr, RateLimiterEntry>>>;

/// Type alias for user-based rate limiters map
type UserLimitersMap = Arc<RwLock<HashMap<String, RateLimiterEntry>>>;

/// Type alias for server-based rate limiters map
type ServerLimitersMap = Arc<RwLock<HashMap<String, RateLimiterEntry>>>;

/// Rate limiting service using governor crate with proper per-IP, per-user, and per-server limiting
pub struct RateLimitService {
    // Per-IP rate limiters (one for each IP address) with last usage tracking
    ip_limiters: IpLimitersMap,
    // Per-user limiters (stored by user ID) with last usage tracking
    user_limiters: UserLimitersMap,
    // Per-server limiters (stored by server name) with last usage tracking for federation
    server_limiters: ServerLimitersMap,
    // Configuration
    requests_per_minute: u32,
    federation_requests_per_minute: u32,
    media_requests_per_minute: u32,
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
            server_limiters: Arc::new(RwLock::new(HashMap::new())),
            requests_per_minute: rpm,
            federation_requests_per_minute: 200, // Default higher for server-to-server
            media_requests_per_minute: 50,       // Default lower for media (bandwidth intensive)
        })
    }

    pub fn new_with_federation_limits(
        requests_per_minute: Option<u32>,
        federation_requests_per_minute: Option<u32>,
        media_requests_per_minute: Option<u32>,
    ) -> Result<Self, String> {
        let rpm = requests_per_minute.unwrap_or(100);
        let federation_rpm = federation_requests_per_minute.unwrap_or(200);
        let media_rpm = media_requests_per_minute.unwrap_or(50);

        // Validate rate limit values with proper bounds checking
        if rpm == 0 || federation_rpm == 0 || media_rpm == 0 {
            return Err("Rate limits must be greater than 0".to_string());
        }
        if rpm > 10000 || federation_rpm > 10000 || media_rpm > 10000 {
            return Err("Rate limits must not exceed 10000 requests per minute".to_string());
        }

        Ok(Self {
            ip_limiters: Arc::new(RwLock::new(HashMap::new())),
            user_limiters: Arc::new(RwLock::new(HashMap::new())),
            server_limiters: Arc::new(RwLock::new(HashMap::new())),
            requests_per_minute: rpm,
            federation_requests_per_minute: federation_rpm,
            media_requests_per_minute: media_rpm,
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

    /// Create a new federation rate limiter with proper error handling
    fn create_federation_rate_limiter(
        &self,
    ) -> Result<RateLimiter<NotKeyed, InMemoryState, DefaultClock>, String> {
        let quota = NonZeroU32::new(self.federation_requests_per_minute)
            .ok_or("Invalid federation rate limit value: must be greater than 0")?;
        Ok(RateLimiter::direct(Quota::per_minute(quota)))
    }

    /// Create a new media rate limiter with proper error handling
    fn create_media_rate_limiter(
        &self,
    ) -> Result<RateLimiter<NotKeyed, InMemoryState, DefaultClock>, String> {
        let quota = NonZeroU32::new(self.media_requests_per_minute)
            .ok_or("Invalid media rate limit value: must be greater than 0")?;
        Ok(RateLimiter::direct(Quota::per_minute(quota)))
    }

    /// Check rate limit for IP address with proper per-IP limiting
    pub async fn check_ip_rate_limit(&self, ip: IpAddr) -> Result<(), MatrixError> {
        let mut limiters = self.ip_limiters.write().await;
        let now = Instant::now();

        // Get or create limiter for this IP
        if let std::collections::hash_map::Entry::Vacant(e) = limiters.entry(ip) {
            match self.create_rate_limiter() {
                Ok(limiter) => {
                    e.insert((limiter, now));
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

    /// Check rate limit for federation server
    pub async fn check_server_rate_limit(&self, server_name: &str) -> Result<(), MatrixError> {
        let mut limiters = self.server_limiters.write().await;
        let now = Instant::now();

        // Get or create limiter for this server
        if !limiters.contains_key(server_name) {
            match self.create_federation_rate_limiter() {
                Ok(limiter) => {
                    limiters.insert(server_name.to_string(), (limiter, now));
                },
                Err(_) => return Err(MatrixError::Unknown),
            }
        }

        let (limiter, last_used) = limiters.get_mut(server_name).ok_or(MatrixError::Unknown)?;
        *last_used = now; // Update last usage timestamp

        match limiter.check() {
            Ok(_) => Ok(()),
            Err(_) => {
                let retry_after_ms = Some(60000 / self.federation_requests_per_minute as u64);
                Err(MatrixError::LimitExceeded { retry_after_ms })
            },
        }
    }

    /// Check rate limit for federation server media requests
    pub async fn check_server_media_rate_limit(
        &self,
        server_name: &str,
    ) -> Result<(), MatrixError> {
        let mut limiters = self.server_limiters.write().await;
        let now = Instant::now();

        // Use a different key for media rate limiting to separate from regular federation limits
        let media_key = format!("{}_media", server_name);

        // Get or create limiter for this server's media requests
        if !limiters.contains_key(&media_key) {
            match self.create_media_rate_limiter() {
                Ok(limiter) => {
                    limiters.insert(media_key.clone(), (limiter, now));
                },
                Err(_) => return Err(MatrixError::Unknown),
            }
        }

        let (limiter, last_used) = limiters.get_mut(&media_key).ok_or(MatrixError::Unknown)?;
        *last_used = now; // Update last usage timestamp

        match limiter.check() {
            Ok(_) => Ok(()),
            Err(_) => {
                let retry_after_ms = Some(60000 / self.media_requests_per_minute as u64);
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

        // Remove server limiters not used in the last hour
        let mut server_limiters = self.server_limiters.write().await;
        server_limiters.retain(|_, (_, last_used)| *last_used > cutoff);
    }

    /// Check if CAPTCHA is required for the given IP and operation
    pub async fn is_captcha_required(
        &self,
        db: Arc<surrealdb::Surreal<Any>>,
        ip_address: &str,
        operation: &str,
    ) -> bool {
        let config = CaptchaConfig::from_env();
        let captcha_repo = CaptchaRepository::new((*db).clone());
        let captcha_service = CaptchaService::new(captcha_repo, config);

        match captcha_service.is_captcha_required(ip_address, operation).await {
            Ok(required) => required,
            Err(e) => {
                warn!("Failed to check CAPTCHA requirement: {}", e);
                false
            },
        }
    }

    /// Record a rate limit violation for CAPTCHA tracking
    pub async fn record_rate_limit_violation(
        &self,
        db: Arc<surrealdb::Surreal<Any>>,
        ip_address: &str,
        operation: &str,
    ) {
        let config = CaptchaConfig::from_env();
        let captcha_repo = CaptchaRepository::new((*db).clone());
        let captcha_service = CaptchaService::new(captcha_repo, config);

        if let Err(e) = captcha_service.record_rate_limit_violation(ip_address, operation).await {
            error!("Failed to record rate limit violation: {}", e);
        }
    }

    /// Create a CAPTCHA challenge when rate limits are exceeded
    pub async fn create_captcha_challenge(
        &self,
        db: Arc<surrealdb::Surreal<Any>>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        session_id: Option<String>,
    ) -> Option<String> {
        let config = CaptchaConfig::from_env();
        let captcha_repo = CaptchaRepository::new((*db).clone());
        let captcha_service = CaptchaService::new(captcha_repo, config);

        match captcha_service.create_challenge(ip_address, user_agent, session_id).await {
            Ok(challenge) => Some(challenge.challenge_id),
            Err(e) => {
                error!("Failed to create CAPTCHA challenge: {}", e);
                None
            },
        }
    }

    /// Get CAPTCHA statistics for monitoring
    pub async fn get_captcha_stats(&self, db: Arc<surrealdb::Surreal<Any>>) -> Option<String> {
        let config = CaptchaConfig::from_env();
        let captcha_repo = CaptchaRepository::new((*db).clone());
        let captcha_service = CaptchaService::new(captcha_repo, config);

        match captcha_service.get_captcha_stats().await {
            Ok(_stats) => Some("CAPTCHA stats retrieved".to_string()),
            Err(e) => {
                error!("Failed to get CAPTCHA stats: {}", e);
                None
            },
        }
    }

    /// Cleanup expired CAPTCHA challenges
    pub async fn cleanup_expired_captcha(&self, db: Arc<surrealdb::Surreal<Any>>) -> u64 {
        let config = CaptchaConfig::from_env();
        let captcha_repo = CaptchaRepository::new((*db).clone());
        let captcha_service = CaptchaService::new(captcha_repo, config);

        match captcha_service.cleanup_expired_challenges().await {
            Ok(count) => {
                info!("Cleaned up {} expired CAPTCHA challenges", count);
                count
            },
            Err(e) => {
                error!("Failed to cleanup expired CAPTCHA challenges: {}", e);
                0
            },
        }
    }

    /// Check suspicious activity patterns for CAPTCHA triggering
    pub async fn check_suspicious_activity(
        &self,
        db: Arc<surrealdb::Surreal<Any>>,
        ip_address: &str,
    ) -> bool {
        let config = CaptchaConfig::from_env();
        let captcha_repo = CaptchaRepository::new((*db).clone());
        let captcha_service = CaptchaService::new(captcha_repo, config);

        // Use the private check_suspicious_activity method through the public is_captcha_required method
        match captcha_service.is_captcha_required(ip_address, "suspicious_check").await {
            Ok(suspicious) => suspicious,
            Err(e) => {
                warn!("Failed to check suspicious activity: {}", e);
                false
            },
        }
    }
}

/// Rate limiting middleware for Matrix API endpoints with full authentication integration
pub async fn rate_limit_middleware(
    State(rate_limit_service): State<Arc<RateLimitService>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract IP address
    let ip = addr.ip();

    // Check IP-based rate limit first
    if let Err(_matrix_error) = rate_limit_service.check_ip_rate_limit(ip).await {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    // Determine endpoint type for specialized rate limiting
    let is_media_endpoint = request.uri().path().contains("/media/");

    // Check authentication-specific rate limits
    if let Some(auth) = request.extensions().get::<MatrixAuth>() {
        match auth {
            MatrixAuth::User(user_token) => {
                if let Err(_matrix_error) =
                    rate_limit_service.check_user_rate_limit(&user_token.user_id).await
                {
                    return Err(StatusCode::TOO_MANY_REQUESTS);
                }
            },
            MatrixAuth::Server(_) => {
                // Apply server-based rate limiting for federation
                if let Some(server_name) = auth.server_name() {
                    let result = if is_media_endpoint {
                        rate_limit_service.check_server_media_rate_limit(server_name).await
                    } else {
                        rate_limit_service.check_server_rate_limit(server_name).await
                    };

                    if let Err(_matrix_error) = result {
                        return Err(StatusCode::TOO_MANY_REQUESTS);
                    }
                } else {
                    return Err(StatusCode::UNAUTHORIZED);
                }
            },
            MatrixAuth::Anonymous => {
                // No user authentication - only IP-based rate limiting applies
            },
        }
    }

    Ok(next.run(request).await)
}
