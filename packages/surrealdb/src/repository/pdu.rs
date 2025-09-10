use crate::repository::error::RepositoryError;
use futures::Stream;
use matryx_entity::types::PDU;
use std::pin::Pin;
use surrealdb::{Connection, Surreal};

pub struct PDURepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> PDURepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    pub async fn create(&self, pdu: &PDU) -> Result<PDU, RepositoryError> {
        let pdu_clone = pdu.clone();
        let created: Option<PDU> =
            self.db.create(("pdu", &pdu.event_id)).content(pdu_clone).await?;

        created
            .ok_or_else(|| RepositoryError::Database(surrealdb::Error::msg("Failed to create PDU")))
    }

    pub async fn get_by_event_id(&self, event_id: &str) -> Result<Option<PDU>, RepositoryError> {
        let pdu: Option<PDU> = self.db.select(("pdu", event_id)).await?;
        Ok(pdu)
    }

    pub async fn get_room_pdus(&self, room_id: &str) -> Result<Vec<PDU>, RepositoryError> {
        let room_id_owned = room_id.to_string();
        let pdus: Vec<PDU> = self
            .db
            .query("SELECT * FROM pdu WHERE room_id = $room_id ORDER BY origin_server_ts DESC")
            .bind(("room_id", room_id_owned))
            .await?
            .take(0)?;
        Ok(pdus)
    }

    pub async fn get_by_origin(&self, origin: &str) -> Result<Vec<PDU>, RepositoryError> {
        let origin_owned = origin.to_string();
        let pdus: Vec<PDU> = self
            .db
            .query("SELECT * FROM pdu WHERE origin = $origin ORDER BY origin_server_ts DESC")
            .bind(("origin", origin_owned))
            .await?
            .take(0)?;
        Ok(pdus)
    }

    pub fn subscribe(
        &self,
    ) -> Pin<Box<dyn Stream<Item = Result<PDU, RepositoryError>> + Send + '_>> {
        let query = "LIVE SELECT * FROM pdu";
        let stream = self.db.query(query);

        Box::pin(async_stream::stream! {
            match stream.await {
                Ok(mut response) => {
                    match response.take::<Vec<PDU>>(0) {
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
