#[cfg(test)]
mod room_operations_tests {
    use crate::repository::{
        MembershipRepository, RoomRepository, RoomOperationsService,
        room_operations::{MembershipAction, RoomAction, MembershipEvent},
    };
    use matryx_entity::types::MembershipState;
    use surrealdb::{Surreal, engine::any::Any};

    async fn setup_test_db() -> Surreal<Any> {
        let db = surrealdb::engine::any::connect("surrealkv://test_data/room_ops_test.db").await.unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        db
    }

    #[tokio::test]
    async fn test_membership_repository_kick_member() {
        let db = setup_test_db().await;
        let membership_repo = MembershipRepository::new(db);
        
        let room_id = "!test:example.com";
        let user_id = "@user:example.com";
        let kicker_id = "@kicker:example.com";
        
        // Test kicking a member
        let result = membership_repo.kick_member(room_id, user_id, kicker_id, Some("Test kick")).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_membership_repository_ban_member() {
        let db = setup_test_db().await;
        let membership_repo = MembershipRepository::new(db);
        
        let room_id = "!test:example.com";
        let user_id = "@user:example.com";
        let banner_id = "@banner:example.com";
        
        // Test banning a member
        let result = membership_repo.ban_member(room_id, user_id, banner_id, Some("Test ban")).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_membership_repository_unban_member() {
        let db = setup_test_db().await;
        let membership_repo = MembershipRepository::new(db);
        
        let room_id = "!test:example.com";
        let user_id = "@user:example.com";
        let unbanner_id = "@unbanner:example.com";
        
        // Test unbanning a member
        let result = membership_repo.unban_member(room_id, user_id, unbanner_id, Some("Test unban")).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_membership_repository_invite_member() {
        let db = setup_test_db().await;
        let membership_repo = MembershipRepository::new(db);
        
        let room_id = "!test:example.com";
        let user_id = "@user:example.com";
        let inviter_id = "@inviter:example.com";
        
        // Test inviting a member
        let result = membership_repo.invite_member(room_id, user_id, inviter_id, Some("Test invite")).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_membership_repository_join_room() {
        let db = setup_test_db().await;
        let membership_repo = MembershipRepository::new(db);
        
        let room_id = "!test:example.com";
        let user_id = "@user:example.com";
        
        // Test joining a room
        let result = membership_repo.join_room(room_id, user_id, Some("Test join".to_string()), None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_membership_repository_leave_room() {
        let db = setup_test_db().await;
        let membership_repo = MembershipRepository::new(db);
        
        let room_id = "!test:example.com";
        let user_id = "@user:example.com";
        
        // Test leaving a room
        let result = membership_repo.leave_room(room_id, user_id, Some("Test leave".to_string())).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_membership_repository_forget_room() {
        let db = setup_test_db().await;
        let membership_repo = MembershipRepository::new(db);
        
        let room_id = "!test:example.com";
        let user_id = "@user:example.com";
        
        // Test forgetting a room
        let result = membership_repo.forget_room(room_id, user_id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_membership_repository_can_perform_action() {
        let db = setup_test_db().await;
        let membership_repo = MembershipRepository::new(db);
        
        let room_id = "!test:example.com";
        let user_id = "@user:example.com";
        let target_id = "@target:example.com";
        
        // Test permission checking for kick action
        let result = membership_repo.can_perform_action(room_id, user_id, MembershipAction::Kick, Some(target_id)).await;
        assert!(result.is_ok());
        
        let can_kick = result.unwrap();
        // In empty database, user won't have sufficient power level
        assert!(!can_kick);
    }

    #[tokio::test]
    async fn test_membership_repository_get_membership_history_events() {
        let db = setup_test_db().await;
        let membership_repo = MembershipRepository::new(db);
        
        let room_id = "!test:example.com";
        let user_id = "@user:example.com";
        
        // Test getting membership history
        let result = membership_repo.get_membership_history_events(room_id, user_id).await;
        assert!(result.is_ok());
        
        let history = result.unwrap();
        assert_eq!(history.len(), 0); // No events in empty database
    }

    #[tokio::test]
    async fn test_room_repository_validate_room_access() {
        let db = setup_test_db().await;
        let room_repo = RoomRepository::new(db);
        
        let room_id = "!test:example.com";
        let user_id = "@user:example.com";
        
        // Test room access validation for read action
        let result = room_repo.validate_room_access(room_id, user_id, RoomAction::Read).await;
        assert!(result.is_ok());
        
        let can_read = result.unwrap();
        // In empty database, room doesn't exist so access is denied
        assert!(!can_read);
    }

    #[tokio::test]
    async fn test_room_repository_get_room_join_rules() {
        let db = setup_test_db().await;
        let room_repo = RoomRepository::new(db);
        
        let room_id = "!test:example.com";
        
        // Test getting room join rules
        let result = room_repo.get_room_join_rules(room_id).await;
        assert!(result.is_ok());
        
        let join_rules = result.unwrap();
        // Default should be invite
        assert!(matches!(join_rules, crate::repository::room::JoinRules::Invite));
    }

    #[tokio::test]
    async fn test_room_repository_is_room_invite_only() {
        let db = setup_test_db().await;
        let room_repo = RoomRepository::new(db);
        
        let room_id = "!test:example.com";
        
        // Test checking if room is invite only
        let result = room_repo.is_room_invite_only(room_id).await;
        assert!(result.is_ok());
        
        let is_invite_only = result.unwrap();
        // Default join rules are invite, so should be true
        assert!(is_invite_only);
    }

    #[tokio::test]
    async fn test_room_repository_can_user_invite() {
        let db = setup_test_db().await;
        let room_repo = RoomRepository::new(db);
        
        let room_id = "!test:example.com";
        let user_id = "@user:example.com";
        
        // Test checking if user can invite
        let result = room_repo.can_user_invite(room_id, user_id).await;
        assert!(result.is_ok());
        
        let can_invite = result.unwrap();
        // User not joined, so cannot invite
        assert!(!can_invite);
    }

    #[tokio::test]
    async fn test_room_repository_get_room_guest_access() {
        let db = setup_test_db().await;
        let room_repo = RoomRepository::new(db);
        
        let room_id = "!test:example.com";
        
        // Test getting room guest access
        let result = room_repo.get_room_guest_access(room_id).await;
        assert!(result.is_ok());
        
        let guest_access = result.unwrap();
        // Default should be forbidden
        assert!(matches!(guest_access, crate::repository::room::GuestAccess::Forbidden));
    }

    #[tokio::test]
    async fn test_room_operations_service_kick_user() {
        let db = setup_test_db().await;
        let room_repo = RoomRepository::new(db.clone());
        let membership_repo = MembershipRepository::new(db.clone());
        let event_repo = crate::repository::event::EventRepository::new(db.clone());
        let relations_repo = crate::repository::relations::RelationsRepository::new(db.clone());
        let threads_repo = crate::repository::threads::ThreadsRepository::new(db.clone());
        
        let room_ops_service = RoomOperationsService::new(
            room_repo,
            event_repo,
            membership_repo,
            relations_repo,
            threads_repo,
        );
        
        let room_id = "!test:example.com";
        let user_id = "@user:example.com";
        let kicker_id = "@kicker:example.com";
        
        // Test kicking user (will fail due to validation in empty database)
        let result = room_ops_service.kick_user_from_room(room_id, user_id, kicker_id, Some("Test kick")).await;
        assert!(result.is_err()); // Should fail validation
    }

    #[tokio::test]
    async fn test_room_operations_service_validate_membership_operation() {
        let db = setup_test_db().await;
        let room_repo = RoomRepository::new(db.clone());
        let membership_repo = MembershipRepository::new(db.clone());
        let event_repo = crate::repository::event::EventRepository::new(db.clone());
        let relations_repo = crate::repository::relations::RelationsRepository::new(db.clone());
        let threads_repo = crate::repository::threads::ThreadsRepository::new(db.clone());
        
        let room_ops_service = RoomOperationsService::new(
            room_repo,
            event_repo,
            membership_repo,
            relations_repo,
            threads_repo,
        );
        
        let room_id = "!test:example.com";
        let actor_id = "@actor:example.com";
        let target_id = "@target:example.com";
        
        // Test validating kick operation
        let result = room_ops_service.validate_membership_operation(room_id, actor_id, target_id, crate::repository::room_operations::MembershipOperation::Kick).await;
        assert!(result.is_ok());
        
        let is_valid = result.unwrap();
        // Should be false in empty database
        assert!(!is_valid);
    }

    #[tokio::test]
    async fn test_membership_action_enum() {
        // Test that all membership actions are properly defined
        let actions = [
            MembershipAction::Kick,
            MembershipAction::Ban,
            MembershipAction::Unban,
            MembershipAction::Invite,
            MembershipAction::Join,
            MembershipAction::Leave,
            MembershipAction::Forget,
        ];
        
        assert_eq!(actions.len(), 7);
    }

    #[tokio::test]
    async fn test_room_action_enum() {
        // Test that all room actions are properly defined
        let actions = [
            RoomAction::Read,
            RoomAction::Write,
            RoomAction::Invite,
            RoomAction::Kick,
            RoomAction::Ban,
            RoomAction::RedactEvents,
            RoomAction::SendEvents,
            RoomAction::StateEvents,
        ];
        
        assert_eq!(actions.len(), 8);
    }

    #[tokio::test]
    async fn test_membership_event_creation() {
        // Test creating a membership event
        let event = MembershipEvent {
            event_id: "$test:example.com".to_string(),
            room_id: "!test:example.com".to_string(),
            user_id: "@user:example.com".to_string(),
            membership: MembershipState::Join,
            reason: Some("Test reason".to_string()),
            actor_id: Some("@actor:example.com".to_string()),
            timestamp: chrono::Utc::now(),
        };
        
        assert_eq!(event.room_id, "!test:example.com");
        assert_eq!(event.user_id, "@user:example.com");
        assert!(matches!(event.membership, MembershipState::Join));
        assert_eq!(event.reason, Some("Test reason".to_string()));
    }
}