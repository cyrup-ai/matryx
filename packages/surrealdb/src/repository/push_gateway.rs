use crate::repository::error::RepositoryError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use surrealdb::{Connection, Surreal};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pusher {
    pub pusher_key: String,
    pub kind: String,
    pub app_id: String,
    pub app_display_name: String,
    pub device_display_name: String,
    pub profile_tag: Option<String>,
    pub lang: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushStatistics {
    pub total_attempts: u64,
    pub successful_attempts: u64,
    pub failed_attempts: u64,
    pub last_success: Option<DateTime<Utc>>,
    pub last_failure: Option<DateTime<Utc>>,
    pub failure_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PusherRecord {
    pub id: String,
    pub user_id: String,
    pub pusher_key: String,
    pub pusher_data: Pusher,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_used: Option<DateTime<Utc>>,
    pub failure_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushAttemptRecord {
    pub id: String,
    pub pusher_key: String,
    pub notification_id: String,
    pub success: bool,
    pub attempted_at: DateTime<Utc>,
    pub error_message: Option<String>,
    pub response_code: Option<u16>,
}

#[derive(Clone)]
pub struct PushGatewayRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> PushGatewayRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    pub async fn register_pusher(
        &self,
        user_id: &str,
        pusher: &Pusher,
    ) -> Result<(), RepositoryError> {
        let record = PusherRecord {
            id: format!("pusher:{}:{}", user_id, pusher.pusher_key),
            user_id: user_id.to_string(),
            pusher_key: pusher.pusher_key.clone(),
            pusher_data: pusher.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_used: None,
            failure_count: 0,
        };

        let _: Option<PusherRecord> = self
            .db
            .create(("pusher", format!("{}:{}", user_id, pusher.pusher_key)))
            .content(record)
            .await?;

        Ok(())
    }

    pub async fn get_user_pushers(&self, user_id: &str) -> Result<Vec<Pusher>, RepositoryError> {
        let query = "SELECT * FROM pusher WHERE user_id = $user_id ORDER BY created_at DESC";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;

        let records: Vec<PusherRecord> = result.take(0)?;
        Ok(records.into_iter().map(|r| r.pusher_data).collect())
    }

    pub async fn update_pusher(
        &self,
        user_id: &str,
        pusher_key: &str,
        pusher: &Pusher,
    ) -> Result<(), RepositoryError> {
        let record_id = format!("{}:{}", user_id, pusher_key);

        // Get existing record to preserve created_at and failure_count
        let existing: Option<PusherRecord> = self.db.select(("pusher", &record_id)).await?;

        let (created_at, failure_count) = existing
            .map(|r| (r.created_at, r.failure_count))
            .unwrap_or_else(|| (Utc::now(), 0));

        let record = PusherRecord {
            id: format!("pusher:{}", record_id),
            user_id: user_id.to_string(),
            pusher_key: pusher_key.to_string(),
            pusher_data: pusher.clone(),
            created_at,
            updated_at: Utc::now(),
            last_used: None,
            failure_count,
        };

        let _: Option<PusherRecord> = self.db.update(("pusher", record_id)).content(record).await?;

        Ok(())
    }

    pub async fn remove_pusher(
        &self,
        user_id: &str,
        pusher_key: &str,
    ) -> Result<(), RepositoryError> {
        let _: Option<PusherRecord> =
            self.db.delete(("pusher", format!("{}:{}", user_id, pusher_key))).await?;

        // Also remove related push attempts
        let cleanup_query = "DELETE FROM push_attempt WHERE pusher_key = $pusher_key";
        self.db
            .query(cleanup_query)
            .bind(("pusher_key", pusher_key.to_string()))
            .await?;

        Ok(())
    }

    pub async fn get_pusher_by_key(
        &self,
        user_id: &str,
        pusher_key: &str,
    ) -> Result<Option<Pusher>, RepositoryError> {
        let record: Option<PusherRecord> =
            self.db.select(("pusher", format!("{}:{}", user_id, pusher_key))).await?;

        Ok(record.map(|r| r.pusher_data))
    }

    pub async fn record_push_attempt(
        &self,
        pusher_key: &str,
        notification_id: &str,
        success: bool,
    ) -> Result<(), RepositoryError> {
        let attempt_record = PushAttemptRecord {
            id: format!("push_attempt:{}:{}", pusher_key, notification_id),
            pusher_key: pusher_key.to_string(),
            notification_id: notification_id.to_string(),
            success,
            attempted_at: Utc::now(),
            error_message: if success {
                None
            } else {
                Some("Push delivery failed".to_string())
            },
            response_code: if success { Some(200) } else { Some(500) },
        };

        let _: Option<PushAttemptRecord> = self
            .db
            .create(("push_attempt", format!("{}:{}", pusher_key, notification_id)))
            .content(attempt_record)
            .await?;

        // Update pusher record with last used time and failure count
        self.update_pusher_stats(pusher_key, success).await?;

        Ok(())
    }

    pub async fn get_push_statistics(
        &self,
        pusher_key: &str,
    ) -> Result<PushStatistics, RepositoryError> {
        // Get total attempts
        let total_query =
            "SELECT count() AS total FROM push_attempt WHERE pusher_key = $pusher_key";
        let mut total_result = self
            .db
            .query(total_query)
            .bind(("pusher_key", pusher_key.to_string()))
            .await?;

        #[derive(Deserialize)]
        struct TotalCountResult {
            total: u64,
        }

        let total_counts: Vec<TotalCountResult> = total_result.take(0)?;
        let total_attempts = total_counts.into_iter().next().map(|r| r.total).unwrap_or(0);

        // Get successful attempts
        let success_query = "SELECT count() AS successful FROM push_attempt WHERE pusher_key = $pusher_key AND success = true";
        let mut success_result = self
            .db
            .query(success_query)
            .bind(("pusher_key", pusher_key.to_string()))
            .await?;

        #[derive(Deserialize)]
        struct SuccessCountResult {
            successful: u64,
        }

        let success_counts: Vec<SuccessCountResult> = success_result.take(0)?;
        let successful_attempts =
            success_counts.into_iter().next().map(|r| r.successful).unwrap_or(0);

        let failed_attempts = total_attempts - successful_attempts;
        let failure_rate = if total_attempts > 0 {
            failed_attempts as f64 / total_attempts as f64
        } else {
            0.0
        };

        // Get last success
        let last_success_query = "SELECT attempted_at FROM push_attempt WHERE pusher_key = $pusher_key AND success = true ORDER BY attempted_at DESC LIMIT 1";
        let mut last_success_result = self
            .db
            .query(last_success_query)
            .bind(("pusher_key", pusher_key.to_string()))
            .await?;

        #[derive(Deserialize)]
        struct AttemptResult {
            attempted_at: DateTime<Utc>,
        }

        let last_success_records: Vec<AttemptResult> = last_success_result.take(0)?;
        let last_success = last_success_records.into_iter().next().map(|r| r.attempted_at);

        // Get last failure
        let last_failure_query = "SELECT attempted_at FROM push_attempt WHERE pusher_key = $pusher_key AND success = false ORDER BY attempted_at DESC LIMIT 1";
        let mut last_failure_result = self
            .db
            .query(last_failure_query)
            .bind(("pusher_key", pusher_key.to_string()))
            .await?;

        let last_failure_records: Vec<AttemptResult> = last_failure_result.take(0)?;
        let last_failure = last_failure_records.into_iter().next().map(|r| r.attempted_at);

        Ok(PushStatistics {
            total_attempts,
            successful_attempts,
            failed_attempts,
            last_success,
            last_failure,
            failure_rate,
        })
    }

    pub async fn cleanup_failed_pushers(
        &self,
        failure_threshold: u32,
    ) -> Result<u64, RepositoryError> {
        let query = "DELETE FROM pusher WHERE failure_count >= $threshold";
        let mut result = self.db.query(query).bind(("threshold", failure_threshold)).await?;

        let deleted: Option<Vec<serde_json::Value>> = result.take(0).ok();
        Ok(deleted.map(|v| v.len()).unwrap_or(0) as u64)
    }

    async fn update_pusher_stats(
        &self,
        pusher_key: &str,
        success: bool,
    ) -> Result<(), RepositoryError> {
        if success {
            // Update last used time and reset failure count
            let update_query = "UPDATE pusher SET last_used = $now, failure_count = 0, updated_at = $now WHERE pusher_key = $pusher_key";
            self.db
                .query(update_query)
                .bind(("pusher_key", pusher_key.to_string()))
                .bind(("now", Utc::now()))
                .await?;
        } else {
            // Increment failure count
            let update_query = "UPDATE pusher SET failure_count = failure_count + 1, updated_at = $now WHERE pusher_key = $pusher_key";
            self.db
                .query(update_query)
                .bind(("pusher_key", pusher_key.to_string()))
                .bind(("now", Utc::now()))
                .await?;
        }

        Ok(())
    }

    pub async fn get_active_pushers(&self, days: u32) -> Result<Vec<Pusher>, RepositoryError> {
        let cutoff = Utc::now() - chrono::Duration::days(days as i64);
        let query = "SELECT * FROM pusher WHERE last_used >= $cutoff OR created_at >= $cutoff ORDER BY updated_at DESC";
        let mut result = self.db.query(query).bind(("cutoff", cutoff)).await?;

        let records: Vec<PusherRecord> = result.take(0)?;
        Ok(records.into_iter().map(|r| r.pusher_data).collect())
    }

    pub async fn get_failed_pushers(
        &self,
        failure_threshold: u32,
    ) -> Result<Vec<Pusher>, RepositoryError> {
        let query =
            "SELECT * FROM pusher WHERE failure_count >= $threshold ORDER BY failure_count DESC";
        let mut result = self.db.query(query).bind(("threshold", failure_threshold)).await?;

        let records: Vec<PusherRecord> = result.take(0)?;
        Ok(records.into_iter().map(|r| r.pusher_data).collect())
    }

    pub async fn cleanup_old_push_attempts(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<u64, RepositoryError> {
        let query = "DELETE FROM push_attempt WHERE attempted_at < $cutoff";
        let mut result = self.db.query(query).bind(("cutoff", cutoff)).await?;

        let deleted: Option<Vec<serde_json::Value>> = result.take(0).ok();
        Ok(deleted.map(|v| v.len()).unwrap_or(0) as u64)
    }

    pub async fn get_pusher_attempt_history(
        &self,
        pusher_key: &str,
        limit: Option<u32>,
    ) -> Result<Vec<PushAttemptRecord>, RepositoryError> {
        let query = if let Some(limit) = limit {
            format!(
                "SELECT * FROM push_attempt WHERE pusher_key = $pusher_key ORDER BY attempted_at DESC LIMIT {}",
                limit
            )
        } else {
            "SELECT * FROM push_attempt WHERE pusher_key = $pusher_key ORDER BY attempted_at DESC"
                .to_string()
        };

        let mut result = self.db.query(&query).bind(("pusher_key", pusher_key.to_string())).await?;

        let records: Vec<PushAttemptRecord> = result.take(0)?;
        Ok(records)
    }

    pub async fn get_all_user_pushers_with_stats(
        &self,
        user_id: &str,
    ) -> Result<Vec<(Pusher, PushStatistics)>, RepositoryError> {
        let pushers = self.get_user_pushers(user_id).await?;
        let mut pushers_with_stats = Vec::new();

        for pusher in pushers {
            let stats = self.get_push_statistics(&pusher.pusher_key).await?;
            pushers_with_stats.push((pusher, stats));
        }

        Ok(pushers_with_stats)
    }

    pub async fn record_push_attempt_with_details(
        &self,
        pusher_key: &str,
        notification_id: &str,
        success: bool,
        error_message: Option<String>,
        response_code: Option<u16>,
    ) -> Result<(), RepositoryError> {
        let attempt_record = PushAttemptRecord {
            id: format!("push_attempt:{}:{}", pusher_key, notification_id),
            pusher_key: pusher_key.to_string(),
            notification_id: notification_id.to_string(),
            success,
            attempted_at: Utc::now(),
            error_message,
            response_code,
        };

        let _: Option<PushAttemptRecord> = self
            .db
            .create(("push_attempt", format!("{}:{}", pusher_key, notification_id)))
            .content(attempt_record)
            .await?;

        // Update pusher stats
        self.update_pusher_stats(pusher_key, success).await?;

        Ok(())
    }
}
