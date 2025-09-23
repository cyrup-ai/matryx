/// Get timeline events for a room with optional filtering
pub async fn get_room_timeline_events(
    state: &AppState,
    room_id: &str,
    limit: Option<u32>,
) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
    let event_repo = EventRepository::new(state.db.clone());
    let events = event_repo
        .get_room_events(room_id, limit)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    Ok(events)
}

/// Get rooms where user has joined membership
pub async fn get_joined_rooms(
    state: &AppState,
    user_id: &str,
) -> Result<Vec<Membership>, Box<dyn std::error::Error + Send + Sync>> {
    let membership_repo = MembershipRepository::new(state.db.clone());
    let memberships = membership_repo
        .get_user_rooms_by_state(user_id, "join")
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    Ok(memberships)
}

/// Get rooms where user has invited membership
pub async fn get_invited_rooms(
    state: &AppState,
    user_id: &str,
) -> Result<Vec<Membership>, Box<dyn std::error::Error + Send + Sync>> {
    let membership_repo = MembershipRepository::new(state.db.clone());
    let memberships = membership_repo
        .get_user_rooms_by_state(user_id, "invite")
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    Ok(memberships)
}

/// Get rooms where user has left membership
pub async fn get_left_rooms(
    state: &AppState,
    user_id: &str,
) -> Result<Vec<Membership>, Box<dyn std::error::Error + Send + Sync>> {
    let membership_repo = MembershipRepository::new(state.db.clone());
    let memberships = membership_repo
        .get_user_rooms_by_state(user_id, "leave")
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    Ok(memberships)
}