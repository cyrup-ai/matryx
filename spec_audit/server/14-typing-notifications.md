# Matrix Server-Server API - Typing Notifications

This specification covers typing notification federation using Ephemeral Data Units (EDUs), allowing servers to share real-time typing indicators across federated rooms.

## Overview

When a server's users send typing notifications, those notifications need to be sent to other servers in the room so their users are aware of the same state. Receiving servers should verify that the user is in the room, and is a user belonging to the sending server.

Typing notifications are transmitted as Ephemeral Data Units (EDUs) rather than Persistent Data Units (PDUs), as they represent transient state that doesn't need to be stored long-term or form part of the room's permanent event graph.

## Ephemeral Data Units (EDUs)

EDUs, by comparison to PDUs, do not have an ID, a room ID, or a list of "previous" IDs. They are intended to be non-persistent data such as user presence, typing notifications, etc.

### EDU Structure

An ephemeral data unit has the following structure:

| Name | Type | Description |
| --- | --- | --- |
| `content` | `object` | **Required:** The content of the ephemeral message. |
| `edu_type` | `string` | **Required:** The type of ephemeral message. |

### Example EDU

```json
{
  "content": {
    "key": "value"
  },
  "edu_type": "m.presence"
}
```

## Typing Notifications

### m.typing

---

A typing notification EDU for a user in a room.

| Name | Type | Description |
| --- | --- | --- |
| `content` | `[Typing Notification](https://spec.matrix.org/unstable/server-server-api/#definition-mtyping_typing-notification)` | **Required:** The typing notification. |
| `edu_type` | `string` | **Required:** The string `m.typing`. One of: `[m.typing]`. |

#### Typing Notification Content

| Name | Type | Description |
| --- | --- | --- |
| `room_id` | `string` | **Required:** The room where the user's typing status has been updated. |
| `typing` | `boolean` | **Required:** Whether the user is typing in the room or not. |
| `user_id` | `string` | **Required:** The user ID that has had their typing status changed. |

### Example Typing Notification

```json
{
  "content": {
    "room_id": "!somewhere:matrix.org",
    "typing": true,
    "user_id": "@john:matrix.org"
  },
  "edu_type": "m.typing"
}
```

### Example Typing Stop Notification

```json
{
  "content": {
    "room_id": "!somewhere:matrix.org",
    "typing": false,
    "user_id": "@john:matrix.org"
  },
  "edu_type": "m.typing"
}
```

## Federation Transmission

Typing notifications are transmitted through the standard federation transaction mechanism using the `PUT /_matrix/federation/v1/send/{txnId}` endpoint. They are included in the `edus` array of the transaction payload.

### Transaction Example

```json
{
  "origin": "example.org",
  "origin_server_ts": 1234567890000,
  "pdus": [],
  "edus": [
    {
      "content": {
        "room_id": "!somewhere:matrix.org",
        "typing": true,
        "user_id": "@alice:example.org"
      },
      "edu_type": "m.typing"
    }
  ]
}
```

## Implementation Considerations

### Server Validation

Receiving servers **SHOULD** validate typing notifications by:

1. **User Verification**: Confirming the user belongs to the sending server (user ID domain matches sender)
2. **Room Membership**: Verifying the user is actually in the specified room
3. **Authorization**: Ensuring the sending server is authorized to send typing notifications for the user

### Rate Limiting

Servers **SHOULD** implement rate limiting for typing notifications to prevent spam:

- Limit frequency of typing state changes per user per room
- Implement exponential backoff for rapid typing notifications
- Consider aggregating rapid typing changes before federation

### Timeout Handling

Servers **SHOULD** implement automatic timeout for typing notifications:

- Typing state should automatically reset to `false` after a reasonable timeout (typically 30 seconds)
- Clients should periodically send typing updates to maintain active typing state
- Network failures should not leave users permanently "typing"

### Room State Consistency

Servers **SHOULD** maintain typing state consistency:

- Track typing state per user per room
- Clear typing state when users leave rooms
- Reset typing state on server restart or connection loss

### Privacy Considerations

Servers **MAY** implement privacy controls for typing notifications:

- Allow users to disable sending typing notifications
- Allow rooms to disable typing notifications entirely
- Consider typing notifications as potentially sensitive user activity data

## Performance Optimizations

### Batching

Servers **MAY** batch multiple typing notifications in a single transaction to reduce federation overhead, especially when multiple users are typing simultaneously.

### Debouncing

Servers **SHOULD** implement debouncing to avoid sending excessive typing notifications:

- Wait a brief period before sending typing stop notifications
- Aggregate rapid typing start/stop changes
- Avoid sending redundant typing state updates

### Local Optimization

Servers **SHOULD** optimize local delivery before federation:

- Deliver typing notifications to local clients immediately
- Only federate typing notifications to servers with interested users
- Use efficient local event distribution mechanisms

## Security Considerations

### Spoofing Prevention

Servers **MUST** prevent typing notification spoofing:

- Only accept typing notifications for users belonging to the sending server
- Validate server signatures on federation transactions containing EDUs
- Reject typing notifications for users not in the specified room

### Resource Exhaustion

Servers **SHOULD** protect against resource exhaustion:

- Implement rate limiting on typing notification frequency
- Limit the number of concurrent typing users tracked per room
- Monitor and alert on excessive typing notification volume

### Information Leakage

Servers **SHOULD** be aware that typing notifications can leak information:

- Typing patterns may reveal user activity schedules
- Typing in private rooms may indicate sensitive conversations
- Consider local privacy laws regarding user activity tracking

## Error Handling

### Invalid User

If a typing notification references a user not in the room, servers **SHOULD**:

- Log the error for debugging
- Silently ignore the notification (don't forward to clients)
- Continue processing other EDUs in the transaction

### Unknown Room

If a typing notification references an unknown room, servers **SHOULD**:

- Silently ignore the notification
- Not treat this as a transaction failure
- Continue processing other EDUs normally

### Malformed Notifications

If a typing notification is malformed, servers **SHOULD**:

- Log the error with details
- Ignore the malformed notification
- Continue processing the transaction

## Integration with Client-Server API

Typing notifications received via federation **SHOULD** be delivered to local clients through the standard Client-Server API mechanisms:

- Include in sync responses under room ephemeral events
- Deliver via real-time push for active sync connections  
- Apply client-side filtering based on user preferences

## Future Considerations

This specification may be extended in future versions to support:

- Typing duration estimates
- Multiple typing states (composing, editing, etc.)
- Rich typing indicators with context
- Group typing aggregation for large rooms