use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use cyrup_matrix::db::client::DatabaseClient;
use cyrup_matrix::db::migration;
use cyrup_matrix::db::dao::account_data_dao::AccountDataDao;
use cyrup_matrix::db::dao::media_upload_dao::MediaUploadDao;
use cyrup_matrix::db::dao::presence_dao::PresenceDao;
use cyrup_matrix::db::dao::receipt_dao::ReceiptDao;
use cyrup_matrix::db::dao::request_dependency_dao::RequestDependencyDao;
use cyrup_matrix::db::dao::room_state_dao::RoomStateDao;
use cyrup_matrix::db::dao::send_queue_dao::SendQueueDao;
use cyrup_matrix::db::dao::api_cache_dao::ApiCacheDao;

use chrono::Utc;
use matrix_sdk_base::MilliSecondsSinceUnixEpoch;
use matrix_sdk_base::store::{QueuedRequestKind, SentRequestKey};
use matrix_sdk_base::ruma::{events::room::message::RoomMessageEventContent, OwnedRoomId, OwnedUserId, OwnedTransactionId, OwnedEventId};
use matrix_sdk_base::ruma::events::receipt::{Receipt, ReceiptType, ReceiptThread};
use serde_json::json;
use uuid::Uuid;

// Helper function to create a test database client with an in-memory database
async fn create_test_db() -> Arc<DatabaseClient> {
    let client = DatabaseClient::connect_memory().await.expect("Failed to connect to memory database");
    
    // Run migrations
    migration::run_migration(&client, migration::get_hardcoded_migration())
        .await
        .expect("Failed to run migrations");
    
    Arc::new(client)
}

// Helper to generate a random room ID
fn random_room_id() -> OwnedRoomId {
    OwnedRoomId::try_from(format!("!{}:test.com", Uuid::new_v4())).expect("Failed to create room ID")
}

// Helper to generate a random user ID
fn random_user_id() -> OwnedUserId {
    OwnedUserId::try_from(format!("@user_{}:test.com", Uuid::new_v4())).expect("Failed to create user ID")
}

// Helper to generate a random transaction ID
fn random_transaction_id() -> OwnedTransactionId {
    OwnedTransactionId::try_from(format!("txn_{}", Uuid::new_v4())).expect("Failed to create transaction ID")
}

// Helper to generate a random event ID
fn random_event_id() -> OwnedEventId {
    OwnedEventId::try_from(format!("${}:test.com", Uuid::new_v4())).expect("Failed to create event ID")
}

// Helper to generate a random string
fn random_string() -> String {
    Uuid::new_v4().to_string()
}

#[tokio::test]
async fn test_room_state_dao() {
    let client = create_test_db().await;
    let dao = RoomStateDao::new(client.clone());
    
    let room_id = random_room_id();
    let event_type = "m.room.name";
    let state_key = "";
    let room_name = json!({
        "name": "Test Room",
        "type": "m.room.name"
    });
    
    // Test saving room state
    dao.set_state_event(&room_id, event_type, state_key, room_name.clone())
        .await
        .expect("Failed to save room state");
    
    // Test getting room state
    let result = dao.get_state_event(&room_id, event_type, state_key)
        .await
        .expect("Failed to get room state");
    
    assert!(result.is_some(), "Expected room state to be found");
    assert_eq!(result.unwrap()["name"], "Test Room");
    
    // Test getting state events by type
    let events = dao.get_state_events(&room_id, event_type)
        .await
        .expect("Failed to get state events by type");
    
    assert_eq!(events.len(), 1, "Expected exactly one state event");
    assert_eq!(events[0].0, state_key);
    assert_eq!(events[0].1["name"], "Test Room");
    
    // Test getting all state events
    let all_events = dao.get_all_state_events(&room_id)
        .await
        .expect("Failed to get all state events");
    
    assert_eq!(all_events.len(), 1, "Expected exactly one state event");
    assert_eq!(all_events[0]["name"], "Test Room");
}

#[tokio::test]
async fn test_account_data_dao() {
    let client = create_test_db().await;
    let dao = AccountDataDao::new(client.clone());
    
    let room_id = random_room_id();
    let event_type = "m.fully_read";
    let event_data = json!({
        "event_id": "$someEventId:server.org",
        "ts": 1234567890
    });
    
    // Test saving room account data
    dao.set_room_account_data(&room_id, event_type, event_data.clone())
        .await
        .expect("Failed to save room account data");
    
    // Test getting room account data
    let result = dao.get_room_account_data(&room_id, event_type)
        .await
        .expect("Failed to get room account data");
    
    assert!(result.is_some(), "Expected room account data to be found");
    assert_eq!(result.unwrap()["event_id"], "$someEventId:server.org");
    
    // Test global account data
    let global_type = "m.direct";
    let global_data = json!({
        "user1": ["!room1:server.org"]
    });
    
    dao.set_global_account_data(global_type, global_data.clone())
        .await
        .expect("Failed to save global account data");
    
    let global_result = dao.get_global_account_data(global_type)
        .await
        .expect("Failed to get global account data");
    
    assert!(global_result.is_some(), "Expected global account data to be found");
    
    // Test getting all room account data
    let all_room_data = dao.get_all_room_account_data(&room_id)
        .await
        .expect("Failed to get all room account data");
    
    assert_eq!(all_room_data.len(), 1, "Expected exactly one room account data item");
    
    // Test getting all global account data
    let all_global_data = dao.get_all_global_account_data()
        .await
        .expect("Failed to get all global account data");
    
    assert_eq!(all_global_data.len(), 1, "Expected exactly one global account data item");
}

#[tokio::test]
async fn test_presence_dao() {
    let client = create_test_db().await;
    let dao = PresenceDao::new(client.clone());
    
    let user_id = random_user_id();
    let presence_data = json!({
        "presence": "online",
        "status_msg": "I am online",
        "last_active_ago": 10000
    });
    
    // Test setting presence
    dao.set_presence_event(&user_id, presence_data.clone())
        .await
        .expect("Failed to set presence");
    
    // Test getting presence
    let result = dao.get_presence_event(&user_id)
        .await
        .expect("Failed to get presence");
    
    assert!(result.is_some(), "Expected presence to be found");
    assert_eq!(result.unwrap()["status_msg"], "I am online");
    
    // Test getting presence for multiple users
    let user_id2 = random_user_id();
    let presence_data2 = json!({
        "presence": "offline",
        "status_msg": "I am offline",
        "last_active_ago": 20000
    });
    
    dao.set_presence_event(&user_id2, presence_data2.clone())
        .await
        .expect("Failed to set presence for user 2");
    
    let multi_result = dao.get_presence_events(&[user_id.clone(), user_id2.clone()])
        .await
        .expect("Failed to get presence for multiple users");
    
    assert_eq!(multi_result.len(), 2, "Expected two presence events");
}

#[tokio::test]
async fn test_receipt_dao() {
    let client = create_test_db().await;
    let dao = ReceiptDao::new(client.clone());
    
    let room_id = random_room_id();
    let user_id = random_user_id();
    let event_id = random_event_id();
    let receipt_type = ReceiptType::Read;
    let thread = ReceiptThread::Main;
    
    let receipt = Receipt {
        ts: Some(MilliSecondsSinceUnixEpoch(Utc::now().timestamp_millis() as u64)),
    };
    
    // Test setting receipt
    dao.set_receipt(&room_id, receipt_type, thread.clone(), &event_id, &user_id, receipt.clone())
        .await
        .expect("Failed to set receipt");
    
    // Test getting user receipt
    let user_receipt = dao.get_user_receipt(&room_id, receipt_type, thread.clone(), &user_id)
        .await
        .expect("Failed to get user receipt");
    
    assert!(user_receipt.is_some(), "Expected user receipt to be found");
    let (stored_event_id, stored_receipt) = user_receipt.unwrap();
    assert_eq!(stored_event_id, event_id);
    assert_eq!(stored_receipt.ts, receipt.ts);
    
    // Test getting event receipts
    let event_receipts = dao.get_event_receipts(&room_id, receipt_type, thread.clone(), &event_id)
        .await
        .expect("Failed to get event receipts");
    
    assert_eq!(event_receipts.len(), 1, "Expected exactly one receipt for the event");
    assert_eq!(event_receipts[0].0, user_id);
    assert_eq!(event_receipts[0].1.ts, receipt.ts);
}

#[tokio::test]
async fn test_send_queue_dao() {
    let client = create_test_db().await;
    let dao = SendQueueDao::new(client.clone());
    
    let room_id = random_room_id();
    let txn_id = random_transaction_id();
    let created_at = MilliSecondsSinceUnixEpoch(Utc::now().timestamp_millis() as u64);
    
    let content = RoomMessageEventContent::text_plain("Hello, world!");
    let request = QueuedRequestKind::Message {
        content: Box::new(content),
        txn_id: txn_id.clone(),
    };
    
    // Test adding request to queue
    dao.add_request_to_queue(&room_id, txn_id.clone(), created_at, request.clone(), 1)
        .await
        .expect("Failed to add request to queue");
    
    // Test getting request
    let stored_request = dao.get_request(&room_id, &txn_id)
        .await
        .expect("Failed to get request");
    
    assert!(stored_request.is_some(), "Expected request to be found");
    
    // Test getting all requests for a room
    let room_requests = dao.get_all_requests(&room_id)
        .await
        .expect("Failed to get all room requests");
    
    assert_eq!(room_requests.len(), 1, "Expected exactly one request in the room");
    
    // Test removing request
    dao.remove_request(&room_id, &txn_id)
        .await
        .expect("Failed to remove request");
    
    let removed_request = dao.get_request(&room_id, &txn_id)
        .await
        .expect("Failed to check for removed request");
    
    assert!(removed_request.is_none(), "Expected request to be removed");
}

#[tokio::test]
async fn test_request_dependency_dao() {
    let client = create_test_db().await;
    let dao = RequestDependencyDao::new(client.clone());
    
    let room_id = random_room_id();
    let parent_txn_id = random_transaction_id();
    let child_txn_id = random_transaction_id().to_string();
    let created_at = MilliSecondsSinceUnixEpoch(Utc::now().timestamp_millis() as u64);
    let content = serde_json::to_value("Hello, dependent world!").unwrap();
    
    // Test saving dependent request
    dao.save_dependent_request(&room_id, &parent_txn_id, child_txn_id.clone(), created_at, content.clone())
        .await
        .expect("Failed to save dependent request");
    
    // Test getting dependent requests
    let dependents = dao.get_dependent_requests(&room_id, &parent_txn_id)
        .await
        .expect("Failed to get dependent requests");
    
    assert_eq!(dependents.len(), 1, "Expected exactly one dependent request");
    assert_eq!(dependents[0], child_txn_id);
    
    // Test marking request as ready
    let key = SentRequestKey::EventId(random_event_id());
    dao.mark_as_ready(&room_id, &parent_txn_id, key)
        .await
        .expect("Failed to mark request as ready");
    
    // Test removing dependent request
    dao.remove_dependent_request(&room_id, &child_txn_id)
        .await
        .expect("Failed to remove dependent request");
    
    let removed_dependents = dao.get_dependent_requests(&room_id, &parent_txn_id)
        .await
        .expect("Failed to check for removed dependent requests");
    
    assert!(removed_dependents.is_empty(), "Expected dependent request to be removed");
}

#[tokio::test]
async fn test_media_upload_dao() {
    let client = create_test_db().await;
    let dao = MediaUploadDao::new(client.clone());
    
    let request_id = random_string();
    
    // Test marking upload as started
    dao.mark_upload_started(&request_id)
        .await
        .expect("Failed to mark upload as started");
    
    // Test getting uploads
    let uploads = dao.get_uploads()
        .await
        .expect("Failed to get uploads");
    
    assert_eq!(uploads.len(), 1, "Expected exactly one upload");
    assert_eq!(uploads[0], request_id);
    
    // Test removing upload
    dao.remove_upload(&request_id)
        .await
        .expect("Failed to remove upload");
    
    let remaining_uploads = dao.get_uploads()
        .await
        .expect("Failed to check for remaining uploads");
    
    assert!(remaining_uploads.is_empty(), "Expected upload to be removed");
}

#[tokio::test]
async fn test_api_cache_dao() {
    let client = create_test_db().await;
    let dao = ApiCacheDao::new(client.clone());
    
    let key = "sync_token";
    let value = "t123_456";
    
    // Test setting cache value
    dao.set_value(key, value)
        .await
        .expect("Failed to set cache value");
    
    // Test getting cache value
    let stored_value = dao.get_value(key)
        .await
        .expect("Failed to get cache value");
    
    assert_eq!(stored_value, Some(value.to_string()), "Expected to get the stored value");
    
    // Test removing cache value
    dao.remove_value(key)
        .await
        .expect("Failed to remove cache value");
    
    let removed_value = dao.get_value(key)
        .await
        .expect("Failed to check for removed cache value");
    
    assert_eq!(removed_value, None, "Expected cache value to be removed");
}

// Test that runs all DAO tests together to verify they work in combination
#[tokio::test]
async fn test_all_daos_together() {
    let client = create_test_db().await;
    
    // Create sample data
    let room_id = random_room_id();
    let user_id = random_user_id();
    let event_id = random_event_id();
    let txn_id = random_transaction_id();
    
    // Test RoomStateDao
    let room_state_dao = RoomStateDao::new(client.clone());
    room_state_dao.set_state_event(
        &room_id, 
        "m.room.name", 
        "", 
        json!({"name": "Test Combined Room"})
    ).await.expect("Failed to set room name");
    
    // Test AccountDataDao
    let account_data_dao = AccountDataDao::new(client.clone());
    account_data_dao.set_room_account_data(
        &room_id,
        "m.fully_read",
        json!({"event_id": event_id.to_string()})
    ).await.expect("Failed to set account data");
    
    // Test PresenceDao
    let presence_dao = PresenceDao::new(client.clone());
    presence_dao.set_presence_event(
        &user_id,
        json!({"presence": "online"})
    ).await.expect("Failed to set presence");
    
    // Test SendQueueDao
    let send_queue_dao = SendQueueDao::new(client.clone());
    let content = RoomMessageEventContent::text_plain("Combined test message");
    let request = QueuedRequestKind::Message {
        content: Box::new(content),
        txn_id: txn_id.clone(),
    };
    let created_at = MilliSecondsSinceUnixEpoch(Utc::now().timestamp_millis() as u64);
    send_queue_dao.add_request_to_queue(
        &room_id,
        txn_id.clone(),
        created_at,
        request,
        1
    ).await.expect("Failed to add request to queue");
    
    // Verify all data is correctly stored
    let room_name = room_state_dao.get_state_event(&room_id, "m.room.name", "")
        .await.expect("Failed to get room name");
    assert!(room_name.is_some(), "Room name should be set");
    assert_eq!(room_name.unwrap()["name"], "Test Combined Room");
    
    let account_data = account_data_dao.get_room_account_data(&room_id, "m.fully_read")
        .await.expect("Failed to get account data");
    assert!(account_data.is_some(), "Account data should be set");
    assert_eq!(account_data.unwrap()["event_id"], event_id.to_string());
    
    let presence = presence_dao.get_presence_event(&user_id)
        .await.expect("Failed to get presence");
    assert!(presence.is_some(), "Presence should be set");
    assert_eq!(presence.unwrap()["presence"], "online");
    
    let queue_request = send_queue_dao.get_request(&room_id, &txn_id)
        .await.expect("Failed to get queued request");
    assert!(queue_request.is_some(), "Queued request should be set");
}

// Add a main function just for cargo run convenience
#[allow(dead_code)]
fn main() {
    println!("Run tests with cargo nextest run");
}