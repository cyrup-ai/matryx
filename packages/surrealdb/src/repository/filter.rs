use crate::repository::error::RepositoryError;
use futures::Stream;
use matryx_entity::types::Filter;
use std::pin::Pin;
use surrealdb::{Connection, Surreal};

pub struct FilterRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> FilterRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    pub async fn create(
        &self,
        filter: &Filter,
        filter_id: &str,
    ) -> Result<Filter, RepositoryError> {
        let filter_clone = filter.clone();
        let created: Option<Filter> =
            self.db.create(("filter", filter_id)).content(filter_clone).await?;

        created.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to create filter"))
        })
    }

    pub async fn get_by_id(&self, filter_id: &str) -> Result<Option<Filter>, RepositoryError> {
        let filter: Option<Filter> = self.db.select(("filter", filter_id)).await?;
        Ok(filter)
    }

    pub async fn get_user_filters(&self, user_id: &str) -> Result<Vec<Filter>, RepositoryError> {
        let user_id_owned = user_id.to_string();
        let filters: Vec<Filter> = self
            .db
            .query("SELECT * FROM filter WHERE user_id = $user_id")
            .bind(("user_id", user_id_owned))
            .await?
            .take(0)?;
        Ok(filters)
    }

    pub async fn delete(&self, filter_id: &str) -> Result<(), RepositoryError> {
        let _: Option<Filter> = self.db.delete(("filter", filter_id)).await?;
        Ok(())
    }

    pub fn subscribe_user(
        &self,
        user_id: String,
    ) -> Pin<Box<dyn Stream<Item = Result<Filter, RepositoryError>> + Send + '_>> {
        let query = format!("LIVE SELECT * FROM filter WHERE user_id = '{}'", user_id);
        let stream = self.db.query(query);

        Box::pin(async_stream::stream! {
            match stream.await {
                Ok(mut response) => {
                    match response.take::<Vec<Filter>>(0) {
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
