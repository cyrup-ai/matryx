use crate::db::entity::ApiCache;
use crate::db::generic_dao::{Dao, BaseDao};
use crate::future::MatrixFuture;
use serde_json::json;
use chrono::Utc;

pub struct ApiCacheDao {
    dao: Dao<ApiCache>,
}

impl ApiCacheDao {
    pub fn new(client: crate::db::client::DatabaseClient) -> Self {
        Self { dao: Dao::new(client) }
    }
    
    /// Get a cached value by key
    pub fn get_cache_value(&self, key: &str) -> MatrixFuture<Option<String>> {
        let dao = self.dao.clone();
        let key = key.to_string();
        
        MatrixFuture::spawn(async move {
            let caches: Vec<ApiCache> = dao.query_with_params::<Vec<ApiCache>>(
                "SELECT * FROM api_cache WHERE endpoint = 'matrix_cache' AND parameters.key = $key LIMIT 1",
                json!({ "key": key })
            ).await?;
            
            if let Some(cache) = caches.first() {
                if let Some(value) = cache.response_data.get("value") {
                    if let Some(string_val) = value.as_str() {
                        return Ok(Some(string_val.to_string()));
                    }
                }
            }
            
            Ok(None)
        })
    }
    
    /// Store a cached value
    pub fn set_cache_value(&self, key: &str, value: String) -> MatrixFuture<()> {
        let dao = self.dao.clone();
        let key = key.to_string();
        
        MatrixFuture::spawn(async move {
            let now = Utc::now();
            
            // Try to update if exists
            let updated: Vec<ApiCache> = dao.query_with_params::<Vec<ApiCache>>(
                "UPDATE api_cache SET response_data.value = $value, cached_at = $now WHERE endpoint = 'matrix_cache' AND parameters.key = $key",
                json!({ "key": key, "value": value, "now": now })
            ).await?;
            
            // If not updated, create new
            if updated.is_empty() {
                let cache = ApiCache {
                    id: None,
                    endpoint: "matrix_cache".to_string(),
                    parameters: json!({ "key": key }),
                    response_data: json!({ "value": value }),
                    cached_at: now,
                    expires_at: None,
                    etag: None,
                };
                
                let mut cache = cache;
                dao.create(&mut cache).await?;
            }
            
            Ok(())
        })
    }
    
    /// Remove a cached value
    pub fn remove_cache_value(&self, key: &str) -> MatrixFuture<()> {
        let dao = self.dao.clone();
        let key = key.to_string();
        
        MatrixFuture::spawn(async move {
            dao.query_with_params::<()>(
                "DELETE FROM api_cache WHERE endpoint = 'matrix_cache' AND parameters.key = $key",
                json!({ "key": key })
            ).await?;
            
            Ok(())
        })
    }
}