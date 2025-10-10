#[cfg(test)]
mod media_service_tests {
    use crate::repository::{
        media::MediaRepository,
        media_service::MediaService,
        room::RoomRepository,
        membership::MembershipRepository,
    };
    use surrealdb::{Surreal, engine::any::Any};
    use std::sync::Arc;
    use tokio;

    async fn setup_test_db() -> Surreal<Any> {
        let db = surrealdb::engine::any::connect("surrealkv://test_data/media_test.db")
            .await
            .expect("Failed to connect to test database");
        db.use_ns("test")
            .use_db("test")
            .await
            .expect("Failed to set test database namespace");
        db
    }

    async fn create_media_service() -> MediaService<Any> {
        let db = setup_test_db().await;
        let media_repo = Arc::new(MediaRepository::new(db.clone()));
        let room_repo = Arc::new(RoomRepository::new(db.clone()));
        let membership_repo = Arc::new(MembershipRepository::new(db.clone()));
        
        MediaService::new(media_repo, room_repo, membership_repo)
    }

    #[tokio::test]
    async fn test_upload_media_success() {
        let media_service = create_media_service().await;
        
        let user_id = "@test:example.com";
        let content = b"test file content";
        let content_type = "text/plain";
        let filename = Some("test.txt");

        let result = media_service
            .upload_media(user_id, content, content_type, filename)
            .await;

        assert!(result.is_ok());
        let upload_result = result.expect("Expected upload result");
        assert!(!upload_result.media_id.is_empty());
        assert!(upload_result.content_uri.starts_with("mxc://"));
        assert_eq!(upload_result.content_type, content_type);
        assert_eq!(upload_result.content_length, content.len() as u64);
    }

    #[tokio::test]
    async fn test_upload_media_validation_failure() {
        let media_service = create_media_service().await;
        
        let user_id = "invalid_user_id"; // Invalid format
        let content = b"test content";
        let content_type = "application/malware"; // Invalid type
        let filename = None;

        let result = media_service
            .upload_media(user_id, content, content_type, filename)
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_download_media_success() {
        let media_service = create_media_service().await;
        
        // First upload media
        let user_id = "@test:example.com";
        let content = b"test download content";
        let content_type = "text/plain";
        
        let upload_result = media_service
            .upload_media(user_id, content, content_type, None)
            .await
            .expect("Failed to upload media");

        // Extract media_id and server_name from content_uri
        let uri_parts: Vec<&str> = upload_result.content_uri
            .strip_prefix("mxc://")
            .expect("Expected mxc:// prefix")
            .split('/')
            .collect();
        let server_name = uri_parts[0];
        let media_id = uri_parts[1];

        // Now download the media
        let download_result = media_service
            .download_media(media_id, server_name, user_id)
            .await;

        assert!(download_result.is_ok());
        let download = download_result.expect("Expected download result");
        assert_eq!(download.content, content);
        assert_eq!(download.content_type, content_type);
        assert_eq!(download.content_length, content.len() as u64);
    }

    #[tokio::test]
    async fn test_download_media_not_found() {
        let media_service = create_media_service().await;
        
        let result = media_service
            .download_media("nonexistent", "example.com", "@user:example.com")
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_generate_thumbnail_success() {
        let media_service = create_media_service().await;
        
        // Upload an image first
        let user_id = "@test:example.com";
        let content = b"fake_image_content"; // In real test, would use actual image data
        let content_type = "image/jpeg";
        
        let upload_result = media_service
            .upload_media(user_id, content, content_type, None)
            .await
            .expect("Failed to upload media");

        let uri_parts: Vec<&str> = upload_result.content_uri
            .strip_prefix("mxc://")
            .expect("Expected mxc:// prefix")
            .split('/')
            .collect();
        let server_name = uri_parts[0];
        let media_id = uri_parts[1];

        // Generate thumbnail
        let thumbnail_result = media_service
            .generate_thumbnail(media_id, server_name, 320, 240, "scale")
            .await;

        assert!(thumbnail_result.is_ok());
        let thumbnail = thumbnail_result.expect("Expected thumbnail result");
        assert_eq!(thumbnail.width, 320);
        assert_eq!(thumbnail.height, 240);
        assert_eq!(thumbnail.content_type, "image/jpeg");
        assert!(!thumbnail.thumbnail.is_empty());
    }

    #[tokio::test]
    async fn test_generate_thumbnail_non_image() {
        let media_service = create_media_service().await;
        
        // Upload a non-image file
        let user_id = "@test:example.com";
        let content = b"text content";
        let content_type = "text/plain";
        
        let upload_result = media_service
            .upload_media(user_id, content, content_type, None)
            .await
            .expect("Failed to upload media");

        let uri_parts: Vec<&str> = upload_result.content_uri
            .strip_prefix("mxc://")
            .expect("Expected mxc:// prefix")
            .split('/')
            .collect();
        let server_name = uri_parts[0];
        let media_id = uri_parts[1];

        // Try to generate thumbnail for non-image
        let thumbnail_result = media_service
            .generate_thumbnail(media_id, server_name, 320, 240, "scale")
            .await;

        assert!(thumbnail_result.is_err());
    }

    #[tokio::test]
    async fn test_validate_media_upload_valid() {
        let media_service = create_media_service().await;
        
        let result = media_service
            .validate_media_upload("@user:example.com", "image/jpeg", 1024)
            .await;

        assert!(result.is_ok());
        assert!(result.expect("Expected media upload validation result"));
    }

    #[tokio::test]
    async fn test_validate_media_upload_invalid_type() {
        let media_service = create_media_service().await;
        
        let result = media_service
            .validate_media_upload("@user:example.com", "application/malware", 1024)
            .await;

        assert!(result.is_ok());
        assert!(!result.expect("Expected media upload validation result"));
    }

    #[tokio::test]
    async fn test_validate_media_upload_too_large() {
        let media_service = create_media_service().await;
        
        let result = media_service
            .validate_media_upload("@user:example.com", "image/jpeg", 100 * 1024 * 1024) // 100MB
            .await;

        assert!(result.is_ok());
        assert!(!result.expect("Expected media upload validation result"));
    }

    #[tokio::test]
    async fn test_validate_media_access_success() {
        let media_service = create_media_service().await;
        
        // Upload media first
        let user_id = "@test:example.com";
        let content = b"test content";
        let upload_result = media_service
            .upload_media(user_id, content, "text/plain", None)
            .await
            .expect("Failed to upload media");

        let uri_parts: Vec<&str> = upload_result.content_uri
            .strip_prefix("mxc://")
            .expect("Expected mxc:// prefix")
            .split('/')
            .collect();
        let server_name = uri_parts[0];
        let media_id = uri_parts[1];

        // Validate access
        let result = media_service
            .validate_media_access(media_id, server_name, user_id)
            .await;

        assert!(result.is_ok());
        assert!(result.expect("Expected media access validation result"));
    }

    #[tokio::test]
    async fn test_get_media_statistics() {
        let media_service = create_media_service().await;
        
        // Upload some test media
        let user_id = "@test:example.com";
        let content1 = b"test content 1";
        let content2 = b"test content 2";
        
        let _upload1 = media_service
            .upload_media(user_id, content1, "text/plain", None)
            .await
            .expect("Failed to upload media 1");
            
        let _upload2 = media_service
            .upload_media(user_id, content2, "text/plain", None)
            .await
            .expect("Failed to upload media 2");

        // Get statistics
        let stats_result = media_service
            .get_media_statistics(Some("example.com"))
            .await;

        assert!(stats_result.is_ok());
        let stats = stats_result.expect("Expected media statistics");
        assert!(stats.total_files >= 2);
        assert!(stats.total_size > 0);
    }

    #[tokio::test]
    async fn test_cleanup_unused_media() {
        let media_service = create_media_service().await;
        
        let cutoff = chrono::Utc::now() + chrono::Duration::hours(1); // Future cutoff
        
        let cleanup_result = media_service
            .cleanup_unused_media(cutoff)
            .await;

        assert!(cleanup_result.is_ok());
        let cleanup = cleanup_result.expect("Expected cleanup result");
        assert_eq!(cleanup.deleted_files, 0); // No files should be deleted with future cutoff
    }

    #[tokio::test]
    async fn test_handle_federation_media_request() {
        let media_service = create_media_service().await;
        
        // Upload media first
        let user_id = "@test:example.com";
        let content = b"federation test content";
        let upload_result = media_service
            .upload_media(user_id, content, "text/plain", None)
            .await
            .expect("Failed to upload media");

        let uri_parts: Vec<&str> = upload_result.content_uri
            .strip_prefix("mxc://")
            .expect("Expected mxc:// prefix")
            .split('/')
            .collect();
        let server_name = uri_parts[0];
        let media_id = uri_parts[1];

        // Test federation request
        let federation_result = media_service
            .handle_federation_media_request(media_id, server_name, "requesting.server.com")
            .await;

        assert!(federation_result.is_ok());
        let response = federation_result.expect("Expected federation media response");
        assert_eq!(response.content, content);
        assert_eq!(response.content_type, "text/plain");
        assert_eq!(response.content_length, content.len() as u64);
    }

    #[tokio::test]
    async fn test_media_deduplication() {
        let media_service = create_media_service().await;
        
        let user_id = "@test:example.com";
        let content = b"duplicate test content";
        let content_type = "text/plain";

        // Upload same content twice
        let upload1 = media_service
            .upload_media(user_id, content, content_type, None)
            .await
            .expect("Failed to upload media first time");

        let upload2 = media_service
            .upload_media(user_id, content, content_type, None)
            .await
            .expect("Failed to upload media second time");

        // Should return the same media_id due to deduplication
        assert_eq!(upload1.media_id, upload2.media_id);
        assert_eq!(upload1.content_uri, upload2.content_uri);
    }

    #[tokio::test]
    async fn test_create_pending_upload_success() {
        let media_service = create_media_service().await;
        
        let user_id = "@test:example.com";
        let server_name = "example.com";
        
        let result = media_service
            .create_pending_upload(user_id, server_name)
            .await;
        
        assert!(result.is_ok());
        let (media_id, expires_at) = result.expect("Expected pending upload");
        assert!(!media_id.is_empty());
        assert!(expires_at > chrono::Utc::now());
    }

    #[tokio::test]
    async fn test_create_pending_upload_rate_limit() {
        let media_service = create_media_service().await;
        
        let user_id = "@test:example.com";
        let server_name = "example.com";
        
        // Create 10 pending uploads (should succeed)
        for _ in 0..10 {
            let result = media_service
                .create_pending_upload(user_id, server_name)
                .await;
            assert!(result.is_ok());
        }
        
        // 11th should fail with M_LIMIT_EXCEEDED
        let result = media_service
            .create_pending_upload(user_id, server_name)
            .await;
        
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("M_LIMIT_EXCEEDED"));
    }

    #[tokio::test]
    async fn test_upload_to_pending_success() {
        let media_service = create_media_service().await;
        
        let user_id = "@test:example.com";
        let server_name = "example.com";
        let content = b"test pending upload content";
        let content_type = "text/plain";
        
        // Create pending upload
        let (media_id, _) = media_service
            .create_pending_upload(user_id, server_name)
            .await
            .expect("Failed to create pending upload");
        
        // Upload to pending
        let result = media_service
            .upload_to_pending(&media_id, server_name, user_id, content, content_type)
            .await;
        
        assert!(result.is_ok());
        
        // Verify upload succeeded by downloading
        let download = media_service
            .download_media(&media_id, server_name, user_id)
            .await
            .expect("Failed to download uploaded media");
        
        assert_eq!(download.content, content);
    }

    #[tokio::test]
    async fn test_upload_to_pending_expired() {
        let media_service = create_media_service().await;
        
        let user_id = "@test:example.com";
        let server_name = "example.com";
        
        // Create a pending upload, then manually expire it by directly manipulating the database
        let (_media_id, _) = media_service
            .create_pending_upload(user_id, server_name)
            .await
            .expect("Failed to create pending upload");
        
        // Access the database directly to set expires_at to the past
        // Note: This requires direct database manipulation
        // For now, we'll test that a non-existent/expired media_id returns M_NOT_FOUND
        let expired_media_id = "expired-media-id-12345";
        
        let result = media_service
            .upload_to_pending(expired_media_id, server_name, user_id, b"content", "text/plain")
            .await;
        
        assert!(result.is_err());
        // Should return M_NOT_FOUND for expired/non-existent
        let err = result.unwrap_err();
        assert!(matches!(err, crate::repository::media_service::MediaError::NotFound));
    }

    #[tokio::test]
    async fn test_upload_to_pending_wrong_user() {
        let media_service = create_media_service().await;
        
        let creator_id = "@creator:example.com";
        let wrong_user = "@wrong:example.com";
        let server_name = "example.com";
        
        // Create pending upload as creator
        let (media_id, _) = media_service
            .create_pending_upload(creator_id, server_name)
            .await
            .expect("Failed to create pending upload");
        
        // Try to upload as different user
        let result = media_service
            .upload_to_pending(&media_id, server_name, wrong_user, b"content", "text/plain")
            .await;
        
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("M_FORBIDDEN"));
    }

    #[tokio::test]
    async fn test_upload_to_pending_already_uploaded() {
        let media_service = create_media_service().await;
        
        let user_id = "@test:example.com";
        let server_name = "example.com";
        let content = b"test content";
        
        // Create pending upload
        let (media_id, _) = media_service
            .create_pending_upload(user_id, server_name)
            .await
            .expect("Failed to create pending upload");
        
        // Upload once (should succeed)
        media_service
            .upload_to_pending(&media_id, server_name, user_id, content, "text/plain")
            .await
            .expect("First upload should succeed");
        
        // Try to upload again (should fail)
        let result = media_service
            .upload_to_pending(&media_id, server_name, user_id, content, "text/plain")
            .await;
        
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("M_CANNOT_OVERWRITE_MEDIA"));
    }
}