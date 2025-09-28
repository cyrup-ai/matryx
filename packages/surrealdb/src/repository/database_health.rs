use crate::repository::error::RepositoryError;
use crate::repository::metrics::HealthStatus;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use surrealdb::{Connection, Surreal};

#[derive(Clone)]
pub struct DatabaseHealthRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> DatabaseHealthRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    /// Test basic database connectivity
    pub async fn check_connectivity(&self) -> Result<DatabaseHealthStatus, RepositoryError> {
        let start_time = Instant::now();
        
        match self.db.health().await {
            Ok(_) => {
                let response_time = start_time.elapsed();
                Ok(DatabaseHealthStatus {
                    status: HealthStatus::Healthy,
                    response_time_ms: response_time.as_millis() as u64,
                    last_check: Utc::now(),
                    details: Some("Database connectivity verified".to_string()),
                })
            },
            Err(e) => {
                Ok(DatabaseHealthStatus {
                    status: HealthStatus::Unhealthy,
                    response_time_ms: 0,
                    last_check: Utc::now(),
                    details: Some(format!("Database connectivity failed: {}", e)),
                })
            },
        }
    }

    /// Test database query performance
    pub async fn check_query_performance(&self) -> Result<DatabaseHealthStatus, RepositoryError> {
        let start_time = Instant::now();
        
        // Simple query to test performance
        let query_result = self.db.query("SELECT 1 as test").await;
        let response_time = start_time.elapsed();
        
        match query_result {
            Ok(_) => {
                let status = if response_time > Duration::from_millis(1000) {
                    HealthStatus::Degraded
                } else {
                    HealthStatus::Healthy
                };
                
                Ok(DatabaseHealthStatus {
                    status,
                    response_time_ms: response_time.as_millis() as u64,
                    last_check: Utc::now(),
                    details: Some(format!("Query performance: {}ms", response_time.as_millis())),
                })
            },
            Err(e) => {
                Ok(DatabaseHealthStatus {
                    status: HealthStatus::Unhealthy,
                    response_time_ms: response_time.as_millis() as u64,
                    last_check: Utc::now(),
                    details: Some(format!("Query failed: {}", e)),
                })
            },
        }
    }

    /// Comprehensive database health check
    pub async fn comprehensive_health_check(&self) -> Result<DatabaseHealthStatus, RepositoryError> {
        // Test connectivity first
        let connectivity = self.check_connectivity().await?;
        if connectivity.status == HealthStatus::Unhealthy {
            return Ok(connectivity);
        }

        // Test query performance
        let performance = self.check_query_performance().await?;
        
        // Return worst status
        let final_status = match (&connectivity.status, &performance.status) {
            (HealthStatus::Unhealthy, _) | (_, HealthStatus::Unhealthy) => HealthStatus::Unhealthy,
            (HealthStatus::Degraded, _) | (_, HealthStatus::Degraded) => HealthStatus::Degraded,
            _ => HealthStatus::Healthy,
        };

        Ok(DatabaseHealthStatus {
            status: final_status,
            response_time_ms: performance.response_time_ms,
            last_check: Utc::now(),
            details: Some(format!(
                "Connectivity: {}ms, Performance: {}ms", 
                connectivity.response_time_ms, 
                performance.response_time_ms
            )),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseHealthStatus {
    pub status: HealthStatus,
    pub response_time_ms: u64,
    pub last_check: DateTime<Utc>,
    pub details: Option<String>,
}