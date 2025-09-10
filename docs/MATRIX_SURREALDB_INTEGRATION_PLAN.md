# Matrix-SurrealDB 3.0 Integration Plan

## Overview

Based on comprehensive analysis of SurrealDB 3.0 source code, this plan leverages SurrealDB's native authentication, LiveQuery, and distributed features for Matrix homeserver implementation.

## Authentication Integration

### Matrix Access Token → SurrealDB JWT Mapping

**Implementation**: Custom JWT claims in SurrealDB tokens
```rust
// Matrix session data in JWT custom claims
{
  "matrix_user_id": "@user:example.com",
  "matrix_device_id": "ABCDEFGHIJ", 
  "matrix_access_token": "syt_...",
  "matrix_refresh_token": "syr_...",
  "NS": "matrix",
  "DB": "homeserver", 
  "AC": "matrix_client"
}
```

**SurrealDB Session Creation**:
```rust
let session = Session::for_record(
    "matrix",           // namespace
    "homeserver",       // database  
    "matrix_client",    // access method
    user_record_id      // Matrix user record
).with_rt(true);        // Enable real-time queries
```

### Federation Authentication → SurrealDB Namespace Auth

**Server-to-Server**: Map X-Matrix signatures to SurrealDB namespace-level authentication
```rust
let federation_session = Session::for_level(
    Level::Namespace("matrix".to_string()),
    Role::Editor  // Federation servers get Editor role
);
```

## Real-time Event Distribution

### LiveQuery Implementation for Matrix Events

**Room Events**: Subscribe to room event streams
```sql
LIVE SELECT * FROM room_events 
WHERE room_id = $room_id 
AND (sender != $user_id OR event_type IN ['m.room.member', 'm.room.power_levels']);
```

**Presence Updates**: Real-time user presence
```sql  
LIVE SELECT user_id, presence, last_active_ago, status_msg 
FROM user_presence 
WHERE user_id IN $followed_users;
```

**Typing Indicators**: Ephemeral events
```sql
LIVE SELECT * FROM typing_events 
WHERE room_id = $room_id AND user_id != $user_id;
```

### Authentication Context Preservation

SurrealDB LiveQueries automatically preserve the original user's authentication context:
- Each LiveQuery stores `auth: Option<Auth>` and `session: Option<Value>`
- Permission checks use the original user's context, not the current transaction
- Session variables (`$access`, `$auth`, `$token`, `$session`) available in queries

## Permission System Integration

### Matrix Power Levels → SurrealDB Roles

**Mapping Strategy**:
- `power_level >= 100` → `Role::Owner` (room admin)
- `power_level >= 50` → `Role::Editor` (moderator)  
- `power_level >= 0` → `Role::Viewer` (regular user)
- `power_level < 0` → No access (banned)

**Implementation**: Custom permission checking in DAO layer
```rust
impl EventRepository {
    async fn check_matrix_permissions(&self, 
        user_id: &str, 
        room_id: &str, 
        action: &str
    ) -> Result<()> {
        // Get user's power level in room
        let power_level = self.get_user_power_level(user_id, room_id).await?;
        
        // Map to SurrealDB role
        let role = match power_level {
            p if p >= 100 => Role::Owner,
            p if p >= 50 => Role::Editor,
            p if p >= 0 => Role::Viewer,
            _ => return Err(Error::Forbidden),
        };
        
        // Check action permissions
        let auth = Auth::for_record(user_id.to_string(), "matrix", "homeserver", "matrix_client");
        let resource = Resource::new(room_id.to_string(), ResourceKind::Table, 
                                   Level::Database("matrix".to_string(), "homeserver".to_string()));
        
        auth.is_allowed(Action::from(action), &resource)
    }
}
```

## Session Management

### Matrix Session → SurrealDB Session Integration

**Session Creation**: Map Matrix login to SurrealDB session
```rust
pub async fn create_matrix_session(
    user_id: &str,
    device_id: &str, 
    access_token: &str,
    refresh_token: Option<&str>
) -> Result<Session> {
    let custom_claims = map! {
        "matrix_user_id".to_string() => json!(user_id),
        "matrix_device_id".to_string() => json!(device_id),
        "matrix_access_token".to_string() => json!(access_token),
        "matrix_refresh_token".to_string() => json!(refresh_token),
    };
    
    let claims = Claims {
        ns: Some("matrix".to_string()),
        db: Some("homeserver".to_string()),
        ac: Some("matrix_client".to_string()),
        id: Some(user_id.to_string()),
        exp: Some(Utc::now().timestamp() + 3600), // 1 hour expiration
        custom_claims: Some(custom_claims),
        ..Default::default()
    };
    
    // Create JWT token and session
    let token = create_jwt_token(claims)?;
    let session = Session::for_record("matrix", "homeserver", "matrix_client", 
                                    Value::from(user_id))
        .with_rt(true);
    
    Ok(session)
}
```

**Session Validation**: Leverage SurrealDB's built-in expiration
```rust
pub fn validate_session(session: &Session) -> Result<()> {
    if session.expired() {
        return Err(Error::TokenExpired);
    }
    Ok(())
}
```

## Distributed Architecture

### Node-based LiveQuery Distribution

SurrealDB 3.0's node system supports Matrix federation:
- Each homeserver instance gets a unique `node_id` 
- LiveQueries are distributed across nodes using `/node/{node_id}/lq/{query_id}` keys
- Cross-node event delivery for federated rooms
- Automatic node heartbeat and garbage collection

### Federation Event Routing

**Local Events**: Process on local node
```rust
if opt.id()? == lv.node.0 {
    // Process locally - send notification
    let notification = Notification {
        id: lv.id.0,
        action: Action::Create,
        result: event_data,
    };
    chn.send(notification).await?;
}
```

**Remote Events**: Route to federation layer
```rust
else {
    // Send to federation message broker
    federation_sender.send_to_remote_servers(
        &event.room_id,
        &event
    ).await?;
}
```

## Implementation Phases

### Phase 1: Authentication Foundation
1. Implement Matrix JWT token creation with SurrealDB custom claims
2. Create session management layer mapping Matrix sessions to SurrealDB sessions  
3. Implement permission checking using SurrealDB's role system
4. Add Matrix access token validation middleware

### Phase 2: Real-time Event System
1. Implement LiveQuery subscriptions for room events, presence, typing
2. Create event notification routing using SurrealDB's native notification system
3. Add authentication context preservation for live queries
4. Implement session variable injection for Matrix-specific data

### Phase 3: Federation Integration  
1. Map server-server authentication to SurrealDB namespace-level auth
2. Implement distributed LiveQuery routing for federated rooms
3. Add cross-node event delivery using SurrealDB's node system
4. Create federation message broker integration

### Phase 4: Optimization
1. Implement caching strategies using SurrealDB's built-in cache invalidation
2. Add performance monitoring for LiveQuery subscriptions
3. Optimize permission checking with role-based caching
4. Add horizontal scaling support using SurrealDB's distributed features

## Technical Benefits

1. **Zero External Dependencies**: No Redis, message queues, or external auth systems needed
2. **Native Real-time**: SurrealDB LiveQueries provide Matrix-compliant real-time events  
3. **Distributed by Design**: Built-in support for multi-node Matrix federation
4. **Authentication Integration**: JWT tokens with Matrix metadata in custom claims
5. **Permission Enforcement**: Automatic permission checking in live queries
6. **Session Management**: Built-in expiration, refresh, and metadata handling

This integration leverages SurrealDB 3.0's native capabilities to provide a complete Matrix homeserver backend without external dependencies.