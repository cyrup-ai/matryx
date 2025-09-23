# Matrix Server-Server API: Receipts

## Overview

Receipts are Ephemeral Data Units (EDUs) used to communicate read markers for events in Matrix rooms. They provide a way for users to indicate which events they have read, enabling features like read receipt indicators and unread message counts across federated homeservers.

Unlike Persistent Data Units (PDUs), receipts are ephemeral and do not form part of the room's permanent event graph. They are transmitted between servers as part of federation transactions but are not stored as part of the room's historical timeline.

## Receipt Types

Currently, Matrix supports one primary type of receipt:

### Read Receipts (m.read)

Read receipts indicate where in the event graph a user has read up to. They represent the "read marker" position for a particular user in a room, showing which events the user has acknowledged reading.

**Important behavior:**
- Read receipts for events that a user sent do not need to be transmitted
- It is implied that by sending an event, the user has read up to that event
- Receipts only need to track external events that the user has acknowledged reading

## m.receipt EDU

The `m.receipt` EDU is used to transmit receipt updates between homeservers during federation.

### EDU Structure

```json
{
  "edu_type": "m.receipt",
  "content": {
    "[room_id]": {
      "m.read": {
        "[user_id]": {
          "data": {
            "ts": [timestamp]
          },
          "event_ids": ["[event_id]", ...]
        }
      }
    }
  }
}
```

### Schema Definition

| Field | Type | Description |
|-------|------|-------------|
| `edu_type` | `string` | **Required:** Must be `"m.receipt"` |
| `content` | `object` | **Required:** Receipt data organized by room ID |

#### Content Structure (by Room ID)

| Field | Type | Description |
|-------|------|-------------|
| `[room_id]` | `object` | **Required:** Receipts for a specific room. Key is the room ID |

#### Room Receipts Structure

| Field | Type | Description |
|-------|------|-------------|
| `m.read` | `object` | **Required:** Read receipts for users in the room |

#### User Read Receipt Structure

| Field | Type | Description |
|-------|------|-------------|
| `[user_id]` | `object` | **Required:** Receipt data for a specific user. Key is the user ID |

#### Read Receipt Data Structure

| Field | Type | Description |
|-------|------|-------------|
| `data` | `object` | **Required:** Metadata for the read receipt |
| `event_ids` | `[string]` | **Required:** The extremity event IDs that the user has read up to |

#### Read Receipt Metadata

| Field | Type | Description |
|-------|------|-------------|
| `ts` | `integer` | **Required:** POSIX timestamp in milliseconds when the receipt was recorded |

## Federation Protocol

### Transmission

Receipts are transmitted as part of federation transactions using the `PUT /_matrix/federation/v1/send/{txnId}` endpoint. They are included in the `edus` array of the transaction payload.

### Processing Requirements

When a homeserver receives receipt EDUs, it must:

1. **Validate the EDU format** - Ensure the EDU conforms to the m.receipt schema
2. **Check room membership** - Verify the user has permission to send receipts for the room  
3. **Update local state** - Store the receipt information for local users and sync
4. **Selective updating** - Only update receipt entries that are explicitly listed in the EDU
5. **Preserve existing receipts** - Do not remove receipts that are not mentioned in the current EDU

### Receipt Aggregation Rules

When processing receipts, servers should follow these aggregation rules:

1. **Latest timestamp wins** - If multiple receipts exist for the same user/room, use the most recent
2. **Event ID advancement** - New receipts should represent forward progress in the event timeline
3. **Multiple event IDs** - The `event_ids` array represents extremities the user has read up to
4. **Backwards compatibility** - Handle receipt formats from different Matrix versions appropriately

## Security Considerations

### Anti-Spoofing Measures

1. **User authentication** - Only accept receipts for users belonging to the sending server
2. **Room membership validation** - Verify users have appropriate access to the room
3. **Origin verification** - Validate receipts come from the authoritative homeserver for each user

### Privacy Protection

1. **User consent** - Respect user privacy settings regarding read receipt visibility
2. **Room privacy** - Consider room visibility and access controls when processing receipts
3. **Data retention** - Apply appropriate retention policies for ephemeral receipt data

### Resource Management

1. **Rate limiting** - Apply reasonable limits to prevent receipt spam
2. **Storage efficiency** - Implement efficient storage for ephemeral receipt data
3. **Memory management** - Avoid excessive memory usage for receipt processing

## Implementation Guidelines

### Server Processing Pipeline

1. **Receipt validation** - Validate EDU format and required fields
2. **Authentication** - Verify the sending server can send receipts for the specified users  
3. **Room access control** - Check user permissions for the target room
4. **State updates** - Update local receipt state for affected users
5. **Client notification** - Notify local clients of receipt updates via sync

### Storage Considerations

1. **Ephemeral nature** - Receipts are transient and don't require permanent storage
2. **Sync integration** - Include receipt updates in client-server sync responses
3. **Cleanup policies** - Implement cleanup for old or stale receipt data
4. **Performance optimization** - Use efficient data structures for receipt lookups

### Error Handling

1. **Malformed EDUs** - Gracefully handle invalid receipt formats
2. **Access denied** - Handle cases where users lack room permissions
3. **Unknown rooms** - Process receipts for rooms the server doesn't participate in
4. **Partial failures** - Continue processing valid receipts even if some fail

## Access Control Lists (ACLs)

Receipt EDUs are subject to room Access Control Lists (ACLs):

- **Room-specific EDUs** - Receipts are considered local to specific rooms
- **ACL enforcement** - All receipts for a room must be ignored if the sending server is denied access
- **Per-room validation** - ACLs are applied based on each room ID mentioned in the receipt EDU
- **Complete rejection** - If access is denied, the entire receipt set for that room is ignored

## Example Implementation

### Complete Receipt EDU Example

```json
{
  "edu_type": "m.receipt",
  "content": {
    "!room1:example.org": {
      "m.read": {
        "@alice:example.org": {
          "data": {
            "ts": 1533358089009
          },
          "event_ids": [
            "$event123:example.org"
          ]
        },
        "@bob:example.org": {
          "data": {
            "ts": 1533358095123
          },
          "event_ids": [
            "$event124:example.org",
            "$event125:example.org"
          ]
        }
      }
    },
    "!room2:example.org": {
      "m.read": {
        "@alice:example.org": {
          "data": {
            "ts": 1533358100456
          },
          "event_ids": [
            "$event200:example.org"
          ]
        }
      }
    }
  }
}
```

### Processing Logic Example

```rust
// Example processing logic for receipt EDUs
fn process_receipt_edu(edu: &ReceiptEDU, origin_server: &str) -> Result<(), ReceiptError> {
    for (room_id, room_receipts) in &edu.content {
        // Check room ACLs
        if is_server_denied_access(room_id, origin_server)? {
            log::warn!("Ignoring receipts for {} from denied server {}", room_id, origin_server);
            continue;
        }
        
        // Process read receipts
        if let Some(read_receipts) = &room_receipts.m_read {
            for (user_id, receipt_data) in read_receipts {
                // Validate user belongs to origin server
                if !user_belongs_to_server(user_id, origin_server) {
                    return Err(ReceiptError::InvalidUser);
                }
                
                // Check room membership
                if !is_user_in_room(user_id, room_id)? {
                    log::warn!("Ignoring receipt from {} not in room {}", user_id, room_id);
                    continue;
                }
                
                // Update receipt state
                update_user_receipt(room_id, user_id, receipt_data)?;
                
                // Notify local clients via sync
                notify_receipt_update(room_id, user_id, receipt_data)?;
            }
        }
    }
    
    Ok(())
}
```

## Integration with Client-Server API

Receipt information processed through federation should be made available to local clients through:

1. **Sync responses** - Include receipt updates in room data
2. **Real-time notifications** - Push receipt updates to active client connections  
3. **Receipt queries** - Allow clients to query current receipt state
4. **Unread counts** - Use receipt data to calculate unread message indicators

## Performance Optimizations

### Batching Strategies

1. **Receipt aggregation** - Batch multiple receipts together in single EDUs
2. **Debouncing** - Avoid sending excessive receipt updates for rapid reading
3. **Differential updates** - Only send changed receipts, not complete state
4. **Compression** - Use efficient encoding for receipt data transmission

### Caching Considerations

1. **Memory caching** - Cache recent receipt state for quick access
2. **Persistent storage** - Balance performance with storage requirements
3. **Cleanup policies** - Remove old receipt data to prevent memory leaks
4. **Index optimization** - Use appropriate indices for receipt lookups

### Network Efficiency

1. **Transaction bundling** - Include receipts with other federation data
2. **Selective transmission** - Only send receipts to interested servers
3. **Rate limiting** - Prevent receipt flood attacks
4. **Protocol versioning** - Support efficient receipt formats across versions

## Monitoring and Observability

### Metrics to Track

1. **Receipt throughput** - Number of receipts processed per second
2. **Processing latency** - Time to process receipt EDUs
3. **Error rates** - Failed receipt processing attempts  
4. **Storage usage** - Memory and disk usage for receipt data
5. **Network traffic** - Bandwidth usage for receipt transmission

### Logging Recommendations

1. **Receipt events** - Log receipt processing with appropriate detail levels
2. **Error conditions** - Log validation failures and processing errors
3. **Performance data** - Track processing times and resource usage
4. **Security events** - Log access control violations and suspicious activity

## Future Considerations

### Protocol Evolution

1. **New receipt types** - Support for additional receipt semantics
2. **Enhanced metadata** - Extended receipt information and context
3. **Privacy improvements** - Better user control over receipt visibility
4. **Performance enhancements** - More efficient receipt processing and transmission

### Implementation Improvements

1. **Better aggregation** - Smarter receipt batching and compression
2. **Real-time optimization** - Faster receipt delivery and processing
3. **Storage efficiency** - More compact receipt storage formats
4. **Integration enhancements** - Better client-server receipt synchronization

This specification provides a comprehensive foundation for implementing Matrix receipt federation while maintaining compatibility with existing Matrix ecosystem components and enabling future protocol enhancements.