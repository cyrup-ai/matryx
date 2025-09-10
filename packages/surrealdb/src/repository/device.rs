use crate::repository::error::RepositoryError;
use matryx_entity::types::Device;
use surrealdb::{engine::any::Any, Surreal};

#[derive(Clone)]
pub struct DeviceRepository {
    db: Surreal<Any>,
}

impl DeviceRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    pub async fn create(&self, device: &Device) -> Result<Device, RepositoryError> {
        let device_clone = device.clone();
        let created: Option<Device> = self
            .db
            .create(("device", &device.device_id))
            .content(device_clone)
            .await?;
        created.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to create device"))
        })
    }

    pub async fn get_by_id(&self, device_id: &str) -> Result<Option<Device>, RepositoryError> {
        let device: Option<Device> = self.db.select(("device", device_id)).await?;
        Ok(device)
    }

    pub async fn update(&self, device: &Device) -> Result<Device, RepositoryError> {
        let device_clone = device.clone();
        let updated: Option<Device> = self
            .db
            .update(("device", &device.device_id))
            .content(device_clone)
            .await?;
        updated.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to update device"))
        })
    }

    pub async fn delete(&self, device_id: &str) -> Result<(), RepositoryError> {
        let _: Option<Device> = self.db.delete(("device", device_id)).await?;
        Ok(())
    }

    pub async fn get_user_devices(&self, user_id: &str) -> Result<Vec<Device>, RepositoryError> {
        let query = "SELECT * FROM device WHERE user_id = $user_id";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        let devices: Vec<Device> = result.take(0)?;
        Ok(devices)
    }

    pub async fn delete_user_devices(&self, user_id: &str) -> Result<(), RepositoryError> {
        let query = "DELETE FROM device WHERE user_id = $user_id";
        self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        Ok(())
    }

    pub async fn get_by_user(&self, user_id: &str) -> Result<Vec<Device>, RepositoryError> {
        self.get_user_devices(user_id).await
    }

    pub async fn get_by_user_and_device(&self, user_id: &str, device_id: &str) -> Result<Option<Device>, RepositoryError> {
        let query = "SELECT * FROM device WHERE user_id = $user_id AND device_id = $device_id LIMIT 1";
        let mut result = self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .await?;
        let devices: Vec<Device> = result.take(0)?;
        Ok(devices.into_iter().next())
    }
}