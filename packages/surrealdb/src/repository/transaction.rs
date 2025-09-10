use crate::repository::error::RepositoryError;
use futures::Stream;
use matryx_entity::types::Transaction;
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
}
