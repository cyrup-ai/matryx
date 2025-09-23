use chrono::Utc;
use futures::stream::{Stream, StreamExt};
use serde_json::json;

use crate::state::AppState;
use matryx_entity::sync::PresenceUpdate;

use super::super::types::*;

pub async fn create_presence_live_stream(
    state: AppState,
    user_id: String,
) -> Result<
    impl Stream<Item = Result<LiveSyncUpdate, Box<dyn std::error::Error + Send + Sync>>>,
    Box<dyn std::error::Error + Send + Sync>,
> {
    // Create LiveQuery for presence changes affecting this user's contacts
    let mut stream = state
        .db
        .query(
            r#"
            LIVE SELECT * FROM presence_events
            WHERE user_id IN (
                SELECT VALUE target_user_id FROM user_relationships
                WHERE user_id = $user_id AND relationship_type = 'friend'
            )
            OR user_id = $user_id
        "#,
        )
        .bind(("user_id", user_id.clone()))
        .await?;

    let sync_stream = stream.stream::<surrealdb::Notification<serde_json::Value>>(0)?
        .map(move |notification_result| -> Result<LiveSyncUpdate, Box<dyn std::error::Error + Send + Sync>> {
            let notification = notification_result?;

            match notification.action {
                surrealdb::Action::Create | surrealdb::Action::Update => {
                    let presence_data = notification.data;

                    let presence_event = json!({
                        "type": "m.presence",
                        "sender": presence_data.get("user_id").and_then(|v| v.as_str()).unwrap_or(""),
                        "content": {
                            "presence": presence_data.get("presence").and_then(|v| v.as_str()).unwrap_or("offline"),
                            "status_msg": presence_data.get("status_msg"),
                            "last_active_ago": presence_data.get("last_active_ago"),
                            "currently_active": presence_data.get("currently_active").and_then(|v| v.as_bool()).unwrap_or(false)
                        }
                    });

                    Ok(LiveSyncUpdate {
                        next_batch: format!("s{}", Utc::now().timestamp_millis()),
                        rooms: None,
                        presence: Some(PresenceUpdate {
                            events: vec![presence_event],
                        }),
                        account_data: None,
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
