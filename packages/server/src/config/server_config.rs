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
            ttl_seconds: 3600,        // 1 hour TTL
            max_capacity: 1000,       // Max 1000 cached gateways
            enable_metrics: true,     // Enable performance monitoring
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
    pub email_config: EmailConfig,
    pub sms_config: SmsConfig,
    pub push_cache_config: PushCacheConfig,
}

impl ServerConfig {
    pub fn init() -> Result<&'static ServerConfig, ConfigError> {
        Ok(SERVER_CONFIG.get_or_init(|| {
            let homeserver_name = env::var("HOMESERVER_NAME")
                .unwrap_or_else(|_| {
                    warn!("HOMESERVER_NAME not set, defaulting to localhost (development only)");
                    "localhost".to_string()
                });

            let email_config = EmailConfig {
                smtp_server: env::var("SMTP_SERVER").unwrap_or_else(|_| "localhost".to_string()),
                smtp_port: env::var("SMTP_PORT")
                    .unwrap_or_else(|_| "587".to_string())
                    .parse()
                    .unwrap_or(587),
                smtp_username: env::var("SMTP_USERNAME").unwrap_or_default(),
                smtp_password: env::var("SMTP_PASSWORD").unwrap_or_default(),
                from_address: env::var("FROM_EMAIL")
                    .unwrap_or_else(|| format!("noreply@{}", homeserver_name)),
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
                enabled: env::var("SMS_ENABLED")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse()
                    .unwrap_or(false),
            };

            let config = ServerConfig {
                homeserver_name: homeserver_name.clone(),

                federation_port: env::var("FEDERATION_PORT")
                    .unwrap_or_else(|_| "8448".to_string())
                    .parse()
                    .unwrap_or(8448),

                media_base_url: env::var("MEDIA_BASE_URL")
                    .unwrap_or_else(|_| format!("https://{}", homeserver_name)),

                admin_email: env::var("MATRIX_ADMIN_EMAIL")
                    .unwrap_or_else(|_| format!("admin@{}", homeserver_name)),

                environment: env::var("DEPLOYMENT_ENV")
                    .unwrap_or_else(|_| "development".to_string()),

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
            };

            // Enhanced validation
            if config.environment == "production" {
                if config.homeserver_name == "localhost" {
                    error!("HOMESERVER_NAME must not be localhost in production");
                    panic!("Invalid production configuration: localhost server name");
                }
                
                if !crate::utils::matrix_identifiers::is_valid_server_name(&config.homeserver_name) {
                    error!("Invalid server name format: {}", config.homeserver_name);
                    panic!("Invalid production configuration: malformed server name");
                }
            }

            info!(
                "Server configuration initialized: server={}, env={}",
                config.homeserver_name, config.environment
            );
            config
        }))
    }

    pub fn get() -> &'static ServerConfig {
        SERVER_CONFIG
            .get()
            .expect("ServerConfig not initialized - call ServerConfig::init() first")
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
