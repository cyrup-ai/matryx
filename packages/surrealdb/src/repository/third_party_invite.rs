use crate::repository::error::RepositoryError;
use futures::Stream;
use matryx_entity::types::ThirdPartyInvite;
use std::pin::Pin;
use surrealdb::{Connection, Surreal};

pub struct ThirdPartyInviteRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> ThirdPartyInviteRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    pub async fn create(
        &self,
        invite: &ThirdPartyInvite,
    ) -> Result<ThirdPartyInvite, RepositoryError> {
        let invite_clone = invite.clone();
        let created: Option<ThirdPartyInvite> = self
            .db
            .create(("third_party_invite", &invite.signed.token))
            .content(invite_clone)
            .await?;

        created.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to create third party invite"))
        })
    }

    pub async fn get_by_token(
        &self,
        token: &str,
    ) -> Result<Option<ThirdPartyInvite>, RepositoryError> {
        let invite: Option<ThirdPartyInvite> =
            self.db.select(("third_party_invite", token)).await?;
        Ok(invite)
    }

    pub async fn get_room_invites(
        &self,
        room_id: &str,
    ) -> Result<Vec<ThirdPartyInvite>, RepositoryError> {
        let invites: Vec<ThirdPartyInvite> = self
            .db
            .query("SELECT * FROM third_party_invite WHERE room_id = $room_id")
            .bind(("room_id", room_id.to_string()))
            .await?
            .take(0)?;
        Ok(invites)
    }

    pub async fn delete(&self, token: &str) -> Result<(), RepositoryError> {
        let _: Option<ThirdPartyInvite> = self.db.delete(("third_party_invite", token)).await?;
        Ok(())
    }

    pub fn subscribe(
        &self,
    ) -> Pin<Box<dyn Stream<Item = Result<ThirdPartyInvite, RepositoryError>> + Send + '_>> {
        let query = "LIVE SELECT * FROM third_party_invite";
        let stream = self.db.query(query);

        Box::pin(async_stream::stream! {
            match stream.await {
                Ok(mut response) => {
                    match response.take::<Vec<ThirdPartyInvite>>(0) {
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
