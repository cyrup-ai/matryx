use crate::repository::error::RepositoryError;
use crate::repository::third_party::{BridgeConfig, BridgeStatus, BridgeStatistics};
use chrono::{DateTime, Utc};
use std::time::Duration;
use surrealdb::{Connection, Surreal};
use tokio::time::Instant;

// TASK17 SUBTASK 3: Create BridgeRepository
pub struct BridgeRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> BridgeRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    /// Register a new bridge
    pub async fn register_bridge(&self, bridge: &BridgeConfig) -> Result<(), RepositoryError> {
        let _: Option<BridgeConfig> = self.db
            .create(("bridges", &bridge.bridge_id))
            .content(bridge.clone())
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "register_bridge".to_string(),
            })?;

        Ok(())
    }

    /// Get bridge by ID
    pub async fn get_bridge_by_id(&self, bridge_id: &str) -> Result<Option<BridgeConfig>, RepositoryError> {
        let bridge: Option<BridgeConfig> = self.db
            .select(("bridges", bridge_id))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_bridge_by_id".to_string(),
            })?;

        Ok(bridge)
    }

    /// Get bridges for a specific protocol
    pub async fn get_bridges_for_protocol(&self, protocol: &str) -> Result<Vec<BridgeConfig>, RepositoryError> {
        let query = "SELECT * FROM bridges WHERE protocol = $protocol ORDER BY name";
        let mut result = self.db
            .query(query)
            .bind(("protocol", protocol.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_bridges_for_protocol".to_string(),
            })?;

        let bridges: Vec<BridgeConfig> = result.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_bridges_for_protocol_parse".to_string(),
        })?;

        Ok(bridges)
    }

    /// Update bridge status
    pub async fn update_bridge_status(&self, bridge_id: &str, status: BridgeStatus) -> Result<(), RepositoryError> {
        let query = "UPDATE bridges SET status = $status, last_seen = time::now() WHERE bridge_id = $bridge_id";
        self.db
            .query(query)
            .bind(("bridge_id", bridge_id.to_string()))
            .bind(("status", status))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "update_bridge_status".to_string(),
            })?;

        Ok(())
    }

    /// Get bridge statistics
    pub async fn get_bridge_statistics(&self, bridge_id: &str) -> Result<BridgeStatistics, RepositoryError> {
        // Get basic bridge info
        let bridge = self.get_bridge_by_id(bridge_id).await?
            .ok_or_else(|| RepositoryError::NotFound {
                entity_type: "Bridge".to_string(),
                id: bridge_id.to_string(),
            })?;

        // Query statistics from various tables
        let user_count_query = "SELECT count() FROM thirdparty_users WHERE protocol = $protocol GROUP ALL";
        let mut user_result = self.db
            .query(user_count_query)
            .bind(("protocol", bridge.protocol.clone()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_bridge_statistics_users".to_string(),
            })?;

        let user_count: Option<i64> = user_result.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_bridge_statistics_users_parse".to_string(),
        })?;

        let room_count_query = "SELECT count() FROM thirdparty_locations WHERE protocol = $protocol GROUP ALL";
        let mut room_result = self.db
            .query(room_count_query)
            .bind(("protocol", bridge.protocol.clone()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_bridge_statistics_rooms".to_string(),
            })?;

        let room_count: Option<i64> = room_result.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_bridge_statistics_rooms_parse".to_string(),
        })?;

        // Get performance metrics from bridge_metrics table
        let perf_metrics = self.get_bridge_performance_metrics(bridge_id).await?;

        Ok(BridgeStatistics {
            total_users: user_count.unwrap_or(0) as u64,
            total_rooms: room_count.unwrap_or(0) as u64,
            messages_bridged_24h: perf_metrics.messages_24h,
            uptime_percentage: perf_metrics.uptime_percentage,
            last_error: None, // No error message storage in schema (only error counts tracked)
        })
    }

    /// Cleanup inactive bridges
    pub async fn cleanup_inactive_bridges(&self, cutoff: DateTime<Utc>) -> Result<u64, RepositoryError> {
        let query = "DELETE FROM bridges WHERE status = 'Inactive' AND last_seen < $cutoff";
        let mut result = self.db
            .query(query)
            .bind(("cutoff", cutoff))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "cleanup_inactive_bridges".to_string(),
            })?;

        let deleted: Vec<BridgeConfig> = result.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "cleanup_inactive_bridges_parse".to_string(),
        })?;

        Ok(deleted.len() as u64)
    }

    /// Get all active bridges
    pub async fn get_all_active_bridges(&self) -> Result<Vec<BridgeConfig>, RepositoryError> {
        let query = "SELECT * FROM bridges WHERE status = 'Active' ORDER BY name";
        let mut result = self.db
            .query(query)
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_all_active_bridges".to_string(),
            })?;

        let bridges: Vec<BridgeConfig> = result.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_all_active_bridges_parse".to_string(),
        })?;

        Ok(bridges)
    }

    // TASK17 SUBTASK 10: Add Bridge Health Monitoring

    /// Monitor bridge connectivity and response times
    pub async fn monitor_bridge_health(&self, bridge_id: &str) -> Result<crate::repository::third_party::BridgeHealth, RepositoryError> {
        let bridge = self.get_bridge_by_id(bridge_id).await?
            .ok_or_else(|| RepositoryError::NotFound {
                entity_type: "Bridge".to_string(),
                id: bridge_id.to_string(),
            })?;

        // Perform health check by attempting to connect to bridge URL
        let health_status = match self.perform_health_check(&bridge.url, &bridge.hs_token).await {
            Ok(response_time) => {
                if response_time < 1000 { // Less than 1 second
                    crate::repository::third_party::BridgeHealth::Healthy
                } else if response_time < 5000 { // Less than 5 seconds
                    crate::repository::third_party::BridgeHealth::Degraded
                } else {
                    crate::repository::third_party::BridgeHealth::Unhealthy
                }
            },
            Err(_) => crate::repository::third_party::BridgeHealth::Unhealthy,
        };

        // Update bridge status based on health
        let bridge_status = match health_status {
            crate::repository::third_party::BridgeHealth::Healthy => BridgeStatus::Active,
            crate::repository::third_party::BridgeHealth::Degraded => BridgeStatus::Active,
            crate::repository::third_party::BridgeHealth::Unhealthy => BridgeStatus::Error,
            crate::repository::third_party::BridgeHealth::Unknown => BridgeStatus::Inactive,
        };

        self.update_bridge_status(bridge_id, bridge_status).await?;

        Ok(health_status)
    }

    /// Perform actual health check against bridge URL (simplified implementation)
    async fn perform_health_check(&self, bridge_url: &str, hs_token: &str) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        // Create HTTP client with 5 second timeout
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()?;
        
        // Generate unique transaction ID
        let transaction_id = uuid::Uuid::new_v4().to_string();
        
        // Construct ping endpoint URL
        let ping_url = format!("{}/_matrix/app/v1/ping", bridge_url.trim_end_matches('/'));
        
        // Create request body per Matrix spec
        let request_body = serde_json::json!({
            "transaction_id": transaction_id
        });
        
        // Start timing
        let start = Instant::now();
        
        // Make HTTP POST request with authentication
        let response = client
            .post(&ping_url)
            .header("Authorization", format!("Bearer {}", hs_token))
            .json(&request_body)
            .send()
            .await?;
        
        // Calculate response time in milliseconds
        let response_time_ms = start.elapsed().as_millis() as u64;
        
        // Check response status
        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            
            return Err(format!(
                "Bridge health check failed: HTTP {} - {}",
                status.as_u16(),
                error_body
            ).into());
        }
        
        // Verify response is valid JSON (should be empty object {})
        let _response_body: serde_json::Value = response.json().await?;
        
        Ok(response_time_ms)
    }

    /// Track bridge message throughput and errors
    pub async fn track_bridge_metrics(&self, bridge_id: &str, messages_count: u64, error_count: u64) -> Result<(), RepositoryError> {
        let query = r#"
            UPDATE bridge_metrics SET 
                messages_24h = messages_24h + $messages_count,
                errors_24h = errors_24h + $error_count,
                last_updated = time::now()
            WHERE bridge_id = $bridge_id
        "#;

        self.db
            .query(query)
            .bind(("bridge_id", bridge_id.to_string()))
            .bind(("messages_count", messages_count))
            .bind(("error_count", error_count))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "track_bridge_metrics".to_string(),
            })?;

        Ok(())
    }

    /// Implement automatic bridge failover
    pub async fn perform_bridge_failover(&self, failed_bridge_id: &str) -> Result<Option<String>, RepositoryError> {
        // Get the protocol of the failed bridge
        let failed_bridge = self.get_bridge_by_id(failed_bridge_id).await?
            .ok_or_else(|| RepositoryError::NotFound {
                entity_type: "Bridge".to_string(),
                id: failed_bridge_id.to_string(),
            })?;

        // Find alternative active bridges for the same protocol
        let alternative_bridges = self.get_bridges_for_protocol(&failed_bridge.protocol).await?;
        
        for bridge in alternative_bridges {
            if bridge.bridge_id != failed_bridge_id && matches!(bridge.status, BridgeStatus::Active) {
                // Check if alternative bridge is healthy
                if let Ok(health) = self.monitor_bridge_health(&bridge.bridge_id).await
                    && matches!(health, crate::repository::third_party::BridgeHealth::Healthy) {
                        // Mark failed bridge as inactive
                        self.update_bridge_status(failed_bridge_id, BridgeStatus::Error).await?;

                        // Return the alternative bridge ID
                        return Ok(Some(bridge.bridge_id));
                    }
            }
        }

        // No healthy alternative found
        Ok(None)
    }

    /// Add bridge performance metrics
    pub async fn get_bridge_performance_metrics(&self, bridge_id: &str) -> Result<BridgePerformanceMetrics, RepositoryError> {
        let query = r#"
            SELECT 
                messages_24h,
                errors_24h,
                avg_response_time,
                uptime_percentage,
                last_health_check
            FROM bridge_metrics 
            WHERE bridge_id = $bridge_id 
            LIMIT 1
        "#;

        let mut result = self.db
            .query(query)
            .bind(("bridge_id", bridge_id.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_bridge_performance_metrics".to_string(),
            })?;

        let metrics: Vec<serde_json::Value> = result.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_bridge_performance_metrics_parse".to_string(),
        })?;

        if let Some(metric) = metrics.first() {
            Ok(BridgePerformanceMetrics {
                messages_24h: metric.get("messages_24h").and_then(|v| v.as_u64()).unwrap_or(0),
                errors_24h: metric.get("errors_24h").and_then(|v| v.as_u64()).unwrap_or(0),
                avg_response_time: metric.get("avg_response_time").and_then(|v| v.as_u64()).unwrap_or(0),
                uptime_percentage: metric.get("uptime_percentage").and_then(|v| v.as_f64()).unwrap_or(0.0),
                last_health_check: metric.get("last_health_check")
                    .and_then(|v| v.as_str())
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc)),
            })
        } else {
            // Return default metrics if none found
            Ok(BridgePerformanceMetrics {
                messages_24h: 0,
                errors_24h: 0,
                avg_response_time: 0,
                uptime_percentage: 0.0,
                last_health_check: None,
            })
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BridgePerformanceMetrics {
    pub messages_24h: u64,
    pub errors_24h: u64,
    pub avg_response_time: u64,
    pub uptime_percentage: f64,
    pub last_health_check: Option<DateTime<Utc>>,
}