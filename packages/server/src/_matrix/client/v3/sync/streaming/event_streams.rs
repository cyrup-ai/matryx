use std::collections::HashMap;

use chrono::Utc;
use futures::stream::{Stream, StreamExt};
use serde_json::json;

use crate::state::AppState;
use matryx_entity::types::{
    AccountData,
    AccountDataUpdate,
    JoinedRoomUpdate,
    LiveSyncUpdate,
    RoomsUpdate,
    TimelineUpdate,
};
use matryx_surrealdb::repository::{EventRepository, AccountDataRepository};

pub async fn create_event_live_stream(
    state: AppState,
    user_id: String,
) -> Result<
    impl Stream<Item = Result<LiveSyncUpdate, Box<dyn std::error::Error + Send + Sync>>>,
    Box<dyn std::error::Error + Send + Sync>,
> {
    // Clone the database connection to be owned by the stream
    let db_connection = state.db.clone();

    // Create the stream using async-stream to handle the async repository creation
    let sync_stream = async_stream::stream! {
        // Create EventRepository inside the stream
        let event_repo = EventRepository::new(db_connection);
        let mut event_stream = match event_repo.subscribe_user_events(&user_id).await {
            Ok(stream) => stream,
            Err(e) => {
                yield Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>);
                return;
            }
        };

        while let Some(event_result) = event_stream.next().await {
            let result: Result<LiveSyncUpdate, Box<dyn std::error::Error + Send + Sync>> = (|| {
                let event = event_result
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

                // Create sync update for this event
                let event_json = json!({
                    "event_id": event.event_id,
                    "sender": event.sender,
                    "origin_server_ts": event.origin_server_ts,
                    "type": event.event_type,
                    "content": event.content,
                    "unsigned": event.unsigned
                });

                let mut joined_rooms = HashMap::new();
                joined_rooms.insert(event.room_id.clone(), JoinedRoomUpdate {
                    timeline: Some(TimelineUpdate {
                        events: vec![event_json],
                        limited: Some(false),
                        prev_batch: None,
                    }),
                    state: None,
                    ephemeral: None,
                    account_data: None,
                    unread_notifications: None,
                });

                Ok(LiveSyncUpdate {
                    next_batch: format!("s{}", Utc::now().timestamp_millis()),
                    rooms: Some(RoomsUpdate {
                        join: Some(joined_rooms),
                        invite: None,
                        leave: None,
                    }),
                    presence: None,
                    account_data: None,
                    to_device: None,
                    device_lists: None,
                })
            })();

            yield result;
        }
    };

    Ok(sync_stream)
}

pub async fn create_account_data_live_stream(
    state: AppState,
    user_id: String,
) -> Result<
    impl Stream<Item = Result<LiveSyncUpdate, Box<dyn std::error::Error + Send + Sync>>>,
    Box<dyn std::error::Error + Send + Sync>,
> {
    // Create LiveQuery for account data changes
    let account_data_repo = AccountDataRepository::new(state.db.clone());
    let mut stream = account_data_repo
        .create_account_data_live_query(&user_id)
        .await?;

    let sync_stream = stream.stream::<surrealdb::Notification<AccountData>>(0)?
        .map(move |notification_result| -> Result<LiveSyncUpdate, Box<dyn std::error::Error + Send + Sync>> {
            let notification = notification_result?;

            match notification.action {
                surrealdb::Action::Create | surrealdb::Action::Update => {
                    let account_data = notification.data;

                    let account_event = json!({
                        "type": account_data.account_data_type,
                        "content": account_data.content
                    });

                    Ok(LiveSyncUpdate {
                        next_batch: format!("s{}", Utc::now().timestamp_millis()),
                        rooms: None,
                        presence: None,
                        account_data: Some(AccountDataUpdate {
                            events: vec![account_event],
                        }),
                        to_device: None,
                        device_lists: None,
                    })
                },
                _ => Ok(LiveSyncUpdate {
                    next_batch: format!("s{}", Utc::now().timestamp_millis()),
                    rooms: None,
                    presence: None,
                    account_data: None,
                    to_device: None,
                    device_lists: None,
                }),
            }
        });

    Ok(sync_stream)
}
