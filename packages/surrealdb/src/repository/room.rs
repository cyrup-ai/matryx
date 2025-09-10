use crate::repository::error::RepositoryError;
use matryx_entity::types::Room;
use surrealdb::{engine::any::Any, Surreal};

#[derive(Clone)]
pub struct RoomRepository {
    db: Surreal<Any>,
}

impl RoomRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    pub async fn create(&self, room: &Room) -> Result<Room, RepositoryError> {
        let room_clone = room.clone();
        let created: Option<Room> = self
            .db
            .create(("room", &room.room_id))
            .content(room_clone)
            .await?;
        created.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to create room"))
        })
    }

    pub async fn get_by_id(&self, room_id: &str) -> Result<Option<Room>, RepositoryError> {
        let room: Option<Room> = self.db.select(("room", room_id)).await?;
        Ok(room)
    }

    pub async fn update(&self, room: &Room) -> Result<Room, RepositoryError> {
        let room_clone = room.clone();
        let updated: Option<Room> = self
            .db
            .update(("room", &room.room_id))
            .content(room_clone)
            .await?;
        updated.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to update room"))
        })
    }

    pub async fn delete(&self, room_id: &str) -> Result<(), RepositoryError> {
        let _: Option<Room> = self.db.delete(("room", room_id)).await?;
        Ok(())
    }

    pub async fn get_rooms_for_user(&self, user_id: &str) -> Result<Vec<Room>, RepositoryError> {
        let query = "
            SELECT * FROM room 
            WHERE creator = $user_id 
            OR room_id IN (
                SELECT room_id FROM membership 
                WHERE user_id = $user_id 
                AND membership IN ['join', 'invite']
            )
        ";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        let rooms: Vec<Room> = result.take(0)?;
        Ok(rooms)
    }

    pub async fn is_room_member(&self, room_id: &str, user_id: &str) -> Result<bool, RepositoryError> {
        let query = "
            SELECT count() FROM membership 
            WHERE room_id = $room_id 
            AND user_id = $user_id 
            AND membership IN ['join', 'invite']
            GROUP ALL
        ";
        let mut result = self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await?;
        let count: Option<i64> = result.take(0)?;
        Ok(count.unwrap_or(0) > 0)
    }

    pub async fn get_room_members(&self, room_id: &str) -> Result<Vec<String>, RepositoryError> {
        let query = "
            SELECT user_id FROM membership 
            WHERE room_id = $room_id 
            AND membership IN ['join', 'invite']
        ";
        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;
        let members: Vec<String> = result.take(0)?;
        Ok(members)
    }

    pub async fn get_public_rooms(&self, limit: Option<i64>) -> Result<Vec<Room>, RepositoryError> {
        let query = match limit {
            Some(l) => format!("SELECT * FROM room WHERE is_public = true LIMIT {}", l),
            None => "SELECT * FROM room WHERE is_public = true".to_string(),
        };
        let mut result = self.db.query(&query).await?;
        let rooms: Vec<Room> = result.take(0)?;
        Ok(rooms)
    }
}