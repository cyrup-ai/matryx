use crate::integration::{MatrixTestServer, create_test_user, create_test_room};
use serde_json::json;
use std::time::{Duration, Instant};
use tokio::task::JoinSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Performance and Load Testing Framework
pub struct LoadTest {
    server: Arc<MatrixTestServer>,
    metrics: PerformanceMetrics,
}

impl LoadTest {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let server = Arc::new(MatrixTestServer::new().await);
        let metrics = PerformanceMetrics::new();
        
        Ok(Self {
            server,
            metrics,
        })
    }
    
    pub async fn test_concurrent_users(&self, user_count: u32) -> Result<PerformanceReport, Box<dyn std::error::Error>> {
        let start_time = Instant::now();
        let mut tasks = JoinSet::new();
        
        println!("Starting load test with {} concurrent users", user_count);
        
        // Spawn concurrent user sessions
        for i in 0..user_count {
            let server = self.server.clone();
            let metrics = self.metrics.clone();
            
            tasks.spawn(async move {
                let username = format!("loadtest_user_{}", i);
                
                // Simulate user activity
                match simulate_user_session(&server, &username, &metrics).await {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        println!("User {} failed: {}", username, e);
                        Err(e)
                    }
                }
            });
        }
        
        // Wait for all tasks to complete with timeout
        let mut successful_users = 0;
        let mut failed_users = 0;
        
        while let Some(result) = tasks.join_next().await {
            match result {
                Ok(Ok(_)) => successful_users += 1,
                Ok(Err(_)) => failed_users += 1,
                Err(_) => failed_users += 1,
            }
        }
        
        let duration = start_time.elapsed();
        
        Ok(PerformanceReport {
            user_count,
            successful_users,
            failed_users,
            duration,
            messages_per_second: self.metrics.messages_sent.load(Ordering::Relaxed) as f64 / duration.as_secs_f64(),
            requests_per_second: self.metrics.requests_sent.load(Ordering::Relaxed) as f64 / duration.as_secs_f64(),
            average_response_time_ms: self.metrics.total_response_time_ms.load(Ordering::Relaxed) as f64 / 
                                     self.metrics.requests_sent.load(Ordering::Relaxed) as f64,
        })
    }
    
    pub async fn test_message_throughput(&self, message_count: u32) -> Result<ThroughputReport, Box<dyn std::error::Error>> {
        // Create a test user and room
        let (user_id, access_token) = create_test_user(&self.server, "throughput_user", "password").await?;
        let room_id = create_test_room(&self.server, &access_token, "Throughput Test Room").await?;
        
        let start_time = Instant::now();
        let mut successful_messages = 0;
        
        for i in 0..message_count {
            let message_body = json!({
                "msgtype": "m.text",
                "body": format!("Throughput test message {}", i)
            });
            
            let txn_id = uuid::Uuid::new_v4().to_string();
            let path = format!("/_matrix/client/v3/rooms/{}/send/m.room.message/{}", room_id, txn_id);
            
            let response = self.server.test_authenticated_endpoint("PUT", &path, &access_token, Some(message_body)).await;
            
            if response.status_code() == 200 {
                successful_messages += 1;
            }
            
            // Small delay to prevent overwhelming the server
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        
        let duration = start_time.elapsed();
        
        Ok(ThroughputReport {
            total_messages: message_count,
            successful_messages,
            failed_messages: message_count - successful_messages,
            duration,
            messages_per_second: successful_messages as f64 / duration.as_secs_f64(),
        })
    }
    
    pub async fn test_sync_performance(&self) -> Result<SyncPerformanceReport, Box<dyn std::error::Error>> {
        // Create test user
        let (user_id, access_token) = create_test_user(&self.server, "sync_perf_user", "password").await?;
        
        let mut sync_times = Vec::new();
        let test_iterations = 10;
        
        for _ in 0..test_iterations {
            let start_time = Instant::now();
            
            let response = self.server.test_authenticated_endpoint("GET", "/_matrix/client/v3/sync", &access_token, None).await;
            
            let sync_time = start_time.elapsed();
            
            if response.status_code() == 200 {
                sync_times.push(sync_time);
            }
            
            // Small delay between sync requests
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        let average_sync_time = sync_times.iter().sum::<Duration>() / sync_times.len() as u32;
        let min_sync_time = sync_times.iter().min().copied().unwrap_or_default();
        let max_sync_time = sync_times.iter().max().copied().unwrap_or_default();
        
        Ok(SyncPerformanceReport {
            total_syncs: test_iterations,
            successful_syncs: sync_times.len() as u32,
            average_response_time: average_sync_time,
            min_response_time: min_sync_time,
            max_response_time: max_sync_time,
        })
    }
}

async fn simulate_user_session(
    server: &MatrixTestServer, 
    username: &str, 
    metrics: &PerformanceMetrics
) -> Result<(), Box<dyn std::error::Error>> {
    let start_time = Instant::now();
    
    // Register user
    let (user_id, access_token) = create_test_user(server, username, "loadtest_password").await?;
    metrics.requests_sent.fetch_add(1, Ordering::Relaxed);
    
    // Create room
    let room_id = create_test_room(server, &access_token, &format!("{}'s Room", username)).await?;
    metrics.requests_sent.fetch_add(1, Ordering::Relaxed);
    
    // Send messages
    for i in 0..5 {
        let message_body = json!({
            "msgtype": "m.text",
            "body": format!("Load test message {} from {}", i, username)
        });
        
        let txn_id = uuid::Uuid::new_v4().to_string();
        let path = format!("/_matrix/client/v3/rooms/{}/send/m.room.message/{}", room_id, txn_id);
        
        let response = server.test_authenticated_endpoint("PUT", &path, &access_token, Some(message_body)).await;
        
        if response.status_code() == 200 {
            metrics.messages_sent.fetch_add(1, Ordering::Relaxed);
        }
        metrics.requests_sent.fetch_add(1, Ordering::Relaxed);
    }
    
    // Perform sync
    let response = server.test_authenticated_endpoint("GET", "/_matrix/client/v3/sync", &access_token, None).await;
    metrics.requests_sent.fetch_add(1, Ordering::Relaxed);
    
    let session_duration = start_time.elapsed();
    metrics.total_response_time_ms.fetch_add(session_duration.as_millis() as u64, Ordering::Relaxed);
    
    Ok(())
}

#[derive(Clone)]
pub struct PerformanceMetrics {
    pub messages_sent: Arc<AtomicU64>,
    pub requests_sent: Arc<AtomicU64>,
    pub total_response_time_ms: Arc<AtomicU64>,
}

impl PerformanceMetrics {
    pub fn new() -> Self {
        Self {
            messages_sent: Arc::new(AtomicU64::new(0)),
            requests_sent: Arc::new(AtomicU64::new(0)),
            total_response_time_ms: Arc::new(AtomicU64::new(0)),
        }
    }
}

#[derive(Debug)]
pub struct PerformanceReport {
    pub user_count: u32,
    pub successful_users: u32,
    pub failed_users: u32,
    pub duration: Duration,
    pub messages_per_second: f64,
    pub requests_per_second: f64,
    pub average_response_time_ms: f64,
}

#[derive(Debug)]
pub struct ThroughputReport {
    pub total_messages: u32,
    pub successful_messages: u32,
    pub failed_messages: u32,
    pub duration: Duration,
    pub messages_per_second: f64,
}

#[derive(Debug)]
pub struct SyncPerformanceReport {
    pub total_syncs: u32,
    pub successful_syncs: u32,
    pub average_response_time: Duration,
    pub min_response_time: Duration,
    pub max_response_time: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_small_load() {
        let load_test = LoadTest::new().await.unwrap();
        let report = load_test.test_concurrent_users(5).await.unwrap();
        
        println!("Load test report: {:?}", report);
        
        // Should handle at least 5 concurrent users
        assert!(report.successful_users > 0, "Should have some successful users");
        assert!(report.requests_per_second > 0.0, "Should have some request throughput");
    }
    
    #[tokio::test]
    async fn test_message_throughput() {
        let load_test = LoadTest::new().await.unwrap();
        let report = load_test.test_message_throughput(20).await.unwrap();
        
        println!("Throughput report: {:?}", report);
        
        // Should successfully send most messages
        assert!(report.successful_messages > 10, "Should send most messages successfully");
        assert!(report.messages_per_second > 0.0, "Should have positive throughput");
    }
    
    #[tokio::test]
    async fn test_sync_performance() {
        let load_test = LoadTest::new().await.unwrap();
        let report = load_test.test_sync_performance().await.unwrap();
        
        println!("Sync performance report: {:?}", report);
        
        // Should have reasonable sync performance
        assert!(report.successful_syncs > 0, "Should have successful syncs");
        assert!(report.average_response_time < Duration::from_secs(5), 
                "Sync should be reasonably fast");
    }
}