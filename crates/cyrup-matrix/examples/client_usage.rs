use futures::StreamExt;
use tokio::runtime::Runtime;
use cyrup_matrix::{
    client::CyrumClient,
    store::{CyrumStateStore, SurrealStateStore}
};

fn main() {
    // Create a tokio runtime for async operations
    let runtime = Runtime::new().expect("Failed to create runtime");
    
    // Run everything in the runtime context
    runtime.block_on(async {
        // Create a state store
        let db_path = "./matrix_data.db";
        let surreal_store = SurrealStateStore::new(db_path).await.expect("Failed to create store");
        let store = CyrumStateStore::new(surreal_store);
        
        // Create a client with the store
        let homeserver = "https://matrix.org";
        let client = CyrumClient::with_config(homeserver, store, None, None)
            .expect("Failed to create client");
        
        // Login
        println!("Logging in...");
        client.login("username", "password").await
            .expect("Failed to login");
        
        println!("Logged in as: {:?}", client.user_id());
        
        // Start syncing with the server
        println!("Starting sync...");
        client.sync_background().await.expect("Failed to start sync");
        
        // Subscribe to room messages
        let mut message_stream = client.subscribe_to_messages();
        
        // Process incoming messages
        println!("Listening for messages (Ctrl+C to exit)...");
        while let Some(message_result) = message_stream.next().await {
            if let Ok((room_id, event)) = message_result {
                println!("Message in room {}: {:?}", room_id, event.content);
                
                // Respond to messages that mention us
                if let Some(user_id) = client.user_id() {
                    if event.content.body().contains(&user_id.to_string()) {
                        let room = client.get_room(&room_id)
                            .expect("Failed to get room");
                        
                        println!("Sending reply...");
                        let reply = format!("Hello, I heard you mention me!");
                        client.send_text_message(&room_id, &reply).await
                            .expect("Failed to send message");
                    }
                }
            }
        }
        
        // Cleanup
        client.stop_sync();
        println!("Sync stopped");
    });
}