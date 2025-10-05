use crate::repository::error::RepositoryError;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::{Connection, Surreal};
use uuid::Uuid;

/// Authorization code stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationCode {
    pub code: String,
    pub client_id: String,
    pub user_id: String,
    pub redirect_uri: String,
    pub scope: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub used: bool,
}

/// OAuth 2.0 client registration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuth2Client {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub client_name: String,
    pub redirect_uris: Vec<String>,
    pub client_type: String, // "public" or "confidential"
    pub created_at: DateTime<Utc>,
    pub is_active: bool,
    pub allowed_scopes: Vec<String>,
}

pub struct OAuth2Repository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> OAuth2Repository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    /// Create authorization code in database
    pub async fn create_authorization_code(
        &self,
        client_id: &str,
        user_id: &str,
        redirect_uri: &str,
        scope: Option<&str>,
        code_challenge: Option<&str>,
        code_challenge_method: Option<&str>,
    ) -> Result<String, RepositoryError> {
        let code = format!("mxac_{}", Uuid::new_v4());
        let now = Utc::now();
        let expires_at = now + Duration::minutes(10); // 10 minute validity

        let auth_code = AuthorizationCode {
            code: code.clone(),
            client_id: client_id.to_string(),
            user_id: user_id.to_string(),
            redirect_uri: redirect_uri.to_string(),
            scope: scope.map(|s| s.to_string()),
            code_challenge: code_challenge.map(|c| c.to_string()),
            code_challenge_method: code_challenge_method.map(|m| m.to_string()),
            created_at: now,
            expires_at,
            used: false,
        };

        let _: Option<AuthorizationCode> =
            self.db.create(("oauth2_codes", &code)).content(auth_code).await?;

        Ok(code)
    }

    /// Get authorization code by code
    pub async fn get_authorization_code(
        &self,
        code: &str,
    ) -> Result<Option<AuthorizationCode>, RepositoryError> {
        let auth_code: Option<AuthorizationCode> = self.db.select(("oauth2_codes", code)).await?;
        Ok(auth_code)
    }

    /// Validate and consume authorization code
    pub async fn consume_authorization_code(
        &self,
        code: &str,
    ) -> Result<Option<AuthorizationCode>, RepositoryError> {
        // Get the authorization code
        let auth_code = self.get_authorization_code(code).await?;

        if let Some(mut auth_code) = auth_code {
            // Check if already used
            if auth_code.used {
                return Ok(None);
            }

            // Check if expired
            if Utc::now() > auth_code.expires_at {
                return Ok(None);
            }

            // Mark as used
            let query = "UPDATE oauth2_codes SET used = true WHERE id = $code";
            self.db.query(query).bind(("code", code.to_string())).await?;

            // Update the local copy
            auth_code.used = true;
            Ok(Some(auth_code))
        } else {
            Ok(None)
        }
    }

    /// Get OAuth2 client by ID
    pub async fn get_client(
        &self,
        client_id: &str,
    ) -> Result<Option<OAuth2Client>, RepositoryError> {
        let client: Option<OAuth2Client> = self.db.select(("oauth2_clients", client_id)).await?;

        if let Some(client) = client {
            if client.is_active {
                Ok(Some(client))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// Register a new OAuth2 client
    pub async fn register_client(
        &self,
        client_name: &str,
        redirect_uris: Vec<String>,
        client_type: &str,
        allowed_scopes: Option<Vec<String>>,
    ) -> Result<OAuth2Client, RepositoryError> {
        let client_id = format!("mxcl_{}", Uuid::new_v4());
        let client_secret = if client_type == "confidential" {
            Some(format!("mxcs_{}", Uuid::new_v4()))
        } else {
            None
        };

        // Default scopes if none provided
        let scopes = allowed_scopes.unwrap_or_else(|| vec![
            "openid".to_string(),
            "profile".to_string(),
            "email".to_string(),
            "urn:matrix:client:api:*".to_string(),
        ]);

        let client = OAuth2Client {
            client_id: client_id.clone(),
            client_secret: client_secret.clone(),
            client_name: client_name.to_string(),
            redirect_uris: redirect_uris.clone(),
            client_type: client_type.to_string(),
            created_at: Utc::now(),
            is_active: true,
            allowed_scopes: scopes,
        };

        let _: Option<OAuth2Client> = self
            .db
            .create(("oauth2_clients", &client_id))
            .content(client.clone())
            .await?;

        Ok(client)
    }

    /// Deactivate an OAuth2 client
    pub async fn deactivate_client(&self, client_id: &str) -> Result<(), RepositoryError> {
        let query = "UPDATE oauth2_clients SET is_active = false WHERE id = $client_id";
        self.db.query(query).bind(("client_id", client_id.to_string())).await?;
        Ok(())
    }

    /// Get all active clients
    pub async fn get_active_clients(&self) -> Result<Vec<OAuth2Client>, RepositoryError> {
        let query = "SELECT * FROM oauth2_clients WHERE is_active = true";
        let mut result = self.db.query(query).await?;
        let clients: Vec<OAuth2Client> = result.take(0)?;
        Ok(clients)
    }

    /// Clean up expired authorization codes
    pub async fn cleanup_expired_codes(&self) -> Result<u64, RepositoryError> {
        let query = "DELETE FROM oauth2_codes WHERE expires_at < datetime::now() OR used = true";
        let mut response = self.db.query(query).await?;
        let deleted_count: Option<u64> = response.take(0).unwrap_or(Some(0));
        Ok(deleted_count.unwrap_or(0))
    }

    /// Get authorization code statistics
    pub async fn get_code_stats(&self) -> Result<CodeStats, RepositoryError> {
        let query = "
            SELECT 
                count() as total_codes,
                count(used = true) as used_count,
                count(expires_at < datetime::now()) as expired_count
            FROM oauth2_codes
        ";

        let mut response = self.db.query(query).await?;
        let stats: Option<CodeStats> = response.take(0)?;
        stats.ok_or_else(|| {
            RepositoryError::NotFound {
                entity_type: "CodeStats".to_string(),
                id: "stats".to_string(),
            }
        })
    }

    /// Validate redirect URI for a client
    pub async fn validate_redirect_uri(
        &self,
        client_id: &str,
        redirect_uri: &str,
    ) -> Result<bool, RepositoryError> {
        let client: Option<OAuth2Client> = self.db.select(("oauth2_clients", client_id)).await?;

        if let Some(client) = client {
            Ok(client.redirect_uris.contains(&redirect_uri.to_string()))
        } else {
            Ok(false)
        }
    }

    /// Validate scope for a client
    /// 
    /// Validates that all requested scopes are in the client's allowed_scopes list.
    /// Scopes are space-delimited per RFC 6749 section 3.3.
    /// 
    /// # Arguments
    /// * `client_id` - The OAuth2 client ID
    /// * `scope` - Space-delimited scope string (e.g., "openid profile email")
    /// 
    /// # Returns
    /// * `Ok(true)` - All requested scopes are valid
    /// * `Ok(false)` - One or more requested scopes are not allowed
    /// * `Err(_)` - Database error or client not found
    pub async fn validate_scope(
        &self,
        client_id: &str,
        scope: &str,
    ) -> Result<bool, RepositoryError> {
        // Fetch the client to get allowed scopes
        let client = self.get_client(client_id).await?;
        
        if let Some(client) = client {
            // Parse requested scopes (space-delimited per RFC 6749)
            let requested_scopes: Vec<&str> = scope.split_whitespace().collect();
            
            // Check each requested scope against allowed scopes
            for requested in requested_scopes {
                let mut is_allowed = false;
                
                // Check for exact match or wildcard match
                for allowed in &client.allowed_scopes {
                    if allowed == requested {
                        // Exact match
                        is_allowed = true;
                        break;
                    } else if allowed.ends_with("*") {
                        // Wildcard match (e.g., "urn:matrix:client:api:*")
                        let prefix = &allowed[..allowed.len() - 1];
                        if requested.starts_with(prefix) {
                            is_allowed = true;
                            break;
                        }
                    }
                }
                
                // If any scope is not allowed, validation fails
                if !is_allowed {
                    return Ok(false);
                }
            }
            
            // All scopes are allowed
            Ok(true)
        } else {
            // Client not found or inactive
            Ok(false)
        }
    }

    /// Validate PKCE challenge
    pub async fn validate_pkce_challenge(
        &self,
        verifier: &str,
        challenge: &str,
        method: &str,
    ) -> Result<bool, RepositoryError> {
        use base64::{Engine, engine::general_purpose};
        use sha2::{Digest, Sha256};

        match method {
            "S256" => {
                let mut hasher = Sha256::new();
                hasher.update(verifier.as_bytes());
                let hash = hasher.finalize();
                let encoded = general_purpose::URL_SAFE_NO_PAD.encode(hash);
                Ok(encoded == challenge)
            },
            "plain" => Ok(verifier == challenge),
            _ => Ok(false),
        }
    }
}

/// Authorization code statistics for monitoring
#[derive(Debug, Serialize, Deserialize)]
pub struct CodeStats {
    pub total_codes: u64,
    pub used_count: u64,
    pub expired_count: u64,
}
