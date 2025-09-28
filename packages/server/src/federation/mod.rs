pub mod authorization;
pub mod client;
pub mod device_edu_handler;
pub mod device_management;
pub mod dns_resolver;
pub mod event_signer;
pub mod event_signing;
pub mod key_management;
pub mod media_client;
pub mod membership_federation;
pub mod pdu_validator;
pub mod server_discovery;
pub mod state_resolution;
pub mod well_known_client;

use reqwest::{Client, ClientBuilder, Certificate, Identity};
use std::fs;
use std::time::Duration;
use crate::config::server_config::{ServerConfig, TlsConfig};

#[derive(Debug, thiserror::Error)]
pub enum TlsError {
    #[error("Failed to load client certificate: {0}")]
    ClientCertificateError(String),
    #[error("Failed to load CA bundle: {0}")]
    CaBundleError(String),
    #[error("TLS configuration error: {0}")]
    ConfigError(String),
    #[error("HTTP client creation failed: {0}")]
    ClientCreationError(#[from] reqwest::Error),
}

/// Create HTTP client with Matrix-compliant TLS configuration
pub fn create_federation_http_client() -> Result<Client, TlsError> {
    let config = ServerConfig::get().map_err(|e| TlsError::ConfigError(format!("{:?}", e)))?;
    create_http_client_with_config(&config.tls_config)
}

pub fn create_http_client_with_config(tls_config: &TlsConfig) -> Result<Client, TlsError> {
    let mut builder = ClientBuilder::new()
        .timeout(Duration::from_secs(tls_config.connect_timeout_secs))
        .connect_timeout(Duration::from_secs(tls_config.connect_timeout_secs));

    // Configure certificate validation
    if !tls_config.validate_certificates {
        tracing::warn!("TLS certificate validation is DISABLED - only use for testing!");
        builder = builder.danger_accept_invalid_certs(true);
    }

    // Load custom CA bundle if specified
    if let Some(ca_path) = &tls_config.ca_bundle_path {
        let ca_cert_pem = fs::read(ca_path)
            .map_err(|e| TlsError::CaBundleError(format!("Failed to read CA bundle: {}", e)))?;
        let ca_cert = Certificate::from_pem(&ca_cert_pem)
            .map_err(|e| TlsError::CaBundleError(format!("Invalid CA certificate: {}", e)))?;
        builder = builder.add_root_certificate(ca_cert);
    }

    // Load client certificate if specified
    if let (Some(cert_path), Some(key_path)) = (&tls_config.client_cert_path, &tls_config.client_key_path) {
        let cert_pem = fs::read(cert_path)
            .map_err(|e| TlsError::ClientCertificateError(format!("Failed to read certificate: {}", e)))?;
        let key_pem = fs::read(key_path)
            .map_err(|e| TlsError::ClientCertificateError(format!("Failed to read private key: {}", e)))?;

        let mut identity_pem = cert_pem.clone();
        identity_pem.extend_from_slice(&key_pem);

        let identity = Identity::from_pkcs12_der(&identity_pem, "")
            .or_else(|_| Identity::from_pkcs8_pem(&cert_pem, &key_pem))
            .map_err(|e| TlsError::ClientCertificateError(format!("Invalid client certificate: {}", e)))?;
        builder = builder.identity(identity);
    }

    builder.build().map_err(TlsError::ClientCreationError)
}




