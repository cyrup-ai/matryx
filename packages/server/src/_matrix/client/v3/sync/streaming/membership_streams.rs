


use chrono::Utc;
use futures::stream::{Stream, StreamExt};


use crate::cache::lazy_loading_cache::LazyLoadingCache;
use crate::room::LiveMembershipService;
use crate::state::AppState;
// Function is defined in this module, no import needed
use matryx_surrealdb::repository::MembershipRepository;


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
    
    // Create lazy loading cache for membership optimization
    let lazy_cache = LazyLoadingCache::new();

    // Create a membership stream using repository to avoid lifetime issues
    let membership_stream = async move {
        // Create SurrealDB LiveQuery for memberships for the specific user
        let membership_repo = MembershipRepository::new(db_clone);
        let mut stream = membership_repo
            .create_user_membership_simple_live_query(&user_id_owned)
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

    // Integrate lazy loading with membership updates for all user's rooms
    // This runs in the background to optimize membership loading
    let state_clone = state.clone();
    let user_id_for_lazy = user_id.clone();
    let lazy_cache_clone = lazy_cache.clone();
    
    tokio::spawn(async move {
        // Get user's rooms and integrate lazy loading for each
        let membership_repo = MembershipRepository::new(state_clone.db.clone());
        if let Ok(user_memberships) = membership_repo.get_user_rooms(&user_id_for_lazy).await {
            for membership in user_memberships {
                if let Err(e) = integrate_live_membership_with_lazy_loading(
                    &state_clone,
                    &membership.room_id,
                    &user_id_for_lazy,
                    &lazy_cache_clone,
                ).await {
                    tracing::error!("Failed to integrate lazy loading for room {}: {}", membership.room_id, e);
                }
            }
        }
    });

    Ok(sync_stream)
}

/// Integration with LiveMembershipService for real-time updates
#[allow(dead_code)]
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
