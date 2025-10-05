#[cfg(test)]
mod sync_tests {
    use crate::repository::{
        SyncRepository, SyncService, PresenceRepository, PresenceState,
        RoomRepository, MembershipRepository,
    };
    use surrealdb::{Surreal, engine::any::Any};
    use std::sync::Arc;
    use chrono::Utc;

    async fn setup_test_db() -> Surreal<Any> {
        let db = surrealdb::engine::any::connect("surrealkv://test_data/sync_test.db")
            .await
            .expect("Failed to connect to test database");
        db.use_ns("test")
            .use_db("test")
            .await
            .expect("Failed to set test database namespace");
        db
    }

    async fn create_sync_service() -> SyncService {
        let db = setup_test_db().await;
        let sync_repo = Arc::new(SyncRepository::new(db.clone()));
        let presence_repo = Arc::new(PresenceRepository::new(db.clone()));
        let room_repo = Arc::new(RoomRepository::new(db.clone()));
        let membership_repo = Arc::new(MembershipRepository::new(db.clone()));
        
        SyncService::new(sync_repo, presence_repo, room_repo, membership_repo)
    }

    #[tokio::test]
    async fn test_sync_repository_room_member_count() {
        let db = setup_test_db().await;
        let sync_repo = SyncRepository::new(db);
        
        let room_id = "!test:example.com";
        
        // Test getting member count for empty room
        let count = sync_repo
            .get_room_member_count(room_id)
            .await
            .expect("Failed to get room member count");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_sync_repository_invited_member_count() {
        let db = setup_test_db().await;
        let sync_repo = SyncRepository::new(db);
        
        let room_id = "!test:example.com";
        
        // Test getting invited member count for empty room
        let count = sync_repo
            .get_room_invited_member_count(room_id)
            .await
            .expect("Failed to get room invited member count");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_sync_repository_timeline_events() {
        let db = setup_test_db().await;
        let sync_repo = SyncRepository::new(db);
        
        let room_id = "!test:example.com";
        
        // Test getting timeline events for empty room
        let events = sync_repo
            .get_room_timeline_events(room_id, None)
            .await
            .expect("Failed to get room timeline events");
        assert_eq!(events.len(), 0);
    }

    #[tokio::test]
    async fn test_sync_repository_state_events() {
        let db = setup_test_db().await;
        let sync_repo = SyncRepository::new(db);
        
        let room_id = "!test:example.com";
        
        // Test getting state events for empty room
        let events = sync_repo
            .get_room_state_events(room_id, None)
            .await
            .expect("Failed to get room state events");
        assert_eq!(events.len(), 0);
    }

    #[tokio::test]
    async fn test_sync_repository_ephemeral_events() {
        let db = setup_test_db().await;
        let sync_repo = SyncRepository::new(db);
        
        let room_id = "!test:example.com";
        
        // Test getting ephemeral events for empty room
        let events = sync_repo
            .get_room_ephemeral_events(room_id, None)
            .await
            .expect("Failed to get room ephemeral events");
        assert_eq!(events.len(), 0);
    }

    #[tokio::test]
    async fn test_sync_repository_room_account_data() {
        let db = setup_test_db().await;
        let sync_repo = SyncRepository::new(db);
        
        let user_id = "@test:example.com";
        let room_id = "!test:example.com";
        
        // Test getting room account data for empty room
        let events = sync_repo
            .get_room_account_data_events(user_id, room_id, None)
            .await
            .expect("Failed to get room account data events");
        assert_eq!(events.len(), 0);
    }

    #[tokio::test]
    async fn test_sync_repository_unread_notifications() {
        let db = setup_test_db().await;
        let sync_repo = SyncRepository::new(db);
        
        let user_id = "@test:example.com";
        let room_id = "!test:example.com";
        
        // Test getting unread notifications for empty room
        let counts = sync_repo
            .get_room_unread_notifications(user_id, room_id)
            .await
            .expect("Failed to get room unread notifications");
        assert_eq!(counts.highlight_count, Some(0));
        assert_eq!(counts.notification_count, Some(0));
    }

    #[tokio::test]
    async fn test_presence_repository_user_presence_events() {
        let db = setup_test_db().await;
        let presence_repo = PresenceRepository::new(db);
        
        let user_id = "@test:example.com";
        
        // Test getting presence events for user with no presence data
        let events = presence_repo
            .get_user_presence_events(user_id, None)
            .await
            .expect("Failed to get user presence events");
        assert_eq!(events.len(), 0);
    }

    #[tokio::test]
    async fn test_presence_repository_update_user_presence() {
        let db = setup_test_db().await;
        let presence_repo = PresenceRepository::new(db);
        
        let user_id = "@test:example.com";
        
        // Test updating user presence
        let result = presence_repo.update_user_presence_state(user_id, PresenceState::Online, Some("Available")).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_presence_repository_get_user_last_active() {
        let db = setup_test_db().await;
        let presence_repo = PresenceRepository::new(db);
        
        let user_id = "@test:example.com";
        
        // Test getting last active time for user with no presence data
        let last_active = presence_repo
            .get_user_last_active(user_id)
            .await
            .expect("Failed to get user last active time");
        assert!(last_active.is_none());
    }

    #[tokio::test]
    async fn test_presence_repository_cleanup_old_events() {
        let db = setup_test_db().await;
        let presence_repo = PresenceRepository::new(db);
        
        let cutoff = Utc::now();
        
        // Test cleanup of old presence events
        let cleaned_count = presence_repo
            .cleanup_old_presence_events(cutoff)
            .await
            .expect("Failed to cleanup old presence events");
        assert_eq!(cleaned_count, 0); // No events to clean in empty database
    }

    #[tokio::test]
    async fn test_sync_service_full_sync_response() {
        let sync_service = create_sync_service().await;
        
        let user_id = "@test:example.com";
        
        // Test getting full sync response
        let result = sync_service.get_full_sync_response(user_id, None).await;
        assert!(result.is_ok());
        
        let sync_response = result.expect("Expected full sync response");
        assert!(!sync_response.next_batch.is_empty());
    }

    #[tokio::test]
    async fn test_sync_service_presence_sync_data() {
        let sync_service = create_sync_service().await;
        
        let user_id = "@test:example.com";
        
        // Test getting presence sync data
        let result = sync_service.get_presence_sync_data(user_id, None).await;
        assert!(result.is_ok());
        
        let presence_events = result.expect("Expected presence sync data");
        assert_eq!(presence_events.len(), 0); // No presence data in empty database
    }

    #[tokio::test]
    async fn test_sync_service_room_sync_data() {
        let sync_service = create_sync_service().await;
        
        let user_id = "@test:example.com";
        let room_id = "!test:example.com";
        
        // Test getting room sync data
        let result = sync_service.get_room_sync_data(user_id, room_id, None).await;
        assert!(result.is_ok());
        
        let room_sync_data = result.expect("Expected room sync data");
        assert_eq!(room_sync_data.room_id, room_id);
    }
}