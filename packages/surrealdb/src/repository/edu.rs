use crate::repository::error::RepositoryError;
use futures::Stream;
use matryx_entity::types::EDU;
use std::pin::Pin;
use surrealdb::{Connection, Surreal};

pub struct EDURepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> EDURepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    pub async fn create(&self, edu: &EDU) -> Result<EDU, RepositoryError> {
        let edu_clone = edu.clone();
        let created: Option<EDU> = self.db.create("edu").content(edu_clone).await?;

        created
            .ok_or_else(|| RepositoryError::Database(surrealdb::Error::msg("Failed to create EDU")))
    }

    pub async fn get_by_type(&self, edu_type: &str) -> Result<Vec<EDU>, RepositoryError> {
        let edu_type_owned = edu_type.to_string();
        let edus: Vec<EDU> = self
            .db
            .query("SELECT * FROM edu WHERE edu_type = $edu_type ORDER BY created_at DESC")
            .bind(("edu_type", edu_type_owned))
            .await?
            .take(0)?;
        Ok(edus)
    }

    pub async fn get_by_origin(&self, origin: &str) -> Result<Vec<EDU>, RepositoryError> {
        let origin_owned = origin.to_string();
        let edus: Vec<EDU> = self
            .db
            .query("SELECT * FROM edu WHERE origin = $origin ORDER BY created_at DESC")
            .bind(("origin", origin_owned))
            .await?
            .take(0)?;
        Ok(edus)
    }

    pub async fn get_by_destination(&self, destination: &str) -> Result<Vec<EDU>, RepositoryError> {
        let destination_owned = destination.to_string();
        let edus: Vec<EDU> = self
            .db
            .query("SELECT * FROM edu WHERE destination = $destination ORDER BY created_at DESC")
            .bind(("destination", destination_owned))
            .await?
            .take(0)?;
        Ok(edus)
    }

    pub fn subscribe(
        &self,
    ) -> Pin<Box<dyn Stream<Item = Result<EDU, RepositoryError>> + Send + '_>> {
        let query = "LIVE SELECT * FROM edu";
        let stream = self.db.query(query);

        Box::pin(async_stream::stream! {
            match stream.await {
                Ok(mut response) => {
                    match response.take::<Vec<EDU>>(0) {
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
