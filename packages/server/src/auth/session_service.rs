use anyhow::Result;
use base64::{Engine, engine::general_purpose};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use tracing::{debug, info, warn};

use crate::auth::{
    MatrixAccessToken,
    MatrixAuth,
    MatrixAuthError,
    MatrixJwtClaims,
    MatrixServerAuth,
};

/// Service for managing Matrix authentication sessions with SurrealDB 3.0
#[derive(Clone)]
pub struct MatrixSessionService {
    jwt_secret: Vec<u8>,
    homeserver_name: String,
    db: Option<Surreal<Any>>,
}

impl MatrixSessionService {
    /// Create new session service
    pub fn new(jwt_secret: Vec<u8>, homeserver_name: String) -> Self {
        Self { jwt_secret, homeserver_name, db: None }
    }

    /// Create new session service with database connection
    pub fn with_db(jwt_secret: Vec<u8>, homeserver_name: String, db: Surreal<Any>) -> Self {
        Self { jwt_secret, homeserver_name, db: Some(db) }
    }

    /// Get the homeserver name
    pub fn get_homeserver_name(&self) -> &str {
        &self.homeserver_name
    }

    /// Create JWT token for Matrix user authentication
    pub fn create_user_token(
        &self,
        user_id: &str,
        device_id: &str,
        access_token: &str,
        refresh_token: Option<&str>,
        expires_in: i64,
    ) -> Result<String, MatrixAuthError> {
        let claims =
            MatrixJwtClaims::for_user(user_id, device_id, access_token, refresh_token, expires_in);

        let header = Header::default();
        encode(&header, &claims, &EncodingKey::from_secret(&self.jwt_secret))
            .map_err(|e| MatrixAuthError::JwtError(e.to_string()))
    }

    /// Create JWT token for Matrix server authentication
    pub fn create_server_token(
        &self,
        server_name: &str,
        key_id: &str,
        expires_in: i64,
    ) -> Result<String, MatrixAuthError> {
        let claims = MatrixJwtClaims::for_server(server_name, key_id, expires_in);

        let header = Header::default();
        encode(&header, &claims, &EncodingKey::from_secret(&self.jwt_secret))
            .map_err(|e| MatrixAuthError::JwtError(e.to_string()))
    }

    /// Validate and decode JWT token
    pub fn validate_token(&self, token: &str) -> Result<MatrixJwtClaims, MatrixAuthError> {
        let mut validation = Validation::default();
        validation.validate_exp = true;
        validation.validate_nbf = true;

        decode::<MatrixJwtClaims>(token, &DecodingKey::from_secret(&self.jwt_secret), &validation)
            .map(|data| data.claims)
            .map_err(|e| MatrixAuthError::JwtError(e.to_string()))
    }

    /// Create SurrealDB session from Matrix access token
    pub async fn create_user_session(
        &self,
        user_id: &str,
        device_id: &str,
        access_token: &str,
        refresh_token: Option<&str>,
    ) -> Result<MatrixAccessToken, MatrixAuthError> {
        // Create JWT claims for the user
        let expires_in = 3600; // 1 hour
        let claims =
            MatrixJwtClaims::for_user(user_id, device_id, access_token, refresh_token, expires_in);

        // Convert to Matrix authentication context
        let matrix_auth = claims.to_matrix_auth()?;

        match matrix_auth {
            MatrixAuth::User(access_token_info) => Ok(access_token_info),
            _ => Err(MatrixAuthError::InvalidXMatrixFormat),
        }
    }

    /// Create SurrealDB session from Matrix server authentication
    pub async fn create_server_session(
        &self,
        server_name: &str,
        key_id: &str,
        signature: &str,
    ) -> Result<MatrixServerAuth, MatrixAuthError> {
        // Create JWT claims for the server
        let expires_in = 300; // 5 minutes for federation
        let claims = MatrixJwtClaims::for_server(server_name, key_id, expires_in);

        // Convert to Matrix authentication context
        let matrix_auth = claims.to_matrix_auth()?;

        match matrix_auth {
            MatrixAuth::Server(server_auth) => Ok(server_auth),
            _ => Err(MatrixAuthError::InvalidXMatrixFormat),
        }
    }

    /// Validate Matrix access token and return session
    pub async fn validate_access_token(
        &self,
        access_token: &str,
    ) -> Result<MatrixAccessToken, MatrixAuthError> {
        if access_token.starts_with("syt_") {
            // Matrix opaque token format - query database
            self.validate_opaque_token(access_token).await
        } else {
            // Try to decode as JWT
            let claims = self.validate_token(access_token)?;

            // Verify it's a user token
            let user_id = claims
                .matrix_user_id
                .clone()
                .ok_or(MatrixAuthError::InvalidXMatrixFormat)?;
            let device_id = claims
                .matrix_device_id
                .clone()
                .ok_or(MatrixAuthError::InvalidXMatrixFormat)?;

            // Check if session is expired
            if claims.exp.map_or(false, |exp| exp <= chrono::Utc::now().timestamp()) {
                return Err(MatrixAuthError::SessionExpired);
            }

            // Create Matrix authentication context from claims
            let matrix_auth = claims.to_matrix_auth()?;

            match matrix_auth {
                MatrixAuth::User(access_token_info) => Ok(access_token_info),
                _ => Err(MatrixAuthError::InvalidXMatrixFormat),
            }
        }
    }

    /// Validate opaque Matrix access token against database
    async fn validate_opaque_token(
        &self,
        access_token: &str,
    ) -> Result<MatrixAccessToken, MatrixAuthError> {
        let db = self.db.as_ref().ok_or_else(|| {
            MatrixAuthError::DatabaseError("Database connection not available".to_string())
        })?;

        // Query the access_tokens table
        let query = "
            SELECT user_id, device_id, expires_at
            FROM user_access_tokens
            WHERE token = $token AND (expires_at IS NULL OR expires_at > datetime::now())
        ";

        let mut response = db
            .query(query)
            .bind(("token", access_token.to_string()))
            .await
            .map_err(|e| MatrixAuthError::DatabaseError(format!("Token query failed: {}", e)))?;

        #[derive(serde::Deserialize)]
        struct TokenRecord {
            user_id: String,
            device_id: String,
            expires_at: Option<chrono::DateTime<chrono::Utc>>,
        }

        let token_record: Option<TokenRecord> = response.take(0).map_err(|e| {
            MatrixAuthError::DatabaseError(format!("Failed to parse token record: {}", e))
        })?;

        match token_record {
            Some(record) => {
                Ok(MatrixAccessToken {
                    token: access_token.to_string(),
                    user_id: record.user_id,
                    device_id: record.device_id,
                    expires_at: record.expires_at.map(|dt| dt.timestamp()),
                })
            },
            None => Err(MatrixAuthError::UnknownToken),
        }
    }

    /// Validate Matrix X-Matrix server signature
    pub async fn validate_server_signature(
        &self,
        server_name: &str,
        key_id: &str,
        signature: &str,
        request_method: &str,
        request_uri: &str,
        request_body: &[u8],
    ) -> Result<MatrixServerAuth, MatrixAuthError> {
        // 1. Fetch the server's public key from key server or cache
        let public_key = self.get_server_public_key(server_name, key_id).await?;

        // 2. Construct canonical JSON for signature verification
        let canonical_json = self.build_canonical_json(
            request_method,
            request_uri,
            server_name,
            &self.homeserver_name,
            request_body,
        )?;

        // 3. Verify the ed25519 signature
        self.verify_ed25519_signature(&signature, &canonical_json, &public_key)?;

        // 4. Create validated server authentication
        Ok(MatrixServerAuth {
            server_name: server_name.to_string(),
            key_id: key_id.to_string(),
            signature: signature.to_string(),
            expires_at: Some((chrono::Utc::now() + chrono::Duration::minutes(5)).timestamp()), // 5 minute validity
        })
    }

    /// Fetch server's public key for signature verification
    pub async fn get_server_public_key(
        &self,
        server_name: &str,
        key_id: &str,
    ) -> Result<String, MatrixAuthError> {
        let db = self.db.as_ref().ok_or_else(|| {
            MatrixAuthError::DatabaseError("Database connection not available".to_string())
        })?;

        // Try cache first
        let query = "
            SELECT public_key, expires_at
            FROM server_signing_keys
            WHERE server_name = $server_name 
              AND key_id = $key_id 
              AND (expires_at IS NULL OR expires_at > datetime::now())
        ";

        let mut response = db
            .query(query)
            .bind(("server_name", server_name.to_string()))
            .bind(("key_id", key_id.to_string()))
            .await
            .map_err(|e| MatrixAuthError::DatabaseError(format!("Key query failed: {}", e)))?;

        #[derive(serde::Deserialize)]
        struct KeyRecord {
            public_key: String,
            expires_at: Option<chrono::DateTime<chrono::Utc>>,
        }

        let key_record: Option<KeyRecord> = response.take(0).map_err(|e| {
            MatrixAuthError::DatabaseError(format!("Failed to parse key record: {}", e))
        })?;

        match key_record {
            Some(record) => Ok(record.public_key),
            None => {
                // Key not in cache - fetch from remote server
                info!("Fetching server key {}:{} from remote server", server_name, key_id);
                let fetched_key = self.fetch_remote_server_key(server_name, key_id).await?;

                // Cache the fetched key for future use
                self.cache_server_key(server_name, key_id, &fetched_key).await?;

                debug!("Successfully fetched and cached server key {}:{}", server_name, key_id);
                Ok(fetched_key)
            },
        }
    }

    /// Build canonical JSON for Matrix signature verification
    fn build_canonical_json(
        &self,
        method: &str,
        uri: &str,
        origin: &str,
        destination: &str,
        content: &[u8],
    ) -> Result<String, MatrixAuthError> {
        use serde_json::{Map, Value, json};

        let mut canonical_request = Map::new();
        canonical_request.insert("method".to_string(), Value::String(method.to_uppercase()));
        canonical_request.insert("uri".to_string(), Value::String(uri.to_string()));
        canonical_request.insert("origin".to_string(), Value::String(origin.to_string()));
        canonical_request.insert("destination".to_string(), Value::String(destination.to_string()));

        // Add content if present
        if !content.is_empty() {
            let content_json: Value =
                serde_json::from_slice(content).map_err(|_| MatrixAuthError::InvalidSignature)?;
            canonical_request.insert("content".to_string(), content_json);
        }

        // Convert to canonical JSON (sorted keys, no whitespace)
        let canonical_json = self.to_canonical_json(&Value::Object(canonical_request))?;

        Ok(canonical_json)
    }

    /// Verify ed25519 signature against canonical JSON
    pub fn verify_ed25519_signature(
        &self,
        signature: &str,
        canonical_json: &str,
        public_key: &str,
    ) -> Result<(), MatrixAuthError> {
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};

        // Decode base64 signature
        let signature_bytes = general_purpose::STANDARD
            .decode(signature)
            .map_err(|_| MatrixAuthError::InvalidSignature)?;

        // Decode base64 public key
        let public_key_bytes = general_purpose::STANDARD
            .decode(public_key)
            .map_err(|_| MatrixAuthError::InvalidSignature)?;

        // Validate signature and public key byte lengths
        if signature_bytes.len() != 64 {
            return Err(MatrixAuthError::InvalidSignature);
        }
        if public_key_bytes.len() != 32 {
            return Err(MatrixAuthError::InvalidSignature);
        }

        // Create Ed25519 public key
        let public_key_array: [u8; 32] = public_key_bytes
            .try_into()
            .map_err(|_| MatrixAuthError::InvalidSignature)?;
        let verifying_key = VerifyingKey::from_bytes(&public_key_array)
            .map_err(|_| MatrixAuthError::InvalidSignature)?;

        // Create Ed25519 signature
        let signature_array: [u8; 64] =
            signature_bytes.try_into().map_err(|_| MatrixAuthError::InvalidSignature)?;
        let signature = Signature::from_bytes(&signature_array);

        // Verify signature against canonical JSON
        verifying_key
            .verify(canonical_json.as_bytes(), &signature)
            .map_err(|_| MatrixAuthError::InvalidSignature)?;

        Ok(())
    }

    /// Refresh an expired access token using refresh token
    pub async fn refresh_token(
        &self,
        refresh_token: &str,
    ) -> Result<(String, String), MatrixAuthError> {
        // Validate refresh token
        let claims = self.validate_token(refresh_token)?;

        // Verify it contains refresh token data
        let user_id = claims.matrix_user_id.ok_or(MatrixAuthError::UnknownToken)?;
        let device_id = claims.matrix_device_id.ok_or(MatrixAuthError::UnknownToken)?;

        // Generate new tokens
        let new_access_token = format!("syt_{}", uuid::Uuid::new_v4());
        let new_refresh_token = format!("syr_{}", uuid::Uuid::new_v4());

        // Create new JWT with extended expiration
        let _new_jwt = self.create_user_token(
            &user_id,
            &device_id,
            &new_access_token,
            Some(&new_refresh_token),
            3600, // 1 hour
        )?;

        Ok((new_access_token, new_refresh_token))
    }

    /// Create access token for user login
    pub async fn create_access_token(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<String, MatrixAuthError> {
        let access_token = format!("syt_{}", uuid::Uuid::new_v4());

        // Create JWT token
        let _jwt_token = self.create_user_token(
            user_id,
            device_id,
            &access_token,
            None,
            3600, // 1 hour
        )?;

        Ok(access_token)
    }

    /// Fetch server public key from remote server's /_matrix/key/v2/server endpoint
    async fn fetch_remote_server_key(
        &self,
        server_name: &str,
        key_id: &str,
    ) -> Result<String, MatrixAuthError> {
        use reqwest::Client;
        use std::time::Duration;

        // Create HTTP client with appropriate timeouts
        let client = Client::builder().timeout(Duration::from_secs(30)).build().map_err(|e| {
            MatrixAuthError::DatabaseError(format!("Failed to create HTTP client: {}", e))
        })?;

        // Construct the server key URL
        let url = format!("https://{}/_matrix/key/v2/server", server_name);
        debug!("Fetching server keys from: {}", url);

        // Make HTTP request to fetch server keys
        let response = client
            .get(&url)
            .header("User-Agent", "matryx-homeserver/1.0")
            .send()
            .await
            .map_err(|e| {
                MatrixAuthError::DatabaseError(format!(
                    "Failed to fetch server keys from {}: {}",
                    server_name, e
                ))
            })?;

        if !response.status().is_success() {
            return Err(MatrixAuthError::DatabaseError(format!(
                "Server key request failed with status: {} for {}",
                response.status(),
                server_name
            )));
        }

        // Parse the JSON response
        let key_response: serde_json::Value = response.json().await.map_err(|e| {
            MatrixAuthError::DatabaseError(format!("Failed to parse server key response: {}", e))
        })?;

        // Verify the response is for the correct server
        let response_server_name =
            key_response.get("server_name").and_then(|v| v.as_str()).ok_or_else(|| {
                MatrixAuthError::DatabaseError(
                    "Server key response missing server_name".to_string(),
                )
            })?;

        if response_server_name != server_name {
            return Err(MatrixAuthError::DatabaseError(format!(
                "Server key response server name mismatch: expected {}, got {}",
                server_name, response_server_name
            )));
        }

        // Check if the key response has not expired
        let valid_until_ts =
            key_response.get("valid_until_ts").and_then(|v| v.as_i64()).unwrap_or(0);

        let current_time_ms = chrono::Utc::now().timestamp_millis();
        if valid_until_ts > 0 && current_time_ms > valid_until_ts {
            return Err(MatrixAuthError::DatabaseError(format!(
                "Server key response has expired for {}",
                server_name
            )));
        }

        // Extract the verify_keys object
        let verify_keys =
            key_response
                .get("verify_keys")
                .and_then(|v| v.as_object())
                .ok_or_else(|| {
                    MatrixAuthError::DatabaseError(
                        "Server key response missing verify_keys".to_string(),
                    )
                })?;

        // Find the requested key_id
        let key_data = verify_keys.get(key_id).and_then(|v| v.as_object()).ok_or_else(|| {
            MatrixAuthError::DatabaseError(format!(
                "Requested key {} not found in server response",
                key_id
            ))
        })?;

        let public_key = key_data.get("key").and_then(|v| v.as_str()).ok_or_else(|| {
            MatrixAuthError::DatabaseError(format!("Public key data missing for key {}", key_id))
        })?;

        // Verify the signatures on the key response
        self.verify_server_key_signatures(&key_response, server_name).await?;

        debug!("Successfully fetched server key {}:{}", server_name, key_id);
        Ok(public_key.to_string())
    }

    /// Verify signatures on the server key response
    async fn verify_server_key_signatures(
        &self,
        key_response: &serde_json::Value,
        server_name: &str,
    ) -> Result<(), MatrixAuthError> {
        let signatures =
            key_response.get("signatures").and_then(|v| v.as_object()).ok_or_else(|| {
                MatrixAuthError::DatabaseError("Server key response missing signatures".to_string())
            })?;

        let server_signatures =
            signatures.get(server_name).and_then(|v| v.as_object()).ok_or_else(|| {
                MatrixAuthError::DatabaseError(format!(
                    "Server key response missing signatures from {}",
                    server_name
                ))
            })?;

        let verify_keys =
            key_response
                .get("verify_keys")
                .and_then(|v| v.as_object())
                .ok_or_else(|| {
                    MatrixAuthError::DatabaseError(
                        "Server key response missing verify_keys for signature verification"
                            .to_string(),
                    )
                })?;

        // Create canonical JSON for signature verification (without signatures field)
        let mut key_for_signing = key_response.clone();
        if let Some(obj) = key_for_signing.as_object_mut() {
            obj.remove("signatures");
        }

        let canonical_json = self.to_canonical_json(&key_for_signing)?;

        // Verify at least one signature from the server
        let mut verified = false;
        for (signature_key_id, signature) in server_signatures {
            let signature_str = signature.as_str().ok_or_else(|| {
                MatrixAuthError::DatabaseError("Server key signature must be a string".to_string())
            })?;

            // Get the public key for this signature
            if let Some(key_data) = verify_keys.get(signature_key_id) {
                if let Some(public_key) = key_data.get("key").and_then(|k| k.as_str()) {
                    match self.verify_ed25519_signature(signature_str, &canonical_json, public_key)
                    {
                        Ok(_) => {
                            debug!(
                                "Verified server key signature from {} with key {}",
                                server_name, signature_key_id
                            );
                            verified = true;
                            break;
                        },
                        Err(e) => {
                            warn!(
                                "Failed to verify server key signature from {} with key {}: {:?}",
                                server_name, signature_key_id, e
                            );
                        },
                    }
                }
            }
        }

        if !verified {
            return Err(MatrixAuthError::DatabaseError(format!(
                "Failed to verify any server key signatures from {}",
                server_name
            )));
        }

        Ok(())
    }

    /// Cache server public key in database for future use
    async fn cache_server_key(
        &self,
        server_name: &str,
        key_id: &str,
        public_key: &str,
    ) -> Result<(), MatrixAuthError> {
        let db = self.db.as_ref().ok_or_else(|| {
            MatrixAuthError::DatabaseError("Database connection not available".to_string())
        })?;

        let query = "
            CREATE server_signing_keys SET
                server_name = $server_name,
                key_id = $key_id,
                public_key = $public_key,
                fetched_at = $fetched_at,
                is_active = true,
                expires_at = $expires_at
        ";

        let expires_at = chrono::Utc::now() + chrono::Duration::hours(24); // Cache for 24 hours

        let _response = db
            .query(query)
            .bind(("server_name", server_name.to_string()))
            .bind(("key_id", key_id.to_string()))
            .bind(("public_key", public_key.to_string()))
            .bind(("fetched_at", chrono::Utc::now()))
            .bind(("expires_at", expires_at))
            .await
            .map_err(|e| {
                MatrixAuthError::DatabaseError(format!("Failed to cache server key: {}", e))
            })?;

        debug!("Cached server key {}:{} (expires: {})", server_name, key_id, expires_at);
        Ok(())
    }

    /// Get our server's signing key for event signing
    pub async fn get_server_signing_key(
        &self,
        server_name: &str,
    ) -> Result<crate::auth::ServerSigningKey, MatrixAuthError> {
        let db = self.db.as_ref().ok_or_else(|| {
            MatrixAuthError::DatabaseError("Database connection not available".to_string())
        })?;

        // Try to get existing signing key
        let query = "
            SELECT key_id, server_name, private_key, public_key, created_at, expires_at, is_active
            FROM server_signing_keys
            WHERE server_name = $server_name 
              AND is_active = true
              AND (expires_at IS NULL OR expires_at > datetime::now())
            ORDER BY created_at DESC
            LIMIT 1
        ";

        let mut response = db
            .query(query)
            .bind(("server_name", server_name.to_string()))
            .await
            .map_err(|e| {
                MatrixAuthError::DatabaseError(format!("Failed to query server signing key: {}", e))
            })?;

        if let Ok(Some(key)) = response.take::<Option<crate::auth::ServerSigningKey>>(0) {
            return Ok(key);
        }

        // Generate new signing key if none exists
        self.generate_server_signing_key(server_name).await
    }

    /// Generate a new server signing key
    async fn generate_server_signing_key(
        &self,
        server_name: &str,
    ) -> Result<crate::auth::ServerSigningKey, MatrixAuthError> {
        use base64::{Engine as _, engine::general_purpose};
        use ed25519_dalek::{SigningKey, VerifyingKey};
        use rand::rngs::OsRng;

        let db = self.db.as_ref().ok_or_else(|| {
            MatrixAuthError::DatabaseError("Database connection not available".to_string())
        })?;

        // Generate Ed25519 key pair
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key: VerifyingKey = (&signing_key).into();

        // Create key ID
        let key_id =
            format!("ed25519:{}", general_purpose::STANDARD.encode(&verifying_key.to_bytes()[..8]));

        // Encode keys
        let private_key = general_purpose::STANDARD.encode(signing_key.to_bytes());
        let public_key = general_purpose::STANDARD.encode(verifying_key.to_bytes());

        let now = chrono::Utc::now();
        let expires_at = now + chrono::Duration::days(365); // 1 year validity

        let signing_key_record = crate::auth::ServerSigningKey {
            key_id: key_id.clone(),
            server_name: server_name.to_string(),
            private_key: private_key.clone(),
            public_key: public_key.clone(),
            created_at: now,
            expires_at: Some(expires_at),
            is_active: true,
        };

        // Store in database
        let cloned_record = signing_key_record.clone();
        let _: Option<crate::auth::ServerSigningKey> = db
            .create(("server_signing_keys", key_id.clone()))
            .content(cloned_record)
            .await
            .map_err(|e| {
                MatrixAuthError::DatabaseError(format!("Failed to store server signing key: {}", e))
            })?;

        info!("Generated new server signing key: {} for {}", key_id, server_name);
        Ok(signing_key_record)
    }

    /// Sign JSON content with server signing key
    pub async fn sign_json(
        &self,
        json_content: &str,
        key_id: &str,
    ) -> Result<String, MatrixAuthError> {
        use base64::{Engine as _, engine::general_purpose};
        use ed25519_dalek::{Signature, Signer, SigningKey};

        let db = self.db.as_ref().ok_or_else(|| {
            MatrixAuthError::DatabaseError("Database connection not available".to_string())
        })?;

        // Get the signing key from database
        let query = "
            SELECT private_key
            FROM server_signing_keys
            WHERE key_id = $key_id 
              AND is_active = true
              AND (expires_at IS NULL OR expires_at > datetime::now())
            LIMIT 1
        ";

        let mut response =
            db.query(query).bind(("key_id", key_id.to_string())).await.map_err(|e| {
                MatrixAuthError::DatabaseError(format!("Failed to query signing key: {}", e))
            })?;

        #[derive(serde::Deserialize)]
        struct SigningKeyData {
            private_key: String,
        }

        let key_data: Option<SigningKeyData> = response.take(0).map_err(|e| {
            MatrixAuthError::DatabaseError(format!("Failed to extract signing key: {}", e))
        })?;

        let key_data = key_data.ok_or_else(|| {
            MatrixAuthError::DatabaseError(format!("Signing key {} not found or expired", key_id))
        })?;

        // Decode private key
        let private_key_bytes =
            general_purpose::STANDARD.decode(&key_data.private_key).map_err(|e| {
                MatrixAuthError::DatabaseError(format!("Failed to decode private key: {}", e))
            })?;

        let key_array: [u8; 32] = private_key_bytes.try_into().map_err(|_| {
            MatrixAuthError::DatabaseError("Invalid private key length".to_string())
        })?;
        let signing_key = SigningKey::from_bytes(&key_array);

        // Sign the JSON content
        let signature: Signature = signing_key.sign(json_content.as_bytes());
        let signature_b64 = general_purpose::STANDARD.encode(signature.to_bytes());

        Ok(signature_b64)
    }

    /// Convert JSON value to Matrix canonical JSON string with sorted keys
    ///
    /// Implements Matrix canonical JSON as defined in the Matrix specification:
    /// - Object keys sorted in lexicographic order
    /// - No insignificant whitespace
    /// - UTF-8 encoding
    /// - Numbers in shortest form
    ///
    /// This is critical for signature verification to work correctly with other Matrix homeservers.
    fn to_canonical_json(&self, value: &serde_json::Value) -> Result<String, MatrixAuthError> {
        match value {
            serde_json::Value::Null => Ok("null".to_string()),
            serde_json::Value::Bool(b) => Ok(b.to_string()),
            serde_json::Value::Number(n) => Ok(n.to_string()),
            serde_json::Value::String(s) => {
                // JSON string with proper escaping
                Ok(serde_json::to_string(s).map_err(|_| MatrixAuthError::InvalidSignature)?)
            },
            serde_json::Value::Array(arr) => {
                let elements: Result<Vec<String>, MatrixAuthError> =
                    arr.iter().map(|v| self.to_canonical_json(v)).collect();
                Ok(format!("[{}]", elements?.join(",")))
            },
            serde_json::Value::Object(obj) => {
                // Sort keys lexicographically (critical for Matrix signature verification)
                let mut sorted_keys: Vec<&String> = obj.keys().collect();
                sorted_keys.sort();

                let pairs: Result<Vec<String>, MatrixAuthError> = sorted_keys
                    .into_iter()
                    .map(|key| {
                        let key_json = serde_json::to_string(key)
                            .map_err(|_| MatrixAuthError::InvalidSignature)?;
                        let value_json = self.to_canonical_json(&obj[key])?;
                        Ok(format!("{}:{}", key_json, value_json))
                    })
                    .collect();

                Ok(format!("{{{}}}", pairs?.join(",")))
            },
        }
    }
}
