use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use matryx_surrealdb::repository::{MonitoringService, ApiRequest};
use std::sync::Arc;
use std::time::Instant;
use surrealdb::engine::any::Any;
use chrono::Utc;

/// Monitoring middleware for automatic metrics collection
pub async fn monitoring_middleware(
    State(monitoring_service): State<Arc<MonitoringService<Any>>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let start_time = Instant::now();
    let method = request.method().to_string();
    let uri = request.uri().path().to_string();
    
    // Create API request record
    let api_request = ApiRequest {
        endpoint: uri.clone(),
        method: method.clone(),
        user_id: None, // Could extract from headers/auth
        timestamp: Utc::now(),
    };
    
    // Process the request
    let response = next.run(request).await;
    
    // Record the request metrics
    let duration_ms = start_time.elapsed().as_millis() as f64;
    let status_code = response.status().as_u16();
    
    // Record in monitoring service (fire and forget)
    if let Err(e) = monitoring_service
        .record_api_request(&api_request, duration_ms, status_code)
        .await
    {
        tracing::warn!("Failed to record API request metrics: {}", e);
    }
    
    Ok(response)
}

/// Memory monitoring middleware
pub async fn memory_monitoring_middleware(
    State(monitoring_service): State<Arc<MonitoringService<Any>>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Track resource usage before and after request
    if let Ok(resource_usage) = monitoring_service.track_resource_usage().await {
        tracing::debug!(
            memory_mb = resource_usage.memory_mb,
            cpu_percent = resource_usage.cpu_percentage,
            "Resource usage tracked"
        );
    }
    
    let response = next.run(request).await;
    
    Ok(response)
}

/// Error rate tracking middleware
pub async fn error_tracking_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let response = next.run(request).await;
    
    // Log errors for monitoring
    if response.status().is_server_error() || response.status().is_client_error() {
        tracing::warn!(
            status = response.status().as_u16(),
            path = request.uri().path(),
            method = request.method().as_str(),
            "Request resulted in error status"
        );
    }
    
    Ok(response)
}