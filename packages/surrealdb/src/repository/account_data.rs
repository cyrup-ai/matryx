use crate::repository::error::RepositoryError;
use chrono::Utc;
use matryx_entity::types::{AccountData, AccountDataEvent, AccountDataSync};
use serde_json::Value;
use std::collections::HashMap;
use surrealdb::{Surreal, engine::any::Any};

#[derive(Clone)]
pub struct AccountDataRepository {
    db: Surreal<Any>,
}

impl AccountDataRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    pub async fn create(&self, account_data: &AccountData) -> Result<AccountData, RepositoryError> {
        let account_data_clone = account_data.clone();
        let id = format!(
            "{}:{}:{}",
            account_data.user_id,
            account_data.room_id.as_deref().unwrap_or("global"),
            account_data.account_data_type
        );
        let created: Option<AccountData> =
            self.db.create(("account_data", id)).content(account_data_clone).await?;
        created.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to create account_data"))
        })
    }

    pub async fn get_by_user_and_type(
        &self,
        user_id: &str,
        account_data_type: &str,
    ) -> Result<Option<AccountData>, RepositoryError> {
        let query = "SELECT * FROM account_data WHERE user_id = $user_id AND account_data_type = $account_data_type AND room_id IS NONE LIMIT 1";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("account_data_type", account_data_type.to_string()))
            .await?;
        let account_data: Vec<AccountData> = result.take(0)?;
        Ok(account_data.into_iter().next())
    }

    pub async fn get_room_account_data(
        &self,
        user_id: &str,
        room_id: &str,
        account_data_type: &str,
    ) -> Result<Option<AccountData>, RepositoryError> {
        let query = "SELECT * FROM account_data WHERE user_id = $user_id AND room_id = $room_id AND account_data_type = $account_data_type LIMIT 1";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .bind(("account_data_type", account_data_type.to_string()))
            .await?;
        let account_data: Vec<AccountData> = result.take(0)?;
        Ok(account_data.into_iter().next())
    }

    pub async fn update(&self, account_data: &AccountData) -> Result<AccountData, RepositoryError> {
        let account_data_clone = account_data.clone();
        let id = format!(
            "{}:{}:{}",
            account_data.user_id,
            account_data.room_id.as_deref().unwrap_or("global"),
            account_data.account_data_type
        );
        let updated: Option<AccountData> =
            self.db.update(("account_data", id)).content(account_data_clone).await?;
        updated.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to update account_data"))
        })
    }

    pub async fn upsert(&self, account_data: &AccountData) -> Result<AccountData, RepositoryError> {
        // Try to update first, if not found then create
        let query = r#"
            UPDATE account_data SET
                content = $content,
                updated_at = time::now()
            WHERE user_id = $user_id AND room_id = $room_id AND account_data_type = $account_data_type
            ELSE CREATE account_data SET
                user_id = $user_id,
                room_id = $room_id,
                account_data_type = $account_data_type,
                content = $content,
                created_at = time::now(),
                updated_at = time::now()
        "#;

        let mut result = self
            .db
            .query(query)
            .bind(("user_id", account_data.user_id.clone()))
            .bind(("room_id", account_data.room_id.clone()))
            .bind(("account_data_type", account_data.account_data_type.clone()))
            .bind(("content", account_data.content.clone()))
            .await?;

        let upserted: Vec<AccountData> = result.take(0)?;
        upserted.into_iter().next().ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to upsert account_data"))
        })
    }

    pub async fn delete(
        &self,
        user_id: &str,
        room_id: Option<&str>,
        account_data_type: &str,
    ) -> Result<(), RepositoryError> {
        let id = format!("{}:{}:{}", user_id, room_id.unwrap_or("global"), account_data_type);
        let _: Option<AccountData> = self.db.delete(("account_data", id)).await?;
        Ok(())
    }

    pub async fn get_all_for_user(
        &self,
        user_id: &str,
    ) -> Result<Vec<AccountData>, RepositoryError> {
        let query = "SELECT * FROM account_data WHERE user_id = $user_id";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        let account_data: Vec<AccountData> = result.take(0)?;
        Ok(account_data)
    }

    pub async fn get_global_for_user(
        &self,
        user_id: &str,
    ) -> Result<Vec<AccountData>, RepositoryError> {
        let query = "SELECT * FROM account_data WHERE user_id = $user_id AND room_id IS NONE";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        let account_data: Vec<AccountData> = result.take(0)?;
        Ok(account_data)
    }

    pub async fn get_room_data_for_user(
        &self,
        user_id: &str,
        room_id: &str,
    ) -> Result<Vec<AccountData>, RepositoryError> {
        let query = "SELECT * FROM account_data WHERE user_id = $user_id AND room_id = $room_id";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .await?;
        let account_data: Vec<AccountData> = result.take(0)?;
        Ok(account_data)
    }

    /// Set global account data
    pub async fn set_global_account_data(
        &self,
        user_id: &str,
        data_type: &str,
        content: Value,
    ) -> Result<(), RepositoryError> {
        // Validate data type
        if data_type.is_empty() {
            return Err(RepositoryError::Validation {
                field: "data_type".to_string(),
                message: "Data type cannot be empty".to_string(),
            });
        }

        let query = r#"
            UPDATE account_data SET
                content = $content,
                updated_at = time::now()
            WHERE user_id = $user_id AND account_data_type = $data_type AND room_id IS NONE
            ELSE CREATE account_data SET
                user_id = $user_id,
                room_id = NONE,
                account_data_type = $data_type,
                content = $content,
                created_at = time::now(),
                updated_at = time::now()
        "#;

        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("data_type", data_type.to_string()))
            .bind(("content", content))
            .await?;

        let _: Vec<AccountData> = result.take(0)?;
        Ok(())
    }

    /// Get global account data
    pub async fn get_global_account_data(
        &self,
        user_id: &str,
        data_type: &str,
    ) -> Result<Option<Value>, RepositoryError> {
        let query = "SELECT content FROM account_data WHERE user_id = $user_id AND account_data_type = $data_type AND room_id IS NONE LIMIT 1";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("data_type", data_type.to_string()))
            .await?;

        let data_rows: Vec<serde_json::Value> = result.take(0)?;
        if let Some(data_row) = data_rows.first() &&
            let Some(content) = data_row.get("content")
        {
            return Ok(Some(content.clone()));
        }

        Ok(None)
    }

    /// Get all global account data for a user
    pub async fn get_all_global_account_data(
        &self,
        user_id: &str,
    ) -> Result<HashMap<String, Value>, RepositoryError> {
        let query = "SELECT account_data_type, content FROM account_data WHERE user_id = $user_id AND room_id IS NONE";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        let data_rows: Vec<serde_json::Value> = result.take(0)?;

        let mut account_data = HashMap::new();
        for row in data_rows {
            if let (Some(data_type), Some(content)) =
                (row.get("account_data_type").and_then(|v| v.as_str()), row.get("content"))
            {
                account_data.insert(data_type.to_string(), content.clone());
            }
        }

        Ok(account_data)
    }

    /// Set room-specific account data
    pub async fn set_room_account_data(
        &self,
        user_id: &str,
        room_id: &str,
        data_type: &str,
        content: Value,
    ) -> Result<(), RepositoryError> {
        // Validate parameters
        if data_type.is_empty() {
            return Err(RepositoryError::Validation {
                field: "data_type".to_string(),
                message: "Data type cannot be empty".to_string(),
            });
        }
        if room_id.is_empty() {
            return Err(RepositoryError::Validation {
                field: "room_id".to_string(),
                message: "Room ID cannot be empty".to_string(),
            });
        }

        let query = r#"
            UPDATE account_data SET
                content = $content,
                updated_at = time::now()
            WHERE user_id = $user_id AND room_id = $room_id AND account_data_type = $data_type
            ELSE CREATE account_data SET
                user_id = $user_id,
                room_id = $room_id,
                account_data_type = $data_type,
                content = $content,
                created_at = time::now(),
                updated_at = time::now()
        "#;

        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .bind(("data_type", data_type.to_string()))
            .bind(("content", content))
            .await?;

        let _: Vec<AccountData> = result.take(0)?;
        Ok(())
    }

    /// Get room-specific account data content
    pub async fn get_room_account_data_content(
        &self,
        user_id: &str,
        room_id: &str,
        data_type: &str,
    ) -> Result<Option<Value>, RepositoryError> {
        let query = "SELECT content FROM account_data WHERE user_id = $user_id AND room_id = $room_id AND account_data_type = $data_type LIMIT 1";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .bind(("data_type", data_type.to_string()))
            .await?;

        let data_rows: Vec<serde_json::Value> = result.take(0)?;
        if let Some(data_row) = data_rows.first() &&
            let Some(content) = data_row.get("content")
        {
            return Ok(Some(content.clone()));
        }

        Ok(None)
    }

    /// Get all room-specific account data for a user and room
    pub async fn get_all_room_account_data(
        &self,
        user_id: &str,
        room_id: &str,
    ) -> Result<HashMap<String, Value>, RepositoryError> {
        let query = "SELECT account_data_type, content FROM account_data WHERE user_id = $user_id AND room_id = $room_id";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .await?;
        let data_rows: Vec<serde_json::Value> = result.take(0)?;

        let mut account_data = HashMap::new();
        for row in data_rows {
            if let (Some(data_type), Some(content)) =
                (row.get("account_data_type").and_then(|v| v.as_str()), row.get("content"))
            {
                account_data.insert(data_type.to_string(), content.clone());
            }
        }

        Ok(account_data)
    }

    /// Delete account data (global or room-specific)
    pub async fn delete_account_data(
        &self,
        user_id: &str,
        data_type: &str,
        room_id: Option<&str>,
    ) -> Result<(), RepositoryError> {
        let query = match room_id {
            Some(_room) => {
                "DELETE FROM account_data WHERE user_id = $user_id AND room_id = $room_id AND account_data_type = $data_type"
            },
            None => {
                "DELETE FROM account_data WHERE user_id = $user_id AND room_id IS NONE AND account_data_type = $data_type"
            },
        };

        let mut query_builder = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("data_type", data_type.to_string()));

        if let Some(room) = room_id {
            query_builder = query_builder.bind(("room_id", room.to_string()));
        }

        let mut result = query_builder.await?;
        let _: Vec<AccountData> = result.take(0)?;
        Ok(())
    }

    /// Get account data for sync with optional since token
    pub async fn get_account_data_for_sync(
        &self,
        user_id: &str,
        since: Option<&str>,
    ) -> Result<AccountDataSync, RepositoryError> {
        let mut sync_data = AccountDataSync::new();

        // Build query based on whether we have a since token
        let query = match since {
            Some(_) => {
                "SELECT * FROM account_data WHERE user_id = $user_id AND updated_at > $since ORDER BY updated_at"
            },
            None => "SELECT * FROM account_data WHERE user_id = $user_id ORDER BY updated_at",
        };

        let mut query_builder = self.db.query(query).bind(("user_id", user_id.to_string()));

        if let Some(since_token) = since {
            query_builder = query_builder.bind(("since", since_token.to_string()));
        }

        let mut result = query_builder.await?;
        let account_data_items: Vec<AccountData> = result.take(0)?;

        // Process account data items
        for item in account_data_items {
            // Create sync event
            let event = if item.room_id.is_some() {
                AccountDataEvent::room(
                    item.account_data_type.clone(),
                    item.content.clone(),
                    item.room_id.clone().unwrap_or_default(),
                )
            } else {
                AccountDataEvent::global(item.account_data_type.clone(), item.content.clone())
            };
            sync_data.add_event(event);

            // Add to appropriate data structure
            if let Some(room_id) = item.room_id {
                sync_data.add_room_data(room_id, item.account_data_type, item.content);
            } else {
                sync_data.add_global_data(item.account_data_type, item.content);
            }
        }

        // Generate next batch token (simplified - in real implementation would be more sophisticated)
        sync_data.next_batch = Some(Utc::now().timestamp_millis().to_string());

        Ok(sync_data)
    }
}
