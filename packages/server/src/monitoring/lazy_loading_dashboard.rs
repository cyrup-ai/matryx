use matryx_surrealdb::repository::MonitoringService;
use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::{Html, Json},
    routing::get,
};
use serde_json::json;
use std::sync::Arc;
use surrealdb::engine::any::Any;

/// Production dashboard for monitoring Matrix lazy loading performance
pub struct LazyLoadingDashboard {
    monitoring_service: Arc<MonitoringService<Any>>,
}

impl LazyLoadingDashboard {
    pub fn new(monitoring_service: Arc<MonitoringService<Any>>) -> Self {
        Self { monitoring_service }
    }

    /// Create router for dashboard endpoints
    pub fn router(&self) -> Router<Arc<Self>> {
        Router::new()
            .route("/dashboard", get(serve_dashboard))
            .route("/api/metrics", get(get_metrics_json))
            .route("/api/cache-stats", get(get_cache_stats))
            .route("/api/health", get(get_health_status))
            .route("/prometheus", get(get_prometheus_metrics))
    }
}

/// Serve HTML dashboard for monitoring lazy loading performance
async fn serve_dashboard() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}

/// Get current metrics as JSON for API consumers
async fn get_metrics_json(
    State(dashboard): State<Arc<LazyLoadingDashboard>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let dashboard_data = dashboard.monitoring_service
        .generate_dashboard_data("performance")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(json!({
        "lazy_loading_performance": dashboard_data.data,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Get cache statistics
async fn get_cache_stats(
    State(dashboard): State<Arc<LazyLoadingDashboard>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let resource_usage = dashboard.monitoring_service
        .track_resource_usage()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(json!({
        "cache_statistics": {
            "memory_usage_mb": resource_usage.memory_mb,
            "cpu_percentage": resource_usage.cpu_percentage,
            "disk_usage_mb": resource_usage.disk_usage_mb,
            "network_bytes_per_sec": resource_usage.network_bytes_per_sec,
            "timestamp": resource_usage.timestamp
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Health status endpoint for monitoring systems
async fn get_health_status(
    State(dashboard): State<Arc<LazyLoadingDashboard>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let dashboard_data = dashboard.monitoring_service
        .generate_dashboard_data("overview")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Extract system health from dashboard data
    let system_health = dashboard_data.data.get("system_health");
    let performance_summary = dashboard_data.data.get("performance_summary");
    
    let status = if let Some(health) = system_health {
        match health.get("overall_status").and_then(|s| s.as_str()) {
            Some("Healthy") => "healthy",
            Some("Degraded") => "degraded", 
            Some("Unhealthy") => "unhealthy",
            _ => "unknown"
        }
    } else {
        "unknown"
    };

    Ok(Json(json!({
        "status": status,
        "system_health": system_health,
        "performance_summary": performance_summary,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Prometheus metrics endpoint for integration with monitoring systems
async fn get_prometheus_metrics(
    State(dashboard): State<Arc<LazyLoadingDashboard>>,
) -> Result<String, StatusCode> {
    let metrics = dashboard.monitoring_service
        .export_prometheus_metrics()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(metrics)
}

/// HTML dashboard template for real-time monitoring
const DASHBOARD_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Matryx Lazy Loading Performance Dashboard</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            margin: 0;
            padding: 20px;
            background: #f5f5f5;
        }
        .container { max-width: 1200px; margin: 0 auto; }
        .header { background: #2d3748; color: white; padding: 20px; border-radius: 8px; margin-bottom: 20px; }
        .metrics-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 20px; }
        .metric-card {
            background: white;
            border-radius: 8px;
            padding: 20px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        .metric-title { font-size: 14px; color: #666; margin-bottom: 8px; }
        .metric-value { font-size: 32px; font-weight: bold; margin-bottom: 8px; }
        .metric-trend { font-size: 12px; color: #999; }
        .status-good { color: #22c55e; }
        .status-warning { color: #f59e0b; }
        .status-error { color: #ef4444; }
        .refresh-info { text-align: center; margin-top: 20px; color: #666; font-size: 14px; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🚀 Matryx Lazy Loading Performance Dashboard</h1>
            <p>Real-time monitoring of Matrix lazy loading optimization</p>
        </div>

        <div class="metrics-grid" id="metrics-grid">
            <!-- Metrics will be populated by JavaScript -->
        </div>

        <div class="refresh-info">
            <p>Dashboard auto-refreshes every 10 seconds</p>
            <p>Last updated: <span id="last-updated">Loading...</span></p>
        </div>
    </div>

    <script>
        async function updateMetrics() {
            try {
                const [metricsResponse, cacheResponse, healthResponse] = await Promise.all([
                    fetch('/api/metrics'),
                    fetch('/api/cache-stats'),
                    fetch('/api/health')
                ]);

                const metrics = await metricsResponse.json();
                const cache = await cacheResponse.json();
                const health = await healthResponse.json();

                const perf = metrics.lazy_loading_performance;
                const cacheStats = cache.cache_statistics;

                const grid = document.getElementById('metrics-grid');
                grid.innerHTML = `
                    <div class="metric-card">
                        <div class="metric-title">Cache Hit Ratio</div>
                        <div class="metric-value ${perf.cache_hit_ratio > 0.8 ? 'status-good' : perf.cache_hit_ratio > 0.6 ? 'status-warning' : 'status-error'}">
                            ${(perf.cache_hit_ratio * 100).toFixed(1)}%
                        </div>
                        <div class="metric-trend">Target: >80%</div>
                    </div>

                    <div class="metric-card">
                        <div class="metric-title">Avg Processing Time</div>
                        <div class="metric-value ${perf.avg_processing_time_us < 50000 ? 'status-good' : perf.avg_processing_time_us < 100000 ? 'status-warning' : 'status-error'}">
                            ${(perf.avg_processing_time_us / 1000).toFixed(1)}ms
                        </div>
                        <div class="metric-trend">Target: <50ms</div>
                    </div>

                    <div class="metric-card">
                        <div class="metric-title">Total Requests</div>
                        <div class="metric-value">${perf.total_requests.toLocaleString()}</div>
                        <div class="metric-trend">All time</div>
                    </div>

                    <div class="metric-card">
                        <div class="metric-title">Members Filtered</div>
                        <div class="metric-value">${perf.total_members_filtered.toLocaleString()}</div>
                        <div class="metric-trend">Optimization efficiency</div>
                    </div>

                    <div class="metric-card">
                        <div class="metric-title">DB Queries Avoided</div>
                        <div class="metric-value status-good">${perf.db_queries_avoided.toLocaleString()}</div>
                        <div class="metric-trend">Cache effectiveness</div>
                    </div>

                    <div class="metric-card">
                        <div class="metric-title">Cache Memory Usage</div>
                        <div class="metric-value ${perf.estimated_memory_usage_kb < 50000 ? 'status-good' : perf.estimated_memory_usage_kb < 100000 ? 'status-warning' : 'status-error'}">
                            ${(perf.estimated_memory_usage_kb / 1024).toFixed(1)} MB
                        </div>
                        <div class="metric-trend">Target: <100MB</div>
                    </div>

                    <div class="metric-card">
                        <div class="metric-title">Total Cache Entries</div>
                        <div class="metric-value">${cacheStats.total_cache_entries.toLocaleString()}</div>
                        <div class="metric-trend">All cache types</div>
                    </div>

                    <div class="metric-card">
                        <div class="metric-title">System Health</div>
                        <div class="metric-value ${health.status === 'healthy' ? 'status-good' : 'status-warning'}">
                            ${health.status.toUpperCase()}
                        </div>
                        <div class="metric-trend">Overall status</div>
                    </div>
                `;

                document.getElementById('last-updated').textContent = new Date().toLocaleTimeString();
            } catch (error) {
                console.error('Failed to update metrics:', error);
                document.getElementById('last-updated').textContent = 'Error loading data';
            }
        }

        // Initial load
        updateMetrics();

        // Auto-refresh every 10 seconds
        setInterval(updateMetrics, 10000);
    </script>
</body>
</html>"#;
