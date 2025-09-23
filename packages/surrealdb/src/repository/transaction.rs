use crate::repository::error::RepositoryError;
use chrono::{DateTime, Utc};
use futures::Stream;
use matryx_entity::types::Transaction;
use serde_json::Value;
use std::pin::Pin;
use surrealdb::{Connection, Surreal};

pub struct TransactionRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> TransactionRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    pub async fn create(&self, transaction: &Transaction) -> Result<Transaction, RepositoryError> {
        let transaction_clone = transaction.clone();
        // Use combination of origin and timestamp as unique identifier
        let record_id = format!("{}_{}", transaction.origin, transaction.origin_server_ts);
        let created: Option<Transaction> = self
            .db
            .create(("transaction", record_id))
            .content(transaction_clone)
            .await?;

        created.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to create transaction"))
        })
    }

    pub async fn get_by_origin_and_timestamp(
        &self,
        origin: &str,
        origin_server_ts: i64,
    ) -> Result<Option<Transaction>, RepositoryError> {
        let record_id = format!("{}_{}", origin, origin_server_ts);
        let transaction: Option<Transaction> = self.db.select(("transaction", record_id)).await?;
        Ok(transaction)
    }

    pub async fn get_by_origin_destination(
        &self,
        origin: &str,
        destination: &str,
    ) -> Result<Vec<Transaction>, RepositoryError> {
        let transactions: Vec<Transaction> = self.db
            .query("SELECT * FROM transaction WHERE origin = $origin AND destination = $destination ORDER BY created_at DESC")
            .bind(("origin", origin.to_string()))
            .bind(("destination", destination.to_string()))
            .await?
            .take(0)?;
        Ok(transactions)
    }

    pub fn subscribe(
        &self,
    ) -> Pin<Box<dyn Stream<Item = Result<Transaction, RepositoryError>> + Send + '_>> {
        let query = "LIVE SELECT * FROM transaction";
        let stream = self.db.query(query);

        Box::pin(async_stream::stream! {
            match stream.await {
                Ok(mut response) => {
                    match response.take::<Vec<Transaction>>(0) {
                        Ok(data) => {
                            for item in data {
                                yield Ok(item);
                            }
                        }
                        Err(e) => yield Err(RepositoryError::Database(e)),
                    }
                }
                Err(e) => yield Err(RepositoryError::Database(e)),
            }
        })
    }

    /// Get cached result for a federation transaction
    pub async fn get_cached_result(
        &self,
        transaction_key: &str,
    ) -> Result<Option<Value>, RepositoryError> {
        let result: Option<Value> = self
            .db
            .query("SELECT result FROM federation_transactions WHERE transaction_key = $transaction_key AND expires_at > $now LIMIT 1")
            .bind(("transaction_key", transaction_key.to_string()))
            .bind(("now", Utc::now()))
            .await?
            .take("result")?;

        Ok(result)
    }

    /// Cache the result of a processed federation transaction
    pub async fn cache_result(
        &self,
        transaction_key: &str,
        result: Value,
    ) -> Result<(), RepositoryError> {
        let now = Utc::now();
        let expires_at = now + chrono::Duration::hours(24);

        self.db
            .query("CREATE federation_transactions SET transaction_key = $transaction_key, result = $result, created_at = $created_at, expires_at = $expires_at")
            .bind(("transaction_key", transaction_key.to_string()))
            .bind(("result", result))
            .bind(("created_at", now))
            .bind(("expires_at", expires_at))
            .await?;

        Ok(())
    }

    /// Clean up expired transaction cache entries
    pub async fn cleanup_expired_cache(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        self.db
            .query("DELETE FROM federation_transactions WHERE expires_at < $cutoff")
            .bind(("cutoff", cutoff))
            .await?;

        Ok(())
    }

    // Middleware transaction deduplication methods

    /// Check if a transaction with the given parameters already exists (duplicate detection)
    pub async fn check_transaction_duplicate(
        &self,
        user_id: &str,
        txn_id: &str,
        endpoint: &str,
    ) -> Result<bool, RepositoryError> {
        let result: Option<Value> = self
            .db
            .query("SELECT VALUE count() FROM transaction_dedupe WHERE user_id = $user_id AND txn_id = $txn_id AND endpoint = $endpoint")
            .bind(("user_id", user_id.to_string()))
            .bind(("txn_id", txn_id.to_string()))
            .bind(("endpoint", endpoint.to_string()))
            .await?
            .take(0)?;

        match result {
            Some(Value::Number(n)) => Ok(n.as_u64().unwrap_or(0) > 0),
            _ => Ok(false),
        }
    }

    /// Store the result of a transaction for deduplication
    pub async fn store_transaction_result(
        &self,
        user_id: &str,
        txn_id: &str,
        endpoint: &str,
        result: Value,
    ) -> Result<(), RepositoryError> {
        let now = Utc::now();
        let expires_at = now + chrono::Duration::hours(1); // Expire after 1 hour

        self.db
            .query("CREATE transaction_dedupe SET user_id = $user_id, txn_id = $txn_id, endpoint = $endpoint, result = $result, created_at = $created_at, expires_at = $expires_at")
            .bind(("user_id", user_id.to_string()))
            .bind(("txn_id", txn_id.to_string()))
            .bind(("endpoint", endpoint.to_string()))
            .bind(("result", result))
            .bind(("created_at", now))
            .bind(("expires_at", expires_at))
            .await?;

        Ok(())
    }

    /// Get the cached result of a previous transaction
    pub async fn get_transaction_result(
        &self,
        user_id: &str,
        txn_id: &str,
        endpoint: &str,
    ) -> Result<Option<Value>, RepositoryError> {
        let result: Option<Value> = self
            .db
            .query("SELECT VALUE result FROM transaction_dedupe WHERE user_id = $user_id AND txn_id = $txn_id AND endpoint = $endpoint AND expires_at > $now LIMIT 1")
            .bind(("user_id", user_id.to_string()))
            .bind(("txn_id", txn_id.to_string()))
            .bind(("endpoint", endpoint.to_string()))
            .bind(("now", Utc::now()))
            .await?
            .take(0)?;

        Ok(result)
    }

    /// Clean up expired transaction deduplication entries
    pub async fn cleanup_expired_transactions(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<u64, RepositoryError> {
        let result: Vec<Value> = self
            .db
            .query("DELETE FROM transaction_dedupe WHERE expires_at < $cutoff RETURN BEFORE")
            .bind(("cutoff", cutoff))
            .await?
            .take(0)?;

        Ok(result.len() as u64)
    }

    /// Validate transaction ID format (should be UUID or similar format)
    pub async fn validate_transaction_format(&self, txn_id: &str) -> Result<bool, RepositoryError> {
        // Basic validation - should be alphanumeric with hyphens, 8-64 characters
        let is_valid = txn_id.len() >= 8 &&
            txn_id.len() <= 64 &&
            txn_id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_');

        Ok(is_valid)
    }
}
