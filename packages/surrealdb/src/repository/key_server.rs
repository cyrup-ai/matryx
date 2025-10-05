use crate::repository::error::RepositoryError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use surrealdb::{Connection, Surreal};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerKeys {
    pub server_name: String,
    pub valid_until_ts: i64,
    pub verify_keys: HashMap<String, VerifyKey>,
    pub old_verify_keys: HashMap<String, OldVerifyKey>,
    pub signatures: HashMap<String, HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyKey {
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OldVerifyKey {
    pub key: String,
    pub expired_ts: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningKey {
    pub key_id: String,
    pub server_name: String,
    pub signing_key: String,
    pub verify_key: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct KeyServerRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> KeyServerRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    pub async fn get_server_keys(
        &self,
        server_name: &str,
        key_ids: Option<&[String]>,
    ) -> Result<ServerKeys, RepositoryError> {
        let query = if let Some(_key_ids) = key_ids {
            "SELECT * FROM server_keys WHERE server_name = $server_name AND valid_until_ts > $now AND verify_keys CONTAINSANY $key_ids LIMIT 1"
        } else {
            "SELECT * FROM server_keys WHERE server_name = $server_name AND valid_until_ts > $now ORDER BY valid_until_ts DESC LIMIT 1"
        };

        let mut result = self
            .db
            .query(query)
            .bind(("server_name", server_name.to_string()))
            .bind(("now", Utc::now().timestamp()));

        if let Some(key_ids) = key_ids {
            result = result.bind(("key_ids", key_ids.to_vec()));
        }

        let keys: Option<ServerKeys> = result.await?.take(0)?;

        keys.ok_or_else(|| {
            RepositoryError::NotFound {
                entity_type: "ServerKeys".to_string(),
                id: server_name.to_string(),
            }
        })
    }

    pub async fn store_server_keys(
        &self,
        server_name: &str,
        keys: &ServerKeys,
        valid_until: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let mut keys_with_validity = keys.clone();
        keys_with_validity.valid_until_ts = valid_until.timestamp();

        let record_id = format!("{}_{}", server_name, valid_until.timestamp());
        let _created: Option<ServerKeys> = self
            .db
            .create(("server_keys", record_id))
            .content(keys_with_validity)
            .await?;

        Ok(())
    }

    pub async fn get_signing_key(
        &self,
        server_name: &str,
        key_id: &str,
    ) -> Result<Option<SigningKey>, RepositoryError> {
        let query = "SELECT * FROM signing_keys WHERE server_name = $server_name AND key_id = $key_id AND (expires_at IS NONE OR expires_at > $now) LIMIT 1";
        let mut result = self
            .db
            .query(query)
            .bind(("server_name", server_name.to_string()))
            .bind(("key_id", key_id.to_string()))
            .bind(("now", Utc::now()))
            .await?;

        let key: Option<SigningKey> = result.take(0)?;
        Ok(key)
    }

    pub async fn store_signing_key(
        &self,
        server_name: &str,
        key_id: &str,
        key: &SigningKey,
    ) -> Result<(), RepositoryError> {
        let record_id = format!("{}_{}", server_name, key_id);
        let _created: Option<SigningKey> =
            self.db.create(("signing_keys", record_id)).content(key.clone()).await?;

        Ok(())
    }

    pub async fn verify_key_signature(
        &self,
        server_name: &str,
        key_id: &str,
        signature: &str,
        content: &[u8],
    ) -> Result<bool, RepositoryError> {
        use base64::{Engine as _, engine::general_purpose};
        use ed25519_dalek::{Signature, VerifyingKey, Verifier};

        // Get the verify key for this server and key_id
        let signing_key = match self.get_signing_key(server_name, key_id).await? {
            Some(key) => key,
            None => {
                return Err(RepositoryError::NotFound {
                    entity_type: "SigningKey".to_string(),
                    id: format!("{}:{}", server_name, key_id),
                });
            }
        };

        // Validate key_id format (must be ed25519:KEYID)
        if !key_id.starts_with("ed25519:") {
            return Err(RepositoryError::Validation {
                field: "key_id".to_string(),
                message: format!("Unsupported key algorithm: {}", key_id),
            });
        }

        // Decode signature from base64
        let signature_bytes = general_purpose::STANDARD.decode(signature).map_err(|e| {
            RepositoryError::Validation {
                field: "signature".to_string(),
                message: format!("Invalid base64 signature: {}", e),
            }
        })?;

        // Decode verify key from base64
        let verify_key_bytes = general_purpose::STANDARD.decode(&signing_key.verify_key).map_err(|e| {
            RepositoryError::Validation {
                field: "verify_key".to_string(),
                message: format!("Invalid base64 verify key: {}", e),
            }
        })?;

        // Validate sizes
        if signature_bytes.len() != 64 {
            return Err(RepositoryError::Validation {
                field: "signature".to_string(),
                message: format!("Invalid signature length: {} (expected 64)", signature_bytes.len()),
            });
        }

        if verify_key_bytes.len() != 32 {
            return Err(RepositoryError::Validation {
                field: "verify_key".to_string(),
                message: format!("Invalid key length: {} (expected 32)", verify_key_bytes.len()),
            });
        }

        // Convert to fixed-size arrays
        let key_array: [u8; 32] = match verify_key_bytes.try_into() {
            Ok(arr) => arr,
            Err(_) => {
                return Err(RepositoryError::Validation {
                    field: "verify_key".to_string(),
                    message: "Failed to convert key bytes to array".to_string(),
                });
            }
        };

        let sig_array: [u8; 64] = match signature_bytes.try_into() {
            Ok(arr) => arr,
            Err(_) => {
                return Err(RepositoryError::Validation {
                    field: "signature".to_string(),
                    message: "Failed to convert signature bytes to array".to_string(),
                });
            }
        };

        // Create verifying key
        let verifying_key = VerifyingKey::from_bytes(&key_array).map_err(|e| {
            RepositoryError::Validation {
                field: "verify_key".to_string(),
                message: format!("Invalid Ed25519 key: {}", e),
            }
        })?;

        // Create signature object
        let signature_obj = Signature::from_bytes(&sig_array);

        // Verify signature against content
        match verifying_key.verify(content, &signature_obj) {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    pub async fn get_key_validity(
        &self,
        server_name: &str,
        key_id: &str,
    ) -> Result<Option<DateTime<Utc>>, RepositoryError> {
        let query = "SELECT VALUE expires_at FROM signing_keys WHERE server_name = $server_name AND key_id = $key_id LIMIT 1";
        let mut result = self
            .db
            .query(query)
            .bind(("server_name", server_name.to_string()))
            .bind(("key_id", key_id.to_string()))
            .await?;

        let expires_at: Option<DateTime<Utc>> = result.take(0)?;
        Ok(expires_at)
    }

    pub async fn cleanup_expired_keys(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<u64, RepositoryError> {
        // Clean up expired server keys
        let server_keys_result: Vec<ServerKeys> = self
            .db
            .query("DELETE FROM server_keys WHERE valid_until_ts < $cutoff_ts RETURN BEFORE")
            .bind(("cutoff_ts", cutoff.timestamp()))
            .await?
            .take(0)?;

        // Clean up expired signing keys
        let signing_keys_result: Vec<SigningKey> = self.db
            .query("DELETE FROM signing_keys WHERE expires_at IS NOT NONE AND expires_at < $cutoff RETURN BEFORE")
            .bind(("cutoff", cutoff))
            .await?
            .take(0)?;

        Ok((server_keys_result.len() + signing_keys_result.len()) as u64)
    }

    /// Get server signing key - used by PDU validator for signature verification
    pub async fn get_server_signing_key(
        &self,
        server_name: &str,
        key_id: &str,
    ) -> Result<Option<String>, RepositoryError> {
        let query = "
            SELECT public_key
            FROM server_signing_keys
            WHERE server_name = $server_name 
              AND key_id = $key_id 
              AND is_active = true
        ";

        let mut response = self
            .db
            .query(query)
            .bind(("server_name", server_name.to_string()))
            .bind(("key_id", key_id.to_string()))
            .await
            .map_err(|e| {
                RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "query server signing keys".to_string(),
                }
            })?;

        let public_key: Option<String> = response.take(0).map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "parse server signing key query result".to_string(),
            }
        })?;

        Ok(public_key)
    }

    /// Cache server signing key - used by PDU validator to store fetched keys
    pub async fn cache_server_signing_key(
        &self,
        server_name: &str,
        key_id: &str,
        public_key: &str,
        fetched_at: chrono::DateTime<chrono::Utc>,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), RepositoryError> {
        let query = "
            CREATE server_signing_keys CONTENT {
                server_name: $server_name,
                key_id: $key_id,
                public_key: $public_key,
                fetched_at: $fetched_at,
                is_active: true,
                expires_at: $expires_at
            }
        ";

        self.db
            .query(query)
            .bind(("server_name", server_name.to_string()))
            .bind(("key_id", key_id.to_string()))
            .bind(("public_key", public_key.to_string()))
            .bind(("fetched_at", fetched_at))
            .bind(("expires_at", expires_at))
            .await
            .map_err(|e| {
                RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "cache server signing key".to_string(),
                }
            })?;

        Ok(())
    }

    /// Get active signing key IDs for a server - used by event signer
    pub async fn get_active_signing_key_ids(
        &self,
        server_name: &str,
    ) -> Result<Vec<String>, RepositoryError> {
        let query = "
            SELECT key_id
            FROM server_signing_keys
            WHERE server_name = $server_name
              AND is_active = true
              AND (expires_at IS NULL OR expires_at > datetime::now())
            ORDER BY created_at DESC
        ";

        let mut response = self
            .db
            .query(query)
            .bind(("server_name", server_name.to_string()))
            .await
            .map_err(|e| {
                RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "query signing key IDs".to_string(),
                }
            })?;

        let key_ids: Vec<String> = response.take(0).map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "parse signing key ID query result".to_string(),
            }
        })?;

        Ok(key_ids)
    }

    /// Store new server signing key - used by event signer for key generation
    pub async fn store_server_signing_key(
        &self,
        key_id: &str,
        server_name: &str,
        private_key: &str,
        public_key: &str,
        created_at: chrono::DateTime<chrono::Utc>,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<(), RepositoryError> {
        let query = "
            CREATE server_signing_keys CONTENT {
                key_id: $key_id,
                server_name: $server_name,
                private_key: $private_key,
                public_key: $public_key,
                created_at: $created_at,
                expires_at: $expires_at,
                is_active: true
            }
        ";

        self.db
            .query(query)
            .bind(("key_id", key_id.to_string()))
            .bind(("server_name", server_name.to_string()))
            .bind(("private_key", private_key.to_string()))
            .bind(("public_key", public_key.to_string()))
            .bind(("created_at", created_at))
            .bind(("expires_at", expires_at))
            .await
            .map_err(|e| {
                RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "store signing key".to_string(),
                }
            })?;

        Ok(())
    }

    /// Get server signing key by server name (for our own server)
    pub async fn get_server_signing_key_by_server(
        &self,
        server_name: &str,
    ) -> Result<Option<ServerSigningKeyRecord>, RepositoryError> {
        let query = "
            SELECT key_id, server_name, private_key, public_key, created_at, expires_at, is_active
            FROM server_signing_keys
            WHERE server_name = $server_name 
              AND is_active = true
              AND (expires_at IS NULL OR expires_at > datetime::now())
            ORDER BY created_at DESC
            LIMIT 1
        ";

        let mut result =
            self.db.query(query).bind(("server_name", server_name.to_string())).await?;

        let keys: Vec<ServerSigningKeyRecord> = result.take(0)?;
        Ok(keys.into_iter().next())
    }

    /// Create server signing key record
    pub async fn create_server_signing_key_record(
        &self,
        key_data: &ServerSigningKeyRecord,
    ) -> Result<(), RepositoryError> {
        let _: Option<ServerSigningKeyRecord> = self
            .db
            .create(("server_signing_keys", &key_data.key_id))
            .content(key_data.clone())
            .await?;
        Ok(())
    }

    /// Get private key for JSON signing
    pub async fn get_private_key_for_signing(
        &self,
        key_id: &str,
    ) -> Result<Option<String>, RepositoryError> {
        let query = "
            SELECT VALUE private_key
            FROM server_signing_keys
            WHERE key_id = $key_id 
              AND is_active = true
              AND (expires_at IS NULL OR expires_at > datetime::now())
            LIMIT 1
        ";

        let mut result = self.db.query(query).bind(("key_id", key_id.to_string())).await?;

        let private_key: Option<String> = result.take(0)?;
        Ok(private_key)
    }

    /// Generate new server signing key with ed25519
    pub async fn generate_and_store_signing_key(
        &self,
        server_name: &str,
    ) -> Result<ServerSigningKeyRecord, RepositoryError> {
        use base64::{Engine as _, engine::general_purpose};
        use ed25519_dalek::{SigningKey, VerifyingKey};

        // Generate Ed25519 key pair
        let signing_key = SigningKey::from_bytes(&rand::random::<[u8; 32]>());
        let verifying_key: VerifyingKey = (&signing_key).into();

        // Create key ID
        let key_id =
            format!("ed25519:{}", general_purpose::STANDARD.encode(&verifying_key.to_bytes()[..8]));

        // Encode keys
        let private_key = general_purpose::STANDARD.encode(signing_key.to_bytes());
        let public_key = general_purpose::STANDARD.encode(verifying_key.to_bytes());

        let now = chrono::Utc::now();
        let expires_at = now + chrono::Duration::days(365); // 1 year validity

        let signing_key_record = ServerSigningKeyRecord {
            key_id: key_id.clone(),
            server_name: server_name.to_string(),
            private_key,
            public_key,
            created_at: now,
            expires_at: Some(expires_at),
            is_active: true,
        };

        // Store in database
        self.create_server_signing_key_record(&signing_key_record).await?;

        Ok(signing_key_record)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSigningKeyRecord {
    pub key_id: String,
    pub server_name: String,
    pub private_key: String,
    pub public_key: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_active: bool,
}
