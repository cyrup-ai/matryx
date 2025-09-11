use crate::repository::error::RepositoryError;
use matryx_entity::types::User;
use surrealdb::{Surreal, engine::any::Any};

#[derive(Clone)]
pub struct UserRepository {
    db: Surreal<Any>,
}

impl UserRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    pub async fn create(&self, user: &User) -> Result<User, RepositoryError> {
        let user_clone = user.clone();
        let created: Option<User> =
            self.db.create(("user", &user.user_id)).content(user_clone).await?;
        created.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to create user"))
        })
    }

    pub async fn get_by_id(&self, user_id: &str) -> Result<Option<User>, RepositoryError> {
        let user: Option<User> = self.db.select(("user", user_id)).await?;
        Ok(user)
    }

    pub async fn update(&self, user: &User) -> Result<User, RepositoryError> {
        let user_clone = user.clone();
        let updated: Option<User> =
            self.db.update(("user", &user.user_id)).content(user_clone).await?;
        updated.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to update user"))
        })
    }

    pub async fn delete(&self, user_id: &str) -> Result<(), RepositoryError> {
        let _: Option<User> = self.db.delete(("user", user_id)).await?;
        Ok(())
    }

    pub async fn authenticate(
        &self,
        user_id: &str,
        password_hash: &str,
    ) -> Result<Option<User>, RepositoryError> {
        let query = "SELECT * FROM user WHERE user_id = $user_id AND password_hash = $password_hash AND is_active = true LIMIT 1";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("password_hash", password_hash.to_string()))
            .await?;
        let users: Vec<User> = result.take(0)?;
        Ok(users.into_iter().next())
    }

    pub async fn get_all_users(&self, limit: Option<i64>) -> Result<Vec<User>, RepositoryError> {
        let query = match limit {
            Some(l) => format!("SELECT * FROM user LIMIT {}", l),
            None => "SELECT * FROM user".to_string(),
        };
        let mut result = self.db.query(&query).await?;
        let users: Vec<User> = result.take(0)?;
        Ok(users)
    }
}
