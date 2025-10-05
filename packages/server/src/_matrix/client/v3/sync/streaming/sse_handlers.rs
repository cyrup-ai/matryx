use std::pin::Pin;

use axum::http::StatusCode;
use axum::response::Sse;
use futures::stream::{Stream, StreamExt};

use crate::auth::AuthenticatedUser;
use crate::state::AppState;

use super::super::filters::{apply_filter_to_update, create_live_filtered_stream, resolve_filter};
use super::super::types::{LiveSyncUpdate, SyncQuery};
use super::event_streams::{create_account_data_live_stream, create_event_live_stream};
use super::membership_streams::create_enhanced_membership_stream;
use super::presence_streams::create_presence_live_stream;

/// Server-Sent Events stream for live sync updates
pub async fn get_sse_stream(
    state: AppState,
    auth: AuthenticatedUser,
    query: SyncQuery,
) -> Result<Sse<impl Stream<Item = Result<axum::response::sse::Event, axum::Error>>>, StatusCode> {
    let user_id = auth.user_id.clone();

    // Resolve filter parameter for streaming
    let applied_filter = if let Some(filter_param) = &query.filter {
        resolve_filter(&state, filter_param, &user_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        None
    };

    // Create combined live sync stream with filter support
    let combined_stream = handle_live_sync_streams(state, user_id, query, applied_filter)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Convert to SSE format
    let sse_stream = combined_stream.map(|update_result| match update_result {
        Ok(update) => {
            let json_str = serde_json::to_string(&update).unwrap_or_else(|_| "{}".to_string());
            Ok(axum::response::sse::Event::default().event("sync").data(json_str))
        },
        Err(e) => {
            tracing::error!("SSE stream error: {:?}", e);
            Err(axum::Error::new(e))
        },
    });

    Ok(Sse::new(sse_stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(30))
            .text("keep-alive"),
    ))
}

/// Handle multiple live sync streams and merge them
async fn handle_live_sync_streams(
    state: AppState,
    user_id: String,
    _query: SyncQuery,
    filter: Option<matryx_entity::types::MatrixFilter>,
) -> Result<
    impl Stream<Item = Result<LiveSyncUpdate, Box<dyn std::error::Error + Send + Sync>>>,
    Box<dyn std::error::Error + Send + Sync>,
> {
    // Create individual live streams with filter support
    let event_stream = if let Some(ref filter_obj) = filter {
        create_live_filtered_stream(state.clone(), user_id.clone(), filter_obj.clone()).await?
    } else {
        Box::pin(create_event_live_stream(state.clone(), user_id.clone()).await?)
    };

    let account_data_stream =
        create_account_data_live_stream(state.clone(), user_id.clone()).await?;
    let presence_stream = create_presence_live_stream(state.clone(), user_id.clone()).await?;

    // Create enhanced membership stream
    let membership_stream = Some(create_enhanced_membership_stream(state, user_id).await?);

    // Type alias for complex stream type to improve readability
    type SyncUpdateStream = Pin<
        Box<
            dyn Stream<Item = Result<LiveSyncUpdate, Box<dyn std::error::Error + Send + Sync>>>
                + Send,
        >,
    >;

    // Merge all streams using select_all
    let mut combined_streams: Vec<SyncUpdateStream> = vec![
        Box::pin(event_stream),
        Box::pin(account_data_stream),
        Box::pin(presence_stream),
    ];

    if let Some(membership_stream) = membership_stream {
        combined_streams.push(Box::pin(membership_stream));
    }

    // Use futures::stream::select_all to merge all streams
    let merged_stream = futures::stream::select_all(combined_streams);

    // Apply live filter updates if filter is present
    let filtered_stream: SyncUpdateStream = if let Some(ref filter) = filter {
        let filter_clone = filter.clone();
        Box::pin(merged_stream.then(move |update_result| {
            let filter = filter_clone.clone();
            async move {
                match update_result {
                    Ok(update) => {
                        // Apply filter to the sync update
                        apply_filter_to_update(update, &filter).await
                    },
                    Err(e) => Err(e),
                }
            }
        }))
    } else {
        Box::pin(merged_stream)
    };

    Ok(filtered_stream)
}
