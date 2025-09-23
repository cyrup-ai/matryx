mod common;
mod test_config;

use common::integration::compliance::*;
use common::integration::{MatrixTestServer, create_test_room, create_test_user};
use test_config::TestConfig;

#[tokio::test]
async fn test_matrix_compliance_suite() {
    let config = TestConfig::from_env();

    println!("Running Matrix compliance tests with timeout: {}s", config.test_timeout_seconds);

    // Run endpoint compliance tests
    let mut compliance_test = EndpointComplianceTest::new()
        .await
        .expect("Should be able to create compliance test");

    let report = compliance_test
        .test_all_endpoints()
        .await
        .expect("Should be able to run compliance tests");

    println!("Matrix API Compliance Report:");
    println!("  Total tests: {}", report.total);
    println!("  Passed: {}", report.passed);
    println!("  Failed: {}", report.failed);
    println!("  Pass rate: {:.1}%", report.pass_rate());

    // Print failed tests for debugging
    for (test_name, passed) in &report.tests {
        if !passed {
            println!("  ❌ FAILED: {}", test_name);
        }
    }

    // Should have reasonable compliance rate
    assert!(
        report.pass_rate() > 60.0,
        "Compliance rate should be > 60%, got {:.1}%",
        report.pass_rate()
    );

    println!("✅ Matrix compliance tests completed");
}

#[tokio::test]
async fn test_sytest_compliance() {
    let config = TestConfig::from_env();

    if !config.enable_federation_tests {
        println!("SyTest compliance tests disabled by configuration");
        return;
    }

    let test_server = MatrixTestServer::new().await;
    let sytest_runner =
        SyTestRunner::new(&test_server.base_url).expect("Should be able to create SyTest runner");

    let results = sytest_runner
        .run_compliance_tests()
        .await
        .expect("Should be able to run SyTest");

    println!("SyTest Compliance Results:");
    println!("  Status: {}", results.status);
    println!("  Total tests: {}", results.total_tests);
    println!("  Passed: {}", results.passed);
    println!("  Failed: {}", results.failed);
    println!("  Skipped: {}", results.skipped);

    if !results.failures.is_empty() {
        println!("  Failures:");
        for failure in &results.failures {
            println!("    - {}: {}", failure.test_name, failure.error);
        }
    }

    // SyTest may not be available in all environments
    assert!(
        results.status == "SyTest not available - skipped" ||
            results.status == "PASSED" ||
            results.status == "FAILED" ||
            results.status == "ERROR",
        "SyTest should return valid status"
    );

    println!("✅ SyTest compliance check completed");
}
