//! Module contains intentional library code not yet fully integrated
#![allow(dead_code)]

use crate::auth::captcha::CaptchaConfig;
use crate::middleware::TransactionConfig;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::OnceLock;
use tracing::{error, info, warn};
static SERVER_CONFIG: OnceLock<ServerConfig> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailConfig {
    pub smtp_server: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    pub smtp_password: String,
    pub from_address: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmsConfig {
    pub provider: String, // "twilio"
    pub api_key: String,
    pub api_secret: String,
    pub from_number: String,
    pub api_base_url: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushCacheConfig {
    pub ttl_seconds: u64,
    pub max_capacity: u64,
    pub enable_metrics: bool,
}

impl Default for PushCacheConfig {
    fn default() -> Self {
        Self {
            ttl_seconds: 3600,    // 1 hour TTL
            max_capacity: 1000,   // Max 1000 cached gateways
            enable_metrics: true, // Enable performance monitoring
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Path to client certificate file (PEM format)
    pub client_cert_path: Option<String>,
    /// Path to client private key file (PEM format)
    pub client_key_path: Option<String>,
    /// Enable TLS certificate validation (default: true)
    pub validate_certificates: bool,
    /// Custom CA certificate bundle path
    pub ca_bundle_path: Option<String>,
    /// Domains to skip certificate validation for (testing/onion)
    pub skip_validation_domains: Vec<String>,
    /// Connection timeout in seconds
    pub connect_timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Client API rate limits (requests per minute)
    pub client_requests_per_minute: u32,
    /// Federation API rate limits (requests per minute)
    pub federation_requests_per_minute: u32,
    /// Media endpoint specific limits (requests per minute)
    pub media_requests_per_minute: u32,
    /// Burst size for all rate limiters
    pub burst_size: u32,
    /// Enable rate limiting globally
    pub enabled: bool,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            client_cert_path: None,
            client_key_path: None,
            validate_certificates: true,
            ca_bundle_path: None,
            skip_validation_domains: vec![],
            connect_timeout_secs: 30,
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            client_requests_per_minute: 100,
            federation_requests_per_minute: 200, // Higher for server-to-server
            media_requests_per_minute: 50,       // Lower for media (bandwidth intensive)
            burst_size: 10,
            enabled: true,
        }
    }
}

impl RateLimitConfig {
    pub fn from_env() -> Self {
        Self {
            client_requests_per_minute: env::var("RATE_LIMIT_CLIENT_PER_MINUTE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(100)
                .clamp(1, 10000),
            federation_requests_per_minute: env::var("RATE_LIMIT_FEDERATION_PER_MINUTE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(200)
                .clamp(1, 10000),
            media_requests_per_minute: env::var("RATE_LIMIT_MEDIA_PER_MINUTE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(50)
                .clamp(1, 10000),
            burst_size: env::var("RATE_LIMIT_BURST")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10)
                .clamp(1, 1000),
            enabled: env::var("RATE_LIMIT_ENABLED")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(true),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub homeserver_name: String,
    pub federation_port: u16,
    pub media_base_url: String,
    pub admin_email: String,
    pub environment: String,
    pub server_implementation_name: String,
    pub server_implementation_version: String,
    pub use_https: bool,
    pub email_config: EmailConfig,
    pub sms_config: SmsConfig,
    pub push_cache_config: PushCacheConfig,
    pub transaction_config: TransactionConfig,
    pub tls_config: TlsConfig,
    pub rate_limiting: RateLimitConfig,
    pub captcha: CaptchaConfig,
}

impl ServerConfig {
    pub fn init() -> Result<&'static ServerConfig, ConfigError> {
        Ok(SERVER_CONFIG.get_or_init(|| {
            let homeserver_name = env::var("HOMESERVER_NAME").unwrap_or_else(|_| {
                warn!("HOMESERVER_NAME not set, defaulting to localhost (development only)");
                "localhost".to_string()
            });

            // Add protocol configuration
            let use_https = env::var("USE_HTTPS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true);

            let email_config = EmailConfig {
                smtp_server: env::var("SMTP_SERVER").unwrap_or_else(|_| "localhost".to_string()),
                smtp_port: env::var("SMTP_PORT")
                    .unwrap_or_else(|_| "587".to_string())
                    .parse()
                    .unwrap_or(587),
                smtp_username: env::var("SMTP_USERNAME").unwrap_or_default(),
                smtp_password: env::var("SMTP_PASSWORD").unwrap_or_default(),
                from_address: env::var("FROM_EMAIL")
                    .unwrap_or_else(|_| format!("noreply@{}", homeserver_name)),
                enabled: env::var("EMAIL_ENABLED")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .unwrap_or(true),
            };

            let sms_config = SmsConfig {
                provider: env::var("SMS_PROVIDER").unwrap_or_else(|_| "twilio".to_string()),
                api_key: env::var("SMS_API_KEY").unwrap_or_default(),
                api_secret: env::var("SMS_API_SECRET").unwrap_or_default(),
                from_number: env::var("SMS_FROM_NUMBER").unwrap_or_default(),
                api_base_url: env::var("SMS_API_BASE_URL")
                    .unwrap_or_else(|_| "https://api.twilio.com".to_string()),
                enabled: env::var("SMS_ENABLED")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse()
                    .unwrap_or(false),
            };

            let tls_config = TlsConfig {
                client_cert_path: env::var("TLS_CLIENT_CERT_PATH").ok(),
                client_key_path: env::var("TLS_CLIENT_KEY_PATH").ok(),
                validate_certificates: env::var("TLS_VALIDATE_CERTIFICATES")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .unwrap_or(true),
                ca_bundle_path: env::var("TLS_CA_BUNDLE_PATH").ok(),
                skip_validation_domains: env::var("TLS_SKIP_VALIDATION_DOMAINS")
                    .unwrap_or_default()
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.trim().to_string())
                    .collect(),
                connect_timeout_secs: env::var("TLS_CONNECT_TIMEOUT")
                    .unwrap_or_else(|_| "30".to_string())
                    .parse()
                    .unwrap_or(30),
            };

            let config = ServerConfig {
                homeserver_name: homeserver_name.clone(),

                federation_port: env::var("FEDERATION_PORT")
                    .unwrap_or_else(|_| "8448".to_string())
                    .parse()
                    .unwrap_or(8448),

                media_base_url: env::var("MEDIA_BASE_URL")
                    .unwrap_or_else(|_| {
                        let protocol = if use_https { "https" } else { "http" };
                        format!("{}://{}", protocol, homeserver_name)
                    }),

                admin_email: env::var("MATRIX_ADMIN_EMAIL")
                    .unwrap_or_else(|_| format!("admin@{}", homeserver_name)),

                environment: env::var("DEPLOYMENT_ENV")
                    .unwrap_or_else(|_| "development".to_string()),

                server_implementation_name: env::var("SERVER_IMPLEMENTATION_NAME")
                    .unwrap_or_else(|_| "matryx".to_string()),

                server_implementation_version: env::var("SERVER_IMPLEMENTATION_VERSION")
                    .unwrap_or_else(|_| "0.1.0".to_string()),

                use_https,
                email_config,
                sms_config,
                push_cache_config: PushCacheConfig {
                    ttl_seconds: env::var("PUSH_CACHE_TTL_SECONDS")
                        .unwrap_or_else(|_| "3600".to_string())
                        .parse()
                        .unwrap_or(3600),
                    max_capacity: env::var("PUSH_CACHE_MAX_CAPACITY")
                        .unwrap_or_else(|_| "1000".to_string())
                        .parse()
                        .unwrap_or(1000),
                    enable_metrics: env::var("PUSH_CACHE_ENABLE_METRICS")
                        .unwrap_or_else(|_| "true".to_string())
                        .parse()
                        .unwrap_or(true),
                },
                transaction_config: TransactionConfig::from_env(),
                tls_config,
                rate_limiting: RateLimitConfig::from_env(),
                captcha: CaptchaConfig::from_env(),
            };

            // Enhanced validation - secure by default
            // Only skip validations if explicitly opted out for local development
            let allow_insecure = std::env::var("ALLOW_INSECURE_CONFIG")
                .ok()
                .and_then(|s| s.parse::<bool>().ok())
                .unwrap_or(false);

            if !allow_insecure {
                // 1. Validate HTTPS is enabled
                if !config.use_https {
                    error!("USE_HTTPS must be true when ALLOW_INSECURE_CONFIG is not set");
                    panic!("Invalid configuration: HTTPS required (set ALLOW_INSECURE_CONFIG=true to bypass for development)");
                }

                // 2. Validate homeserver name is not localhost
                if config.homeserver_name == "localhost" {
                    error!("HOMESERVER_NAME must not be localhost");
                    panic!("Invalid configuration: localhost server name");
                }

                // 2. Validate homeserver name format
                if !crate::utils::matrix_identifiers::is_valid_server_name(&config.homeserver_name)
                {
                    error!("Invalid server name format: {}", config.homeserver_name);
                    panic!("Invalid configuration: malformed server name");
                }

                // 3. Validate homeserver name is not an IP literal
                if crate::utils::matrix_identifiers::is_ip_literal(&config.homeserver_name) {
                    error!("homeserver_name cannot be an IP address: {}", config.homeserver_name);
                    panic!("Invalid configuration: homeserver_name must be a domain name (FQDN)");
                }

                // 4. Validate database URL is not in-memory
                let db_url = env::var("DATABASE_URL").unwrap_or_default();
                if db_url.contains("memory://") || db_url == "memory" {
                    error!("DATABASE_URL cannot use in-memory database (data will be lost on restart): {}", db_url);
                    panic!("Invalid configuration: memory database not allowed");
                }
                if db_url.is_empty() {
                    error!("DATABASE_URL must be explicitly set");
                    panic!("Invalid configuration: missing database URL");
                }

                // 5. Validate media base URL uses configured protocol
                let expected_protocol = if config.use_https { "https://" } else { "http://" };
                if !config.media_base_url.starts_with(expected_protocol) {
                    error!("media_base_url must use {}, got: {}", expected_protocol, config.media_base_url);
                    panic!("Invalid configuration: media_base_url protocol mismatch");
                }

                // 6. Validate admin email format
                if config.admin_email.is_empty() || !config.admin_email.contains('@') {
                    error!("admin_email is invalid: {}", config.admin_email);
                    panic!("Invalid configuration: admin_email must be valid email address");
                }

                // 7. Validate TLS certificate validation is enabled
                if !config.tls_config.validate_certificates {
                    error!("TLS certificate validation is disabled");
                    panic!("Invalid configuration: certificate validation must be enabled");
                }
                if !config.tls_config.skip_validation_domains.is_empty() {
                    warn!(
                        "TLS validation is skipped for {} domains: {:?}",
                        config.tls_config.skip_validation_domains.len(),
                        config.tls_config.skip_validation_domains
                    );
                }

                // 8. Validate rate limiting is enabled
                if !config.rate_limiting.enabled {
                    error!("Rate limiting is disabled");
                    panic!("Invalid configuration: rate limiting must be enabled");
                }

                // 9. Validate JWT private key is explicitly set (not auto-generated)
                if env::var("JWT_PRIVATE_KEY").is_err() {
                    error!("JWT_PRIVATE_KEY must be explicitly set (not auto-generated)");
                    panic!("Invalid configuration: JWT_PRIVATE_KEY environment variable required");
                }

                // 10. Warn about development port usage (non-fatal)
                if config.federation_port == 8008 {
                    warn!(
                        "Federation port 8008 is typically for client API. Standard federation uses 8448. \
                        Current setting: {}",
                        config.federation_port
                    );
                }
            } else {
                // Loud warnings when security is bypassed
                warn!("╔════════════════════════════════════════════════════════════╗");
                warn!("║ SECURITY WARNING: ALLOW_INSECURE_CONFIG=true              ║");
                warn!("║ Security validations are DISABLED                         ║");
                warn!("║ This configuration is NOT safe for production deployment  ║");
                warn!("║ Possible risks:                                            ║");
                warn!("║   - Data loss (in-memory database)                        ║");
                warn!("║   - Credential theft (HTTP instead of HTTPS)              ║");
                warn!("║   - MITM attacks (disabled TLS validation)                ║");
                warn!("║   - DoS attacks (disabled rate limiting)                  ║");
                warn!("║   - Session hijacking (weak JWT secrets)                  ║");
                warn!("╚════════════════════════════════════════════════════════════╝");
            }

            // Validate server implementation details
            if config.server_implementation_name.is_empty() {
                error!("Server implementation name cannot be empty");
                panic!("Invalid configuration: empty server implementation name");
            }

            if config.server_implementation_version.is_empty() {
                error!("Server implementation version cannot be empty");
                panic!("Invalid configuration: empty server implementation version");
            }

            info!(
                "Server configuration initialized: server={}, env={}",
                config.homeserver_name, config.environment
            );
            config
        }))
    }

    pub fn get() -> Result<&'static ServerConfig, ConfigError> {
        SERVER_CONFIG
            .get()
            .ok_or_else(|| ConfigError::MissingRequired("ServerConfig not initialized".to_string()))
    }

    /// Get the protocol scheme based on configuration
    /// Returns "https" or "http"
    pub fn protocol_scheme(&self) -> &'static str {
        if self.use_https { "https" } else { "http" }
    }

    /// Build a base URL with the configured protocol
    pub fn base_url(&self) -> String {
        format!("{}://{}", self.protocol_scheme(), self.homeserver_name)
    }

    /// Build a base URL for identity server
    pub fn identity_server_url(&self) -> String {
        format!("{}://identity.{}", self.protocol_scheme(), self.homeserver_name)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Missing required environment variable: {0}")]
    MissingRequired(String),
    #[error("Invalid format for environment variable: {0}")]
    InvalidFormat(String),
    #[error("Production validation failed: {0}")]
    ProductionValidation(String),
}
