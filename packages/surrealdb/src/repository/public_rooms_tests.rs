#[cfg(test)]
mod public_rooms_tests {
    use crate::repository::{
        PublicRoomsRepository, RoomDiscoveryService, RoomDirectoryVisibility,
        RoomRepository, RoomAliasRepository, MembershipRepository,
    };
    use surrealdb::{Surreal, engine::any::Any};
    use std::sync::Arc;

    async fn setup_test_db() -> Surreal<Any> {
        let db = surrealdb::engine::any::connect("surrealkv://test_data/public_rooms_test.db").await.unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        db
    }

    async fn create_room_discovery_service() -> RoomDiscoveryService {
        let db = setup_test_db().await;
        let public_rooms_repo = Arc::new(PublicRoomsRepository::new(db.clone()));
        let room_repo = Arc::new(RoomRepository::new(db.clone()));
        let room_alias_repo = Arc::new(RoomAliasRepository::new(db.clone()));
        let membership_repo = Arc::new(MembershipRepository::new(db.clone()));
        
        RoomDiscoveryService::new(public_rooms_repo, room_repo, room_alias_repo, membership_repo)
    }

    #[tokio::test]
    async fn test_public_rooms_repository_get_public_rooms() {
        let db = setup_test_db().await;
        let public_rooms_repo = PublicRoomsRepository::new(db);
        
        // Test getting public rooms for empty database
        let response = public_rooms_repo.get_public_rooms(Some(10), None).await.unwrap();
        assert_eq!(response.chunk.len(), 0);
        assert_eq!(response.total_room_count_estimate, Some(0));
    }

    #[tokio::test]
    async fn test_public_rooms_repository_search_public_rooms() {
        let db = setup_test_db().await;
        let public_rooms_repo = PublicRoomsRepository::new(db);
        
        let search_term = "test";
        
        // Test searching public rooms for empty database
        let response = public_rooms_repo.search_public_rooms(search_term, Some(10)).await.unwrap();
        assert_eq!(response.chunk.len(), 0);
    }

    #[tokio::test]
    async fn test_public_rooms_repository_get_public_rooms_count() {
        let db = setup_test_db().await;
        let public_rooms_repo = PublicRoomsRepository::new(db);
        
        // Test getting public rooms count for empty database
        let count = public_rooms_repo.get_public_rooms_count().await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_public_rooms_repository_room_directory_visibility() {
        let db = setup_test_db().await;
        let public_rooms_repo = PublicRoomsRepository::new(db);
        
        let room_id = "!test:example.com";
        
        // Test getting directory visibility for non-existent room
        let visibility = public_rooms_repo.get_room_directory_visibility(room_id).await.unwrap();
        assert!(visibility.is_none());
        
        // Test adding room to directory
        let result = public_rooms_repo.add_room_to_directory(room_id, RoomDirectoryVisibility::Public).await;
        assert!(result.is_ok());
        
        // Test removing room from directory
        let result = public_rooms_repo.remove_room_from_directory(room_id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_public_rooms_repository_federation_public_rooms() {
        let db = setup_test_db().await;
        let public_rooms_repo = PublicRoomsRepository::new(db);
        
        let server_name = "example.com";
        
        // Test getting federation public rooms for empty database
        let response = public_rooms_repo.get_federation_public_rooms(server_name, Some(10)).await.unwrap();
        assert_eq!(response.chunk.len(), 0);
    }

    #[tokio::test]
    async fn test_room_discovery_service_get_public_rooms_list() {
        let discovery_service = create_room_discovery_service().await;
        
        let filter = crate::repository::public_rooms::PublicRoomsFilter {
            limit: Some(10),
            since: None,
            server: None,
            include_all_known_networks: None,
            third_party_instance_id: None,
        };
        
        // Test getting public rooms list
        let result = discovery_service.get_public_rooms_list(filter).await;
        assert!(result.is_ok());
        
        let response = result.unwrap();
        assert_eq!(response.chunk.len(), 0); // Empty database
    }

    #[tokio::test]
    async fn test_room_discovery_service_search_rooms() {
        let discovery_service = create_room_discovery_service().await;
        
        let query = "test room";
        let filter = crate::repository::public_rooms::PublicRoomsFilter {
            limit: Some(10),
            since: None,
            server: None,
            include_all_known_networks: None,
            third_party_instance_id: None,
        };
        
        // Test searching rooms
        let result = discovery_service.search_rooms(query, filter).await;
        assert!(result.is_ok());
        
        let response = result.unwrap();
        assert_eq!(response.chunk.len(), 0); // Empty database
    }

    #[tokio::test]
    async fn test_room_discovery_service_get_room_directory_entry() {
        let discovery_service = create_room_discovery_service().await;
        
        let room_id = "!test:example.com";
        
        // Test getting room directory entry for non-existent room
        let result = discovery_service.get_room_directory_entry(room_id).await;
        assert!(result.is_ok());
        
        let entry = result.unwrap();
        assert!(entry.is_none()); // Room doesn't exist
    }

    #[tokio::test]
    async fn test_room_discovery_service_update_room_directory_stats() {
        let discovery_service = create_room_discovery_service().await;
        
        let room_id = "!test:example.com";
        
        // Test updating room directory stats for non-existent room
        let result = discovery_service.update_room_directory_stats(room_id).await;
        // Should not fail even if room doesn't exist
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_room_discovery_service_get_federation_public_rooms() {
        let discovery_service = create_room_discovery_service().await;
        
        let server_name = "example.com";
        let filter = crate::repository::public_rooms::PublicRoomsFilter {
            limit: Some(10),
            since: None,
            server: Some(server_name.to_string()),
            include_all_known_networks: None,
            third_party_instance_id: None,
        };
        
        // Test getting federation public rooms
        let result = discovery_service.get_federation_public_rooms(server_name, filter).await;
        assert!(result.is_ok());
        
        let response = result.unwrap();
        assert_eq!(response.chunk.len(), 0); // Empty database
    }

    #[tokio::test]
    async fn test_room_discovery_service_resolve_alias_and_get_public_info() {
        let discovery_service = create_room_discovery_service().await;
        
        let alias = "#test:example.com";
        
        // Test resolving alias for non-existent alias
        let result = discovery_service.resolve_alias_and_get_public_info(alias).await;
        assert!(result.is_ok());
        
        let entry = result.unwrap();
        assert!(entry.is_none()); // Alias doesn't exist
    }

    #[tokio::test]
    async fn test_room_discovery_service_get_room_statistics() {
        let discovery_service = create_room_discovery_service().await;
        
        let room_id = "!test:example.com";
        
        // Test getting room statistics for non-existent room
        let result = discovery_service.get_room_statistics(room_id).await;
        assert!(result.is_ok());
        
        let stats = result.unwrap();
        assert_eq!(stats.room_id, room_id);
        assert_eq!(stats.member_count, 0);
        assert!(matches!(stats.visibility, RoomDirectoryVisibility::Private));
    }

    #[tokio::test]
    async fn test_room_discovery_service_update_room_search_index() {
        let discovery_service = create_room_discovery_service().await;
        
        let room_id = "!test:example.com";
        
        // Test updating room search index
        let result = discovery_service.update_room_search_index(room_id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_public_rooms_filter_with_server() {
        let discovery_service = create_room_discovery_service().await;
        
        let filter_with_server = crate::repository::public_rooms::PublicRoomsFilter {
            limit: Some(10),
            since: None,
            server: Some("example.com".to_string()),
            include_all_known_networks: None,
            third_party_instance_id: None,
        };
        
        // Test getting public rooms list with server filter
        let result = discovery_service.get_public_rooms_list(filter_with_server).await;
        assert!(result.is_ok());
        
        let response = result.unwrap();
        assert_eq!(response.chunk.len(), 0); // Empty database
    }

    #[tokio::test]
    async fn test_search_rooms_with_empty_query() {
        let discovery_service = create_room_discovery_service().await;
        
        let empty_query = "";
        let filter = crate::repository::public_rooms::PublicRoomsFilter {
            limit: Some(10),
            since: None,
            server: None,
            include_all_known_networks: None,
            third_party_instance_id: None,
        };
        
        // Test searching with empty query should return regular public rooms list
        let result = discovery_service.search_rooms(empty_query, filter).await;
        assert!(result.is_ok());
        
        let response = result.unwrap();
        assert_eq!(response.chunk.len(), 0); // Empty database
    }
}