use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use futures::stream::{Stream, StreamExt};

use crate::auth::AuthenticatedUser;
use crate::state::AppState;
use matryx_entity::types::MatrixFilter;
use matryx_surrealdb::repository::{FilterLiveUpdate, FilterRepository};

use super::super::types::SyncQuery;

/// Handle real-time filter updates for Matrix sync
#[allow(dead_code)]
pub async fn handle_filter_live_updates(
    state: &AppState,
    user_id: &str,
    _filter_id: Option<&str>,
) -> Result<impl Stream<Item = MatrixFilter>, Box<dyn std::error::Error + Send + Sync>> {
    let db = state.db.clone();
    let user_id_owned = user_id.to_string();

    let live_stream = async_stream::stream! {
        let filter_repo = FilterRepository::new(db);
        let mut stream = filter_repo.subscribe_user(user_id_owned);

        while let Some(result) = stream.next().await {
            match result {
                Ok(FilterLiveUpdate::Created(filter)) => yield filter,
                Ok(FilterLiveUpdate::Updated { new, .. }) => yield *new,
                Ok(FilterLiveUpdate::Deleted(_)) => {}, // Filter deleted, no yield
                Err(e) => {
                    tracing::error!("Filter live query error: {:?}", e);
                }
            }
        }
    };

    Ok(live_stream)
}

/// Enhanced sync endpoint with live filter integration
#[allow(dead_code)]
pub async fn get_with_live_filters(
    State(state): State<AppState>,
    headers: HeaderMap,
    auth: AuthenticatedUser,
    Query(query): Query<SyncQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let user_id = auth.user_id.clone();
    let filter_id_opt = query.filter.clone();

    // Get current filter (if specified)
    let _current_filter = if let Some(filter_id) = &filter_id_opt {
        let filter_repo = FilterRepository::new(state.db.clone());
        filter_repo
            .get_by_id(filter_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        None
    };

    // Start live filter updates stream
    let _filter_updates = handle_filter_live_updates(&state, &user_id, filter_id_opt.as_deref())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // TODO: Integrate filter_updates stream with sync response
    // This would require server-sent events or WebSocket upgrade for live updates

    // For now, return standard sync response with current filter
    use super::super::handlers::get;
    get(State(state.clone()), headers, auth, Query(query)).await
}
