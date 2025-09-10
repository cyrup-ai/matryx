use crate::repository::error::RepositoryError;
use matryx_entity::types::Membership;
use surrealdb::method::Stream;
use surrealdb::{Connection, Surreal};

pub struct MembershipRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> MembershipRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    pub async fn create(&self, membership: &Membership) -> Result<Membership, RepositoryError> {
        let membership_clone = membership.clone();
        let id = format!("{}:{}", membership.room_id, membership.user_id);
        let created: Option<Membership> =
            self.db.create(("room_membership", id)).content(membership_clone).await?;

        created.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to create membership"))
        })
    }

    pub async fn get_by_room_user(
        &self,
        room_id: &str,
        user_id: &str,
    ) -> Result<Option<Membership>, RepositoryError> {
        let id = format!("{}:{}", room_id, user_id);
        let membership: Option<Membership> = self.db.select(("membership", id)).await?;
        Ok(membership)
    }

    pub async fn get_room_members(
        &self,
        room_id: &str,
    ) -> Result<Vec<Membership>, RepositoryError> {
        let room_id_owned = room_id.to_string();
        let memberships: Vec<Membership> = self
            .db
            .query("SELECT * FROM room_membership WHERE room_id = $room_id AND membership = 'join'")
            .bind(("room_id", room_id_owned))
            .await?
            .take(0)?;
        Ok(memberships)
    }

    pub async fn get_user_rooms(&self, user_id: &str) -> Result<Vec<Membership>, RepositoryError> {
        let user_id_owned = user_id.to_string();
        let memberships: Vec<Membership> = self
            .db
            .query("SELECT * FROM room_membership WHERE user_id = $user_id AND membership = 'join'")
            .bind(("user_id", user_id_owned))
            .await?
            .take(0)?;
        Ok(memberships)
    }

    pub async fn update(&self, membership: &Membership) -> Result<Membership, RepositoryError> {
        let membership_clone = membership.clone();
        let id = format!("{}:{}", membership.room_id, membership.user_id);
        let updated: Option<Membership> =
            self.db.update(("room_membership", id)).content(membership_clone).await?;

        updated.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to update membership"))
        })
    }

    /// Subscribe to real-time room membership changes using SurrealDB LiveQuery
    /// Returns a stream of notifications for membership changes in the specified room
    /// Note: Filtering must be done client-side as SurrealDB LiveQuery doesn't support WHERE clauses
    pub async fn subscribe_room_membership(
        &self,
        _room_id: &str,
    ) -> Result<Stream<Vec<Membership>>, RepositoryError> {
        // SurrealDB LiveQuery API - subscribes to all memberships in the table
        // Client must filter by room_id from the stream notifications
        let stream = self.db.select("membership").live().await?;

        Ok(stream)
    }

    /// Subscribe to real-time user membership changes using SurrealDB LiveQuery
    /// Returns a stream of notifications for membership changes for the specified user
    /// Note: Filtering must be done client-side as SurrealDB LiveQuery doesn't support WHERE clauses
    pub async fn subscribe_user_membership(
        &self,
        _user_id: &str,
    ) -> Result<Stream<Vec<Membership>>, RepositoryError> {
        // SurrealDB LiveQuery API - subscribes to all memberships in the table
        // Client must filter by user_id from the stream notifications
        let stream = self.db.select("membership").live().await?;

        Ok(stream)
    }
}
