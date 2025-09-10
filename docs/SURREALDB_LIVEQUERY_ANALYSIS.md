# SurrealDB LiveQuery System Analysis

## Overview
This document provides a comprehensive analysis of the SurrealDB LiveQuery system based on the forked codebase at `./forks/surrealdb`, focusing on its architecture, authentication context preservation, and real-time notification mechanisms for Matrix protocol integration.

## Core LiveQuery Architecture

### LiveStatement Structure
```rust
pub struct LiveStatement {
    pub id: Uuid,           // Unique query identifier
    pub node: Uuid,         // Node ID for distributed queries
    pub expr: Fields,       // SELECT fields to return
    pub what: Expr,         // FROM table expression
    pub cond: Option<Cond>, // WHERE clause conditions
    pub fetch: Option<Fetchs>, // FETCH clause for related data
    
    // Authentication context preservation
    pub(crate) auth: Option<Auth>,     // User's auth at query creation
    pub(crate) session: Option<Value>, // Session data at query creation
}
```

### Key Components

#### 1. Authentication Context Preservation
- **Critical Feature**: LiveQuery preserves the original user's authentication context (`auth`) and session data (`session`) when the query is created
- **Security Model**: All notifications are processed using the **original creator's permissions**, not the current transaction's permissions
- **Matrix Integration**: Perfect for Matrix where room events must respect the original user's power levels and permissions

#### 2. Distributed Node Architecture
- **Node ID**: Each LiveQuery is associated with a specific node (`node: Uuid`)
- **Local Processing**: Notifications are only sent if `opt.id()? == lv.node.0` (current node matches query node)
- **Federation Ready**: Supports distributed Matrix homeserver federation via node-based routing

#### 3. Real-time Notification System
```rust
pub struct Notification {
    pub id: Uuid,        // LiveQuery ID
    pub action: Action,  // CREATE/UPDATE/DELETE/KILLED
    pub record: Value,   // Record ID that changed
    pub result: Value,   // Computed result data
}

pub enum Action {
    Create,  // New record created
    Update,  // Existing record modified
    Delete,  // Record deleted
    Killed,  // Query terminated
}
```

## LiveQuery Processing Flow

### 1. Query Creation (`compute` method)
1. **Validation**: Checks realtime enabled, valid DB context
2. **Auth Capture**: Stores current user's `auth` and `session` context
3. **Storage**: Persists query in two locations:
   - Node-specific: `crate::key::node::lq::new(nid, id)`
   - Table-specific: `crate::key::table::lq::new(ns, db, &tb, id)`
4. **Cache Refresh**: Updates table cache for live queries

### 2. Event Processing (`process_table_lives`)
1. **Change Detection**: Only processes if document changed
2. **Query Retrieval**: Gets all LiveQueries for the affected table
3. **Context Recreation**: Creates new context with **original user's auth/session**
4. **Permission Checks**: Validates against original user's permissions
5. **Notification Generation**: Sends typed notifications via channel

### 3. Context Variables Available in LiveQueries
```rust
// Session context (from original user)
lqctx.add_value("access", sess.pick(AC.as_ref()).into());
lqctx.add_value("auth", sess.pick(RD.as_ref()).into());
lqctx.add_value("token", sess.pick(TK.as_ref()).into());
lqctx.add_value("session", sess.clone().into());

// Event context (current change)
lqctx.add_value("event", met.into());    // CREATE/UPDATE/DELETE
lqctx.add_value("value", current.clone()); // Current document state
lqctx.add_value("after", current);       // Same as $value
lqctx.add_value("before", initial);      // Document state before change
```

## Matrix Protocol Integration Points

### 1. Room Event Subscriptions
```sql
-- Matrix room events LiveQuery example
LIVE SELECT event_type, content, sender, timestamp 
FROM room_events 
WHERE room_id = $room_id 
  AND (
    -- User has joined the room
    $auth.user_id IN (SELECT user_id FROM room_memberships WHERE room_id = $room_id AND membership = 'join')
    OR 
    -- Event is world-readable (public room)
    room_id IN (SELECT room_id FROM rooms WHERE visibility = 'public')
  );
```

### 2. User Presence Updates
```sql
-- Matrix presence LiveQuery example
LIVE SELECT user_id, presence, last_active_ago, status_msg
FROM user_presence 
WHERE user_id IN (
  -- Users in rooms where auth user is joined
  SELECT DISTINCT sender FROM room_events 
  WHERE room_id IN (
    SELECT room_id FROM room_memberships 
    WHERE user_id = $auth.user_id AND membership = 'join'
  )
);
```

### 3. Device Key Updates
```sql
-- Matrix device keys LiveQuery example  
LIVE SELECT user_id, device_id, keys, signatures
FROM device_keys
WHERE user_id = $auth.user_id
   OR user_id IN (
     -- Users in shared rooms
     SELECT DISTINCT sender FROM room_events
     WHERE room_id IN (
       SELECT room_id FROM room_memberships 
       WHERE user_id = $auth.user_id AND membership = 'join'
     )
   );
```

## Security Model

### Permission Enforcement
1. **Query Creation**: Uses current user's auth to create LiveQuery
2. **Notification Processing**: **Always** uses original creator's permissions
3. **WHERE Clause**: Evaluated with original user's context variables
4. **Table Permissions**: Checked against original user's access level
5. **Field Projection**: Computed fields respect original user's permissions

### Matrix Power Levels Integration
- LiveQuery WHERE clauses can reference `$auth.power_level` 
- Room state changes respect original user's power level at query creation time
- Administrative events (bans, kicks) only visible to users with sufficient power

## Performance Characteristics

### Storage Efficiency
- **Dual Indexing**: Node-based and table-based indexes for fast lookup
- **Cache Integration**: Table cache tracks live query versions
- **Transaction Isolation**: Each notification processed in original user's context

### Scalability Features
- **Node Distribution**: Queries tied to specific nodes for horizontal scaling
- **Channel-based**: Asynchronous notification delivery via channels
- **Batch Processing**: Multiple LiveQueries processed per document change

## Integration Recommendations for Matrix

### 1. Authentication Bridge
```rust
// Convert Matrix access token to SurrealDB Auth
impl From<MatrixAccessToken> for surrealdb::iam::Auth {
    fn from(token: MatrixAccessToken) -> Self {
        // Map Matrix user to SurrealDB auth context
        // Include power levels, room memberships, etc.
    }
}
```

### 2. Session Context Mapping
```rust
// Matrix session data for LiveQuery context
let session_data = map! {
    "user_id" => matrix_user_id,
    "device_id" => matrix_device_id,
    "access_token" => matrix_access_token,
    "power_levels" => user_power_levels_map,
    "room_memberships" => user_room_memberships,
};
```

### 3. Real-time Event Streaming
- Use SurrealDB's notification channel as Matrix `/sync` event source
- Map SurrealDB `Action` enum to Matrix event types
- Preserve Matrix event ordering through SurrealDB timestamps

## Conclusion

SurrealDB's LiveQuery system provides an ideal foundation for Matrix protocol real-time features:

- **Authentication Context Preservation** ensures security across distributed operations
- **Node-based Architecture** supports Matrix federation requirements  
- **Real-time Notifications** provide efficient `/sync` endpoint implementation
- **Flexible Querying** allows complex Matrix permission models
- **Performance Optimizations** handle high-throughput Matrix deployments

The system's design aligns perfectly with Matrix's security model where permissions are evaluated at event creation time and preserved for real-time delivery.