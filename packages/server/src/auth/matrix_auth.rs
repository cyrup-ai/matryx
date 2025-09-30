use std::collections::HashMap;

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::auth::errors::MatrixAuthError;

/// Matrix access token extracted from Authorization header
#[derive(Debug, Clone)]
pub struct MatrixAccessToken {
    pub token: String,
    pub user_id: String,
    pub device_id: String,
    pub expires_at: Option<i64>,
}

impl MatrixAccessToken {
    /// Check if the access token has expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            chrono::Utc::now().timestamp() > expires_at
        } else {
            false // No expiration means it's not expired
        }
    }
}

/// Matrix server authentication for federation
#[derive(Debug, Clone)]
pub struct MatrixServerAuth {
    pub server_name: String,
    pub key_id: String,
    pub signature: String,
    pub expires_at: Option<i64>,
}

/// Server signing key for Matrix federation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSigningKey {
    pub key_id: String,
    pub server_name: String,
    pub private_key: String,
    pub public_key: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub is_active: bool,
}

/// Matrix authentication context for requests
#[derive(Debug, Clone)]
pub enum MatrixAuth {
    User(MatrixAccessToken),
    Server(MatrixServerAuth),
    Anonymous,
}

impl MatrixAuth {
    /// Get the user ID if this is user authentication
    pub fn user_id(&self) -> Option<&str> {
        match self {
            MatrixAuth::User(token) => Some(&token.user_id),
            _ => None,
        }
    }

    /// Get the server name if this is server authentication
    pub fn server_name(&self) -> Option<&str> {
        match self {
            MatrixAuth::Server(server) => Some(&server.server_name),
            _ => None,
        }
    }

    /// Check if this authentication allows access to a specific resource
    pub fn can_access(&self, _resource: &str) -> bool {
        match self {
            MatrixAuth::User(token) => {
                // User authentication - check token validity
                token.expires_at.is_none_or(|exp| exp > chrono::Utc::now().timestamp())
            },
            MatrixAuth::Server(server) => {
                // Server authentication - check certificate validity
                server.expires_at.is_none_or(|exp| exp > chrono::Utc::now().timestamp())
            },
            MatrixAuth::Anonymous => false,
        }
    }

    /// Check if the authentication token has expired
    pub fn is_expired(&self) -> bool {
        match self {
            MatrixAuth::User(token) => {
                token.expires_at.is_some_and(|exp| exp <= chrono::Utc::now().timestamp())
            },
            MatrixAuth::Server(server) => {
                server
                    .expires_at
                    .is_some_and(|exp| exp <= chrono::Utc::now().timestamp())
            },
            MatrixAuth::Anonymous => false,
        }
    }

    /// Get the access token if this is user authentication
    pub fn access_token(&self) -> Option<&str> {
        match self {
            MatrixAuth::User(token) => Some(&token.token),
            _ => None,
        }
    }

    /// Get the device ID if this is user authentication
    pub fn device_id(&self) -> Option<&str> {
        match self {
            MatrixAuth::User(token) => Some(&token.device_id),
            _ => None,
        }
    }

    /// Get the signature if this is server authentication
    pub fn signature(&self) -> Option<&str> {
        match self {
            MatrixAuth::Server(server) => Some(&server.signature),
            _ => None,
        }
    }
}

/// Matrix JWT claims for SurrealDB integration
#[derive(Debug, Serialize, Deserialize)]
pub struct MatrixJwtClaims {
    // Standard JWT claims
    pub iss: Option<String>,
    pub sub: Option<String>,
    pub aud: Option<String>,
    pub exp: Option<i64>,
    pub iat: Option<i64>,
    pub nbf: Option<i64>,
    pub jti: Option<String>,

    // SurrealDB claims
    #[serde(rename = "NS")]
    pub ns: Option<String>,
    #[serde(rename = "DB")]
    pub db: Option<String>,
    #[serde(rename = "AC")]
    pub ac: Option<String>,
    #[serde(rename = "ID")]
    pub id: Option<String>,
    #[serde(rename = "RL")]
    pub roles: Option<Vec<String>>,

    // Matrix-specific claims
    pub matrix_user_id: Option<String>,
    pub matrix_device_id: Option<String>,
    pub matrix_access_token: Option<String>,
    pub matrix_refresh_token: Option<String>,
    pub matrix_server_name: Option<String>,

    // Additional custom claims
    #[serde(flatten)]
    pub custom_claims: HashMap<String, serde_json::Value>,
}

impl MatrixJwtClaims {
    /// Create claims for Matrix user authentication
    pub fn for_user(
        user_id: &str,
        device_id: &str,
        access_token: &str,
        refresh_token: Option<&str>,
        expires_in: i64,
    ) -> Self {
        let now = Utc::now().timestamp();

        Self {
            iss: Some("matrix-homeserver".to_string()),
            sub: Some(user_id.to_string()),
            exp: Some(now + expires_in),
            iat: Some(now),
            ns: Some("matrix".to_string()),
            db: Some("homeserver".to_string()),
            ac: Some("matrix_client".to_string()),
            id: Some(user_id.to_string()),
            roles: Some(vec!["user".to_string()]),
            matrix_user_id: Some(user_id.to_string()),
            matrix_device_id: Some(device_id.to_string()),
            matrix_access_token: Some(access_token.to_string()),
            matrix_refresh_token: refresh_token.map(|t| t.to_string()),
            matrix_server_name: None,
            custom_claims: HashMap::new(),
            aud: None,
            nbf: None,
            jti: None,
        }
    }

    /// Create claims for Matrix server authentication
    pub fn for_server(server_name: &str, key_id: &str, expires_in: i64) -> Self {
        let now = Utc::now().timestamp();

        Self {
            iss: Some(server_name.to_string()),
            sub: Some(server_name.to_string()),
            exp: Some(now + expires_in),
            iat: Some(now),
            ns: Some("matrix".to_string()),
            db: Some("federation".to_string()),
            ac: Some("matrix_server".to_string()),
            id: Some(server_name.to_string()),
            roles: Some(vec!["server".to_string()]),
            matrix_user_id: None,
            matrix_device_id: None,
            matrix_access_token: None,
            matrix_refresh_token: None,
            matrix_server_name: Some(server_name.to_string()),
            custom_claims: {
                let mut claims = HashMap::new();
                claims.insert("key_id".to_string(), serde_json::Value::String(key_id.to_string()));
                claims
            },
            aud: None,
            nbf: None,
            jti: None,
        }
    }

    /// Validate claims and return Matrix authentication context
    pub fn to_matrix_auth(&self) -> Result<MatrixAuth, MatrixAuthError> {
        if let Some(server_name) = &self.matrix_server_name {
            // Server authentication
            let key_id = self
                .custom_claims
                .get("key_id")
                .and_then(|v| v.as_str())
                .unwrap_or("default");

            Ok(MatrixAuth::Server(MatrixServerAuth {
                server_name: server_name.clone(),
                key_id: key_id.to_string(),
                signature: String::new(), // Signature would be provided separately
                expires_at: self.exp,
            }))
        } else if let Some(user_id) = &self.matrix_user_id {
            // User authentication
            let device_id = self
                .matrix_device_id
                .as_ref()
                .ok_or(MatrixAuthError::InvalidXMatrixFormat)?;
            let access_token = self
                .matrix_access_token
                .as_ref()
                .ok_or(MatrixAuthError::InvalidXMatrixFormat)?;

            Ok(MatrixAuth::User(MatrixAccessToken {
                token: access_token.clone(),
                user_id: user_id.clone(),
                device_id: device_id.clone(),
                expires_at: self.exp,
            }))
        } else {
            Ok(MatrixAuth::Anonymous)
        }
    }
}
