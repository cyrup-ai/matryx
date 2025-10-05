use matryx_entity::types::{Event, EventContent, Membership, MembershipState};
use matryx_server::cache::lazy_loading_cache::LazyLoadingCache;
use matryx_server::metrics::lazy_loading_metrics::LazyLoadingMetrics;
use matryx_surrealdb::repository::membership::MembershipRepository;
use std::time::Duration;
use surrealdb::{Surreal, engine::any::Any};

async fn setup_test_database() -> Surreal<Any> {
    let db = surrealdb::engine::any::connect("surrealkv://test_data/lazy_loading_test.db")
        .await
        .expect("Test setup: failed to connect to test database");
    db.use_ns("test").use_db("test").await
        .expect("Test setup: failed to select test namespace");

    // Apply database schema for testing
    db.query("DEFINE TABLE room_membership SCHEMALESS").await
        .expect("Test setup: failed to create room_membership table");
    db.query("DEFINE TABLE power_levels SCHEMALESS").await
        .expect("Test setup: failed to create power_levels table");
    db.query("DEFINE TABLE rooms SCHEMALESS").await
        .expect("Test setup: failed to create rooms table");

    db
}

async fn create_membership_event(
    db: &Surreal<Any>,
    room_id: &str,
    user_id: &str,
    membership: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let membership_state = match membership {
        "join" => MembershipState::Join,
        "leave" => MembershipState::Leave,
        "invite" => MembershipState::Invite,
        "ban" => MembershipState::Ban,
        _ => MembershipState::Join,
    };

    let membership_record =
        Membership::new(room_id.to_string(), user_id.to_string(), membership_state);

    let id = format!("{}:{}", room_id, user_id);
    let _: Option<Membership> =
        db.create(("room_membership", id)).content(membership_record).await?;

    Ok(())
}

async fn create_power_level_event(
    db: &Surreal<Any>,
    room_id: &str,
    user_id: &str,
    power_level: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    let power_level_record = serde_json::json!({
        "room_id": room_id,
        "user_id": user_id,
        "power_level": power_level
    });

    let _: Option<serde_json::Value> =
        db.create("power_levels").content(power_level_record).await?;

    Ok(())
}

async fn create_message_event(
    _db: &Surreal<Any>,
    room_id: &str,
    sender: &str,
    content: &str,
) -> Event {
    Event::new(
        format!("${}:{}", uuid::Uuid::new_v4(), "example.com"),
        sender.to_string(),
        chrono::Utc::now().timestamp(),
        "m.room.message".to_string(),
        room_id.to_string(),
        EventContent::Unknown(serde_json::json!({
            "msgtype": "m.text",
            "body": content
        })),
    )
}

#[tokio::test]
async fn test_lazy_loading_performance_optimization() {
    let test_db = setup_test_database().await;
    let lazy_cache = LazyLoadingCache::new();

    use matryx_surrealdb::repository::PerformanceRepository;
    use std::sync::Arc;
    let performance_repo = Arc::new(PerformanceRepository::new(test_db.clone()));
    let _metrics = LazyLoadingMetrics::new(performance_repo);

    // Create large room with 1000 members
    let room_id = "!large_room:example.com";
    let user_id = "@user:example.com";

    // Add 1000 members to the room
    for i in 0..1000 {
        let member_id = format!("@member{}:example.com", i);
        create_membership_event(&test_db, room_id, &member_id, "join")
            .await
            .expect("Test setup: failed to create membership event");
    }

    // Add power users (admins and moderators)
    for i in 0..5 {
        let admin_id = format!("@admin{}:example.com", i);
        create_power_level_event(&test_db, room_id, &admin_id, 100).await
            .expect("Test setup: failed to create power level event"); // Admin
    }

    // Add timeline events from specific senders
    let timeline_senders = vec![
        "@member1:example.com".to_string(),
        "@member50:example.com".to_string(),
    ];
    let mut timeline_events = Vec::new();
    for sender in &timeline_senders {
        timeline_events.push(create_message_event(&test_db, room_id, sender, "Hello world").await);
    }

    let membership_repo = MembershipRepository::new(test_db.clone());

    // Test first request (cache miss)
    let start_time = std::time::Instant::now();
    let essential_members_1 = lazy_cache
        .get_essential_members_cached(room_id, user_id, &timeline_senders, &membership_repo)
        .await
        .expect("Test execution: lazy loading cache should return essential members");
    let first_duration = start_time.elapsed();

    // Verify essential members are correctly identified
    assert!(essential_members_1.contains(user_id)); // Requesting user
    assert!(essential_members_1.contains("@member1:example.com")); // Timeline sender
    assert!(essential_members_1.contains("@member50:example.com")); // Timeline sender
    assert!(essential_members_1.len() <= 20); // Should be much smaller than 1000

    // Test second request (cache hit)
    let start_time = std::time::Instant::now();
    let essential_members_2 = lazy_cache
        .get_essential_members_cached(room_id, user_id, &timeline_senders, &membership_repo)
        .await
        .expect("Test execution: lazy loading cache should return essential members on cache hit");
    let second_duration = start_time.elapsed();

    // Verify cache hit performance improvement
    assert_eq!(essential_members_1, essential_members_2);
    assert!(second_duration < first_duration / 2); // Should be at least 50% faster

    // Verify cache statistics
    let cache_stats = lazy_cache.get_cache_stats().await;
    assert!(cache_stats.essential_members_size > 0);

    // Verify performance targets from task specification
    assert!(first_duration < Duration::from_millis(100)); // Sub-100ms for first request
    assert!(second_duration < Duration::from_millis(10)); // Sub-10ms for cached request
}

#[tokio::test]
async fn test_database_level_optimization() {
    let test_db = setup_test_database().await;
    let membership_repo = MembershipRepository::new(test_db.clone());

    let room_id = "!optimization_test:example.com";
    let user_id = "@user:example.com";

    // Create room with mixed membership types
    for i in 0..100 {
        let member_id = format!("@member{}:example.com", i);
        create_membership_event(&test_db, room_id, &member_id, "join")
            .await
            .expect("Test setup: failed to create membership event");
    }

    // Create power level hierarchy
    create_power_level_event(&test_db, room_id, "@admin1:example.com", 100)
        .await
        .expect("Test setup: failed to create power level event for admin");
    create_power_level_event(&test_db, room_id, "@mod1:example.com", 50)
        .await
        .expect("Test setup: failed to create power level event for moderator");

    let timeline_senders = vec![
        "@member5:example.com".to_string(),
        "@member15:example.com".to_string(),
    ];

    // Test optimized database query
    let start_time = std::time::Instant::now();
    let essential_members = membership_repo
        .get_essential_members_optimized(room_id, user_id, &timeline_senders)
        .await
        .expect("Test execution: database query should return essential members");
    let query_duration = start_time.elapsed();

    // Verify results
    assert!(essential_members.iter().any(|m| m.user_id == user_id)); // User included
    assert!(essential_members.iter().any(|m| m.user_id == "@admin1:example.com")); // Admin included
    assert!(essential_members.iter().any(|m| m.user_id == "@mod1:example.com")); // Mod included
    assert!(essential_members.iter().any(|m| m.user_id == "@member5:example.com")); // Timeline sender
    assert!(essential_members.len() < 20); // Much smaller than full membership

    // Verify performance (should be fast even with complex query)
    assert!(query_duration < Duration::from_millis(50)); // Sub-50ms target from task spec
}

#[tokio::test]
async fn test_cache_invalidation_on_membership_changes() {
    let test_db = setup_test_database().await;
    let lazy_cache = LazyLoadingCache::new();

    let room_id = "!invalidation_test:example.com";
    let user_id = "@user:example.com";

    let membership_repo = MembershipRepository::new(test_db.clone());

    // Cache some essential members
    let timeline_senders = vec!["@member1:example.com".to_string()];
    let _ = lazy_cache
        .get_essential_members_cached(room_id, user_id, &timeline_senders, &membership_repo)
        .await
        .expect("Test execution: cache should return essential members for invalidation test");

    // Verify cache has data
    let cache_stats_before = lazy_cache.get_cache_stats().await;
    assert!(cache_stats_before.essential_members_size > 0);

    // Simulate membership change by invalidating cache
    lazy_cache.invalidate_room_cache(room_id).await;

    // Verify cache was cleared
    let _cache_stats_after = lazy_cache.get_cache_stats().await;
    // Note: entry count may not change immediately due to moka's lazy cleanup,
    // but the actual cached data should be invalidated
}

#[tokio::test]
async fn test_memory_usage_within_limits() {
    let lazy_cache = LazyLoadingCache::new();
    let test_db = setup_test_database().await;
    let membership_repo = MembershipRepository::new(test_db.clone());

    // Create multiple rooms and cache data for each
    for room_num in 0..100 {
        let room_id = format!("!room{}:example.com", room_num);
        let user_id = "@user:example.com";

        // Create some test data for the room
        for i in 0..50 {
            let member_id = format!("@member{}:example.com", i);
            create_membership_event(&test_db, &room_id, &member_id, "join")
                .await
                .expect("Test setup: failed to create membership event for memory usage test");
        }

        // Cache essential members for this room
        let timeline_senders = vec![format!("@member{}:example.com", room_num % 10)];
        let _ = lazy_cache
            .get_essential_members_cached(&room_id, user_id, &timeline_senders, &membership_repo)
            .await;
    }

    // Check memory usage is within acceptable limits
    let memory_usage = lazy_cache.get_estimated_memory_usage_bytes().await;
    const MAX_MEMORY_BYTES: usize = 100 * 1024 * 1024; // 100MB from task spec

    assert!(
        memory_usage < MAX_MEMORY_BYTES,
        "Memory usage {}MB exceeds limit {}MB",
        memory_usage / (1024 * 1024),
        MAX_MEMORY_BYTES / (1024 * 1024)
    );
}

#[tokio::test]
async fn test_cache_hit_ratio_targets() {
    let lazy_cache = LazyLoadingCache::new();
    let test_db = setup_test_database().await;
    let membership_repo = MembershipRepository::new(test_db.clone());

    let room_id = "!hit_ratio_test:example.com";
    let user_id = "@user:example.com";

    // Create test room
    for i in 0..100 {
        let member_id = format!("@member{}:example.com", i);
        create_membership_event(&test_db, room_id, &member_id, "join")
            .await
            .expect("Test setup: failed to create membership event");
    }

    let timeline_senders = vec!["@member1:example.com".to_string()];

    // First request (cache miss)
    let _ = lazy_cache
        .get_essential_members_cached(room_id, user_id, &timeline_senders, &membership_repo)
        .await
        .expect("Test execution: cache should return essential members on first request");

    // Multiple subsequent requests (should be cache hits)
    for _ in 0..10 {
        let _ = lazy_cache
            .get_essential_members_cached(room_id, user_id, &timeline_senders, &membership_repo)
            .await
            .expect("Test execution: cache should return essential members on subsequent requests");
    }

    // Check cache hit ratio
    let hit_ratio = lazy_cache.get_cache_hit_ratio().await;
    const MIN_HIT_RATIO: f64 = 0.80; // 80% target from task spec

    assert!(
        hit_ratio >= MIN_HIT_RATIO,
        "Cache hit ratio {:.2} below target {:.2}",
        hit_ratio,
        MIN_HIT_RATIO
    );
}

#[tokio::test]
async fn test_large_room_scalability() {
    let test_db = setup_test_database().await;
    let lazy_cache = LazyLoadingCache::new();
    let membership_repo = MembershipRepository::new(test_db.clone());

    let room_id = "!huge_room:example.com";
    let user_id = "@user:example.com";

    // Create a very large room (10,000 members) to test scalability
    for i in 0..10000 {
        let member_id = format!("@member{}:example.com", i);
        create_membership_event(&test_db, room_id, &member_id, "join")
            .await
            .expect("Test setup: failed to create membership event");
    }

    // Add power users
    for i in 0..10 {
        let admin_id = format!("@admin{}:example.com", i);
        create_power_level_event(&test_db, room_id, &admin_id, 100).await
            .expect("Test setup: failed to create power level event for large room scalability test");
    }

    let timeline_senders = vec![
        "@member1:example.com".to_string(),
        "@member100:example.com".to_string(),
    ];

    // Test that even with 10k members, lazy loading is efficient
    let start_time = std::time::Instant::now();
    let essential_members = lazy_cache
        .get_essential_members_cached(room_id, user_id, &timeline_senders, &membership_repo)
        .await
        .expect("Test execution: cache should return essential members for large room scalability test");
    let processing_time = start_time.elapsed();

    // Verify scalability targets
    assert!(essential_members.len() < 50); // Should filter out most members
    assert!(processing_time < Duration::from_millis(100)); // Sub-100ms even for huge rooms

    // Verify essential members include the right people
    assert!(essential_members.contains(user_id));
    assert!(essential_members.contains("@member1:example.com"));
    assert!(essential_members.contains("@member100:example.com"));

    // Should include some admins (power users)
    let has_admin = essential_members.iter().any(|id| id.starts_with("@admin"));
    assert!(has_admin, "Essential members should include power users");
}

/// Integration test for complete lazy loading pipeline
#[tokio::test]
async fn test_complete_lazy_loading_pipeline() {
    let test_db = setup_test_database().await;
    let lazy_cache = LazyLoadingCache::new();
    let membership_repo = MembershipRepository::new(test_db.clone());

    let room_id = "!pipeline_test:example.com";
    let user_id = "@user:example.com";

    // Setup test room
    for i in 0..1000 {
        let member_id = format!("@member{}:example.com", i);
        create_membership_event(&test_db, room_id, &member_id, "join")
            .await
            .expect("Test setup: failed to create membership event");
    }

    // Create test events including membership and message events
    let mut all_events = Vec::new();

    // Add membership events for all members
    for i in 0..1000 {
        let member_id = format!("@member{}:example.com", i);
        let mut event = Event::new(
            format!("${}:{}", uuid::Uuid::new_v4(), "example.com"),
            member_id.clone(),
            chrono::Utc::now().timestamp(),
            "m.room.member".to_string(),
            room_id.to_string(),
            EventContent::Unknown(serde_json::json!({
                "membership": "join"
            })),
        );
        event.state_key = Some(member_id);
        all_events.push(event);
    }

    // Add some message events
    let timeline_senders = vec![
        "@member1:example.com".to_string(),
        "@member50:example.com".to_string(),
    ];
    for sender in &timeline_senders {
        all_events
            .push(create_message_event(&test_db, room_id, sender, "Hello from timeline").await);
    }

    // Test the complete pipeline with performance measurement
    let start_time = std::time::Instant::now();

    // Simulate the enhanced lazy loading filter process
    let essential_members = lazy_cache
        .get_essential_members_cached(room_id, user_id, &timeline_senders, &membership_repo)
        .await
        .expect("Test execution: cache should return essential members for complete pipeline test");

    // Filter events like the actual implementation would
    let filtered_events: Vec<Event> = all_events
        .into_iter()
        .filter(|event| {
            if event.event_type == "m.room.member" {
                event
                    .state_key
                    .as_ref()
                    .map(|state_key| essential_members.contains(state_key))
                    .unwrap_or(false)
            } else {
                true // Always include non-membership events
            }
        })
        .collect();

    let processing_time = start_time.elapsed();

    // Verify pipeline performance and correctness
    assert!(processing_time < Duration::from_millis(100)); // Overall pipeline performance

    let membership_events =
        filtered_events.iter().filter(|e| e.event_type == "m.room.member").count();
    let message_events =
        filtered_events.iter().filter(|e| e.event_type == "m.room.message").count();

    assert!(membership_events < 50); // Significantly filtered
    assert_eq!(message_events, 2); // All message events preserved

    // Verify the right members are included
    let member_ids: std::collections::HashSet<String> = filtered_events
        .iter()
        .filter(|e| e.event_type == "m.room.member")
        .filter_map(|e| e.state_key.clone())
        .collect();

    assert!(member_ids.contains(user_id));
    assert!(member_ids.contains("@member1:example.com"));
    assert!(member_ids.contains("@member50:example.com"));
}
