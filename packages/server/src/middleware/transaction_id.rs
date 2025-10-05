//! Module contains intentional library code not yet fully integrated
#![allow(dead_code)]

use crate::{auth::MatrixAuth, error::MatrixError};
use axum::{
    Json,
    body::Body,
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use matryx_surrealdb::repository::InfrastructureService;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use surrealdb::{Surreal, engine::any::Any};

/// Transaction record stored in database for idempotency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRecord {
    pub transaction_id: String,
    pub user_id: String,
    pub response_body: serde_json::Value,
    pub status_code: u16,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Result of transaction check
#[derive(Debug)]
pub enum TransactionResult {
    NewTransaction,
    AlreadyProcessed(serde_json::Value),
}

/// Service for handling transaction ID validation and idempotency using repository pattern
pub struct TransactionService {
    infrastructure_service: InfrastructureService<Any>,
}

impl TransactionService {
    pub fn new(db: Surreal<Any>) -> Self {
        // Create InfrastructureService with all required repositories
        let websocket_repo = matryx_surrealdb::repository::WebSocketRepository::new(db.clone());
        let transaction_repo = matryx_surrealdb::repository::TransactionRepository::new(db.clone());
        let key_server_repo = matryx_surrealdb::repository::KeyServerRepository::new(db.clone());
        let registration_repo =
            matryx_surrealdb::repository::RegistrationRepository::new(db.clone());
        let directory_repo = matryx_surrealdb::repository::DirectoryRepository::new(db.clone());
        let device_repo = matryx_surrealdb::repository::DeviceRepository::new(db.clone());
        let auth_repo = matryx_surrealdb::repository::AuthRepository::new(db);

        let infrastructure_service = InfrastructureService::new(
            websocket_repo,
            transaction_repo,
            key_server_repo,
            registration_repo,
            directory_repo,
            device_repo,
            auth_repo,
        );

        Self { infrastructure_service }
    }

    /// Check if transaction has already been processed using InfrastructureService
    pub async fn check_transaction(
        &self,
        txn_id: &str,
        user_id: &str,
    ) -> Result<TransactionResult, MatrixError> {
        let endpoint = "transaction_middleware"; // Static endpoint identifier for middleware

        match self
            .infrastructure_service
            .handle_transaction_deduplication(user_id, txn_id, endpoint)
            .await
        {
            Ok(Some(cached_result)) => Ok(TransactionResult::AlreadyProcessed(cached_result)),
            Ok(None) => Ok(TransactionResult::NewTransaction),
            Err(_) => Err(MatrixError::Unknown),
        }
    }

    /// Store transaction result for future idempotency checks using InfrastructureService
    pub async fn store_transaction(
        &self,
        txn_id: &str,
        user_id: &str,
        response_body: serde_json::Value,
        status_code: u16,
    ) -> Result<(), MatrixError> {
        let endpoint = "transaction_middleware";

        // Create response object with status code
        let response_with_status = serde_json::json!({
            "body": response_body,
            "status_code": status_code
        });

        match self
            .infrastructure_service
            .store_transaction_result(user_id, txn_id, endpoint, response_with_status)
            .await
        {
            Ok(()) => Ok(()),
            Err(_) => Err(MatrixError::Unknown),
        }
    }

    /// Clean up old transactions using TransactionRepository through InfrastructureService
    pub async fn cleanup_old_transactions(&self, older_than_days: i64) -> Result<(), MatrixError> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(older_than_days);

        match self.infrastructure_service.cleanup_expired_transactions(cutoff).await {
            Ok(_) => Ok(()),
            Err(_) => Err(MatrixError::Unknown),
        }
    }
}

/// Extract transaction ID from request path using simple string parsing
fn extract_transaction_id_from_path(path: &str) -> Option<String> {
    // Matrix transaction IDs appear in paths like:
    // /_matrix/client/v3/rooms/{roomId}/send/{eventType}/{txnId}
    // /_matrix/client/v3/sendToDevice/{eventType}/{txnId}
    // /_matrix/client/v3/rooms/{roomId}/redact/{eventId}/{txnId}

    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    // Look for transaction ID patterns
    for (i, segment) in segments.iter().enumerate() {
        match *segment {
            "send" if i + 2 < segments.len() => {
                // Pattern: .../send/{eventType}/{txnId}
                let txn_id = segments[i + 2];
                if !txn_id.is_empty() && !txn_id.contains('?') {
                    return Some(txn_id.to_string());
                }
            },
            "sendToDevice" if i + 2 < segments.len() => {
                // Pattern: .../sendToDevice/{eventType}/{txnId}
                let txn_id = segments[i + 2];
                if !txn_id.is_empty() && !txn_id.contains('?') {
                    return Some(txn_id.to_string());
                }
            },
            "redact" if i + 2 < segments.len() => {
                // Pattern: .../redact/{eventId}/{txnId}
                let txn_id = segments[i + 2];
                if !txn_id.is_empty() && !txn_id.contains('?') {
                    return Some(txn_id.to_string());
                }
            },
            _ => continue,
        }
    }

    None
}

/// Convert response to storable JSON value
async fn response_to_json(response: &mut Response) -> Result<serde_json::Value, MatrixError> {
    let body = std::mem::replace(response.body_mut(), Body::empty());

    match axum::body::to_bytes(body, usize::MAX).await {
        Ok(bytes) => {
            let bytes_clone = bytes.clone();
            match serde_json::from_slice::<serde_json::Value>(&bytes_clone) {
                Ok(json) => {
                    // Restore the body for the response
                    *response.body_mut() = Body::from(bytes);
                    Ok(json)
                },
                Err(_) => {
                    // If not JSON, store as string
                    let text = String::from_utf8_lossy(&bytes_clone);
                    *response.body_mut() = Body::from(bytes);
                    Ok(serde_json::Value::String(text.to_string()))
                },
            }
        },
        Err(_) => Err(MatrixError::Unknown),
    }
}

/// Functional transaction ID validation middleware with full idempotency support using repository pattern
pub async fn transaction_id_middleware(
    State(transaction_service): State<Arc<TransactionService>>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();

    // Extract transaction ID from request path
    let txn_id = extract_transaction_id_from_path(&path);

    // Extract user ID from authentication context
    let user_id = request.extensions().get::<MatrixAuth>().and_then(|auth| {
        if let MatrixAuth::User(user_token) = auth {
            Some(user_token.user_id.clone())
        } else {
            None
        }
    });

    // If we have both transaction ID and user ID, check for idempotency
    if let (Some(txn_id), Some(user_id)) = (txn_id, user_id) {
        match transaction_service.check_transaction(&txn_id, &user_id).await {
            Ok(TransactionResult::AlreadyProcessed(cached_response)) => {
                // Extract the original response body from cached data
                let response_body = if let Some(body) = cached_response.get("body") {
                    body.clone()
                } else {
                    cached_response
                };

                // Return the stored response for idempotency
                return Json(response_body).into_response();
            },
            Ok(TransactionResult::NewTransaction) => {
                // Process the request and store the result
                let mut response = next.run(request).await;

                // Only store successful responses for idempotency
                if response.status().is_success()
                    && let Ok(response_json) = response_to_json(&mut response).await
                {
                    // Store the transaction result (ignore errors - idempotency is best effort)
                    let _ = transaction_service
                        .store_transaction(
                            &txn_id,
                            &user_id,
                            response_json,
                            response.status().as_u16(),
                        )
                        .await;
                }

                return response;
            },
            Err(_matrix_error) => {
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            },
        }
    }

    // If no transaction ID found or no authentication, just pass through
    next.run(request).await
}

/// Configuration for transaction ID handling
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TransactionConfig {
    pub cleanup_interval_hours: u64,
    pub retention_days: i64,
    pub enabled: bool,
}

impl TransactionConfig {
    pub fn from_env() -> Self {
        Self {
            cleanup_interval_hours: std::env::var("TRANSACTION_CLEANUP_INTERVAL_HOURS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(24),
            retention_days: std::env::var("TRANSACTION_RETENTION_DAYS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(7),
            enabled: std::env::var("TRANSACTION_ID_VALIDATION_ENABLED")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(true),
        }
    }
}
