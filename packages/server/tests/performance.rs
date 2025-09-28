mod common;
mod test_config;

use common::integration::performance::*;
use test_config::TestConfig;

#[tokio::test]
async fn test_concurrent_user_load() {
    let config = TestConfig::from_env();

    if !config.enable_performance_tests {
        println!("Performance tests disabled by configuration");
        return;
    }

    println!("Running load test with {} concurrent users", config.max_concurrent_users);

    let load_test = LoadTest::new().await.expect("Should be able to create load test");

    let report = load_test
        .test_concurrent_users(config.max_concurrent_users)
        .await
        .expect("Should be able to run load test");

    println!("Load Test Report:");
    println!("  User count: {}", report.user_count);
    println!("  Successful users: {}", report.successful_users);
    println!("  Failed users: {}", report.failed_users);
    println!("  Duration: {:.2}s", report.duration.as_secs_f64());
    println!("  Messages/sec: {:.2}", report.messages_per_second);
    println!("  Requests/sec: {:.2}", report.requests_per_second);
    println!("  Avg response time: {:.2}ms", report.average_response_time_ms);

    // Should handle at least half the users successfully
    assert!(
        report.successful_users >= report.user_count / 2,
        "Should handle at least half the users successfully"
    );

    // Should have reasonable throughput
    assert!(report.requests_per_second > 0.0, "Should have positive request throughput");

    println!("✅ Load test completed successfully");
}

#[tokio::test]
async fn test_message_throughput_performance() {
    let config = TestConfig::from_env();

    if !config.enable_performance_tests {
        println!("Performance tests disabled by configuration");
        return;
    }

    let load_test = LoadTest::new().await.expect("Should be able to create load test");

    let message_count = 50; // Reasonable number for CI
    let report = load_test
        .test_message_throughput(message_count)
        .await
        .expect("Should be able to test message throughput");

    println!("Message Throughput Report:");
    println!("  Total messages: {}", report.total_messages);
    println!("  Successful: {}", report.successful_messages);
    println!("  Failed: {}", report.failed_messages);
    println!("  Duration: {:.2}s", report.duration.as_secs_f64());
    println!("  Messages/sec: {:.2}", report.messages_per_second);

    // Should send most messages successfully
    let success_rate = (report.successful_messages as f64 / report.total_messages as f64) * 100.0;
    assert!(
        success_rate > 80.0,
        "Should send >80% of messages successfully, got {:.1}%",
        success_rate
    );

    // Should have reasonable throughput
    assert!(report.messages_per_second > 1.0, "Should send >1 message/sec");

    println!("✅ Message throughput test completed successfully");
}

#[tokio::test]
async fn test_sync_endpoint_performance() {
    let config = TestConfig::from_env();

    if !config.enable_performance_tests {
        println!("Performance tests disabled by configuration");
        return;
    }

    let load_test = LoadTest::new().await.expect("Should be able to create load test");

    let report = load_test
        .test_sync_performance()
        .await
        .expect("Should be able to test sync performance");

    println!("Sync Performance Report:");
    println!("  Total syncs: {}", report.total_syncs);
    println!("  Successful: {}", report.successful_syncs);
    println!("  Average response time: {:.2}ms", report.average_response_time.as_millis());
    println!("  Min response time: {:.2}ms", report.min_response_time.as_millis());
    println!("  Max response time: {:.2}ms", report.max_response_time.as_millis());

    // Should have successful syncs
    assert!(report.successful_syncs > 0, "Should have successful sync operations");

    // Should have reasonable performance (less than 5 seconds average)
    assert!(
        report.average_response_time.as_secs() < 5,
        "Sync should be reasonably fast, got {:.2}s",
        report.average_response_time.as_secs_f64()
    );

    println!("✅ Sync performance test completed successfully");
}
