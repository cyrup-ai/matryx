use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, error};
use crate::AppState;

/// Background task to clean up expired typing notifications
/// Runs every 60 seconds to remove typing events that have expired
pub async fn start_typing_cleanup_task(state: AppState) {
    let mut interval = interval(Duration::from_secs(60));

    loop {
        interval.tick().await;

        let query = "DELETE typing_notification WHERE expires_at < time::now()";

        match state.db.query(query).await {
            Ok(_) => debug!("Cleaned up expired typing events"),
            Err(e) => error!("Failed to cleanup typing events: {}", e),
        }
    }
}
