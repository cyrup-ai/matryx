use std::collections::HashMap;

use async_stream;
use chrono::Utc;
use futures::stream::{Stream, StreamExt, TryStreamExt};
use serde_json::json;

use crate::cache::lazy_loading_cache::LazyLoadingCache;
use crate::room::LiveMembershipService;
use crate::state::AppState;
use matryx_entity::types::MembershipState;

use super::super::types::*;

pub async fn create_enhanced_membership_stream(
    state: AppState,
    user_id: String,
) -> Result<
    impl Stream<Item = Result<LiveSyncUpdate, Box<dyn std::error::Error + Send + Sync>>> + use<>,
    Box<dyn std::error::Error + Send + Sync>,
> {
    // Clone the user_id to avoid borrowing issues
    let user_id_owned = user_id.clone();
    let db_clone = state.db.clone();

    // Create a membership stream using direct DB query to avoid lifetime issues
    let membership_stream = async move {
        // Create SurrealDB LiveQuery for memberships
        let mut stream = db_clone
            .query("LIVE SELECT * FROM membership")
            .await
            .map_err(|e| format!("Failed to create live query: {:?}", e))?;

        // Transform the stream
        let result_stream = stream
            .stream::<surrealdb::Notification<matryx_entity::types::Membership>>(0)
            .map_err(|e| format!("Database error: {:?}", e))?
            .map(|notification_result| -> Result<matryx_entity::types::Membership, String> {
                let notification =
                    notification_result.map_err(|e| format!("Notification error: {:?}", e))?;
                Ok(notification.data)
            });

        Ok::<_, String>(result_stream)
    }
    .await?;

    // Transform membership stream to sync updates
    let sync_stream = membership_stream.map(
        move |membership_result| -> Result<LiveSyncUpdate, Box<dyn std::error::Error + Send + Sync>> {
            let _memberships = membership_result
                .map_err(|e| format!("Membership stream error: {:?}", e))?;

            // Create basic sync update with membership changes
            Ok(LiveSyncUpdate {
                next_batch: format!("s{}", Utc::now().timestamp_millis()),
                rooms: Some(RoomsUpdate {
                    join: None,
                    invite: None,
                    leave: None,
                }),
                presence: None,
                account_data: None,
                to_device: None,
                device_lists: None,
            })
        },
    );

    Ok(sync_stream)
}

/// Integration with LiveMembershipService for real-time updates
pub async fn integrate_live_membership_with_lazy_loading(
    state: &AppState,
    room_id: &str,
    user_id: &str,
    lazy_cache: &LazyLoadingCache,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Spawn background task to handle membership changes
    let room_id_owned = room_id.to_string();
    let user_id_owned = user_id.to_string();
    let lazy_cache_clone = std::sync::Arc::new(lazy_cache.clone());
    let db_clone = std::sync::Arc::new(state.db.clone());

    tokio::spawn(async move {
        let live_membership = LiveMembershipService::new(db_clone);

        let membership_stream: std::pin::Pin<
            Box<
                dyn futures::stream::Stream<
                        Item = Result<
                            crate::room::FilteredMembershipUpdate,
                            axum::http::StatusCode,
                        >,
                    > + Send,
            >,
        > = match live_membership
            .create_room_membership_stream(&room_id_owned, &user_id_owned)
            .await
        {
            Ok(stream) => Box::pin(stream),
            Err(e) => {
                tracing::error!("Failed to create membership stream: {:?}", e);
                return;
            },
        };
        let mut membership_stream = membership_stream;
        while let Some(update_result) = membership_stream.next().await {
            match update_result {
                Ok(_update) => {
                    // Invalidate cache when membership changes
                    lazy_cache_clone.invalidate_room_cache(&room_id_owned).await;
                },
                Err(e) => {
                    tracing::error!("Membership stream error: {:?}", e);
                },
            }
        }
    });

    Ok(())
}
