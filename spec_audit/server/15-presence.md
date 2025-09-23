# Matrix Server-Server API - Presence Federation

This specification covers user presence federation using Ephemeral Data Units (EDUs), allowing servers to share real-time presence information across federated networks.

## Overview

The server API for presence is based entirely on exchange of the following EDUs. There are no PDUs or Federation Queries involved.

Servers should only send presence updates for users that the receiving server would be interested in. Such as the receiving server sharing a room with a given user.

Presence information includes the user's current availability state (online, offline, unavailable), activity indicators, and optional status messages.

## Presence States

Matrix defines three standard presence states:

- **`online`** - The user is actively using their client and is available for communication
- **`offline`** - The user is not available and their client is not connected
- **`unavailable`** - The user's client is connected but they are not actively using it (away/idle)

## m.presence EDU

---

An EDU representing presence updates for users of the sending homeserver.

| Name | Type | Description |
| --- | --- | --- |
| `content` | `[Presence Update](https://spec.matrix.org/unstable/server-server-api/#definition-mpresence_presence-update)` | **Required:** The presence updates and requests. |
| `edu_type` | `string` | **Required:** The string `m.presence`. One of: `[m.presence]`. |

### Presence Update Content

| Name | Type | Description |
| --- | --- | --- |
| `push` | `[[User Presence Update](https://spec.matrix.org/unstable/server-server-api/#definition-mpresence_user-presence-update)]` | **Required:** A list of presence updates that the receiving server is likely to be interested in. |

### User Presence Update

| Name | Type | Description |
| --- | --- | --- |
| `currently_active` | `boolean` | True if the user is likely to be interacting with their client. This may be indicated by the user having a `last_active_ago` within the last few minutes. Defaults to false. |
| `last_active_ago` | `integer` | **Required:** The number of milliseconds that have elapsed since the user last did something. |
| `presence` | `string` | **Required:** The presence of the user. One of: `[offline, unavailable, online]`. |
| `status_msg` | `string` | An optional description to accompany the presence. |
| `user_id` | `string` | **Required:** The user ID this presence EDU is for. |

## Example Presence Update

```json
{
  "content": {
    "push": [
      {
        "currently_active": true,
        "last_active_ago": 5000,
        "presence": "online",
        "status_msg": "Making cupcakes",
        "user_id": "@john:matrix.org"
      }
    ]
  },
  "edu_type": "m.presence"
}
```

## Federation Transmission

Presence updates are transmitted through the standard federation transaction mechanism using the `PUT /_matrix/federation/v1/send/{txnId}` endpoint. They are included in the `edus` array of the transaction payload.

### Transaction Example

```json
{
  "origin": "example.org",
  "origin_server_ts": 1234567890000,
  "pdus": [],
  "edus": [
    {
      "content": {
        "push": [
          {
            "currently_active": false,
            "last_active_ago": 300000,
            "presence": "unavailable",
            "status_msg": "In a meeting",
            "user_id": "@alice:example.org"
          },
          {
            "currently_active": true,
            "last_active_ago": 1000,
            "presence": "online",
            "user_id": "@bob:example.org"
          }
        ]
      },
      "edu_type": "m.presence"
    }
  ]
}
```

## Implementation Considerations

### Server Validation

Receiving servers **SHOULD** validate presence updates by:

1. **User Verification**: Confirming the user belongs to the sending server (user ID domain matches sender)
2. **Interest Filtering**: Only processing updates for users the receiving server has interest in (shared rooms)
3. **Authorization**: Ensuring the sending server is authorized to send presence updates for the user

### Presence Interest Calculation

Servers **SHOULD** only send presence updates to servers that have interest in the user:

- The receiving server shares at least one room with the user
- The receiving server has local users who can see the user's presence
- Avoid sending presence updates to servers with no shared context

### Batching Updates

Servers **SHOULD** batch multiple presence updates in a single `m.presence` EDU:

- Aggregate presence changes for multiple users in one transaction
- Reduce federation traffic by combining updates
- Send batches periodically rather than individual updates

### Rate Limiting

Servers **SHOULD** implement rate limiting for presence updates:

- Limit frequency of presence changes per user
- Implement exponential backoff for rapid presence changes
- Avoid flooding federation with excessive presence updates
- Consider aggregating rapid state changes before federation

### Timeout Handling

Servers **SHOULD** implement automatic presence timeouts:

- Transition users to `offline` after extended inactivity
- Clear `currently_active` flags after reasonable timeouts
- Handle network disconnections gracefully
- Reset presence state on server restart if needed

### Privacy Controls

Servers **MAY** implement privacy controls for presence:

- Allow users to disable presence sharing entirely
- Support invisible mode (appear offline while actually online)
- Implement per-room presence visibility settings
- Consider local privacy regulations

## Performance Optimizations

### Caching

Servers **SHOULD** cache presence information:

- Store last known presence state for federated users
- Cache presence information to avoid redundant updates
- Implement efficient presence state storage and retrieval

### Local Optimization

Servers **SHOULD** optimize local presence handling:

- Deliver presence updates to local clients immediately
- Use efficient internal event distribution
- Minimize database queries for presence lookups

### Federation Efficiency

Servers **SHOULD** optimize federation presence traffic:

- Only federate presence to interested servers
- Batch updates across multiple users
- Implement debouncing for rapid presence changes
- Use efficient serialization for EDU payloads

## Security Considerations

### Information Disclosure

Servers **SHOULD** be aware that presence information can disclose:

- User activity patterns and schedules
- Whether users are actively communicating
- Status messages may contain sensitive information
- Presence timing may reveal personal habits

### Spoofing Prevention

Servers **MUST** prevent presence spoofing:

- Only accept presence updates for users belonging to the sending server
- Validate server signatures on federation transactions containing presence EDUs
- Reject presence updates for users not known to the receiving server

### Resource Exhaustion

Servers **SHOULD** protect against presence-based attacks:

- Implement rate limiting on presence update frequency
- Monitor and alert on excessive presence traffic
- Limit the number of presence updates processed per transaction
- Protect against memory exhaustion from presence state storage

### Status Message Filtering

Servers **MAY** implement content filtering for status messages:

- Filter inappropriate or harmful content
- Limit status message length to prevent abuse
- Consider local content policies and regulations
- Log suspicious status message patterns

## Error Handling

### Invalid User

If a presence update references a user not known to the receiving server:

- Silently ignore the presence update
- Log the error for debugging purposes
- Continue processing other presence updates in the batch
- Do not treat this as a transaction failure

### Malformed Updates

If a presence update is malformed:

- Log the error with sufficient detail for debugging
- Skip the malformed update and continue processing others
- Validate required fields are present and correctly typed
- Handle missing optional fields gracefully

### Network Failures

Servers **SHOULD** handle network failures gracefully:

- Presence updates are ephemeral and do not require delivery guarantees
- Do not retry presence updates on failure (unlike PDUs)
- Reset presence state to safe defaults after connection loss
- Implement exponential backoff for federation reconnection

## Integration with Client-Server API

Presence updates received via federation **SHOULD** be delivered to local clients:

- Include in sync responses under presence events
- Deliver via real-time push for active sync connections
- Apply client-side filtering based on user preferences
- Respect client presence visibility settings

### Sync Integration

```json
{
  "next_batch": "s12345",
  "presence": {
    "events": [
      {
        "content": {
          "currently_active": true,
          "last_active_ago": 5000,
          "presence": "online",
          "status_msg": "Making cupcakes"
        },
        "sender": "@john:matrix.org",
        "type": "m.presence"
      }
    ]
  }
}
```

## Advanced Features

### Presence Aggregation

For large rooms, servers **MAY** implement presence aggregation:

- Aggregate similar presence states in large rooms
- Provide summary statistics instead of individual updates
- Implement smart filtering based on room size and activity

### Rich Presence

Servers **MAY** extend presence with additional information:

- Rich status messages with formatting
- Activity-specific presence states
- Location information (with user consent)
- Integration with external services

### Presence History

Servers **MAY** implement presence history tracking:

- Store historical presence information for analytics
- Provide presence history APIs for clients
- Implement data retention policies for privacy
- Consider regulatory requirements for data storage

## Future Considerations

This specification may be extended to support:

- More granular presence states (busy, in-call, etc.)
- Presence subscriptions with explicit opt-in/opt-out
- Encrypted presence information for sensitive contexts
- Presence federation across different network protocols
- Integration with external presence systems

## Interoperability

Servers implementing presence federation **SHOULD**:

- Follow the standard presence states defined in this specification
- Handle unknown presence states gracefully (default to appropriate fallback)
- Implement consistent timeout behavior across implementations
- Support standard status message conventions