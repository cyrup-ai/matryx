# Room State Management

## Overview

The *state* of a room is a map of `(event_type, state_key)` to `event_id`. Each room starts with an empty state, and each state event which is accepted into the room updates the state of that room.

Where each event has a single `prev_event`, it is clear what the state of the room after each event should be. However, when two branches in the event graph merge, the state of those branches might differ, so a *state resolution* algorithm must be used to determine the resultant state.

## Room State Resolution

For example, consider the following event graph (where the oldest event, E0, is at the top):

```
E0
  |
  E1
 /  \
E2  E4
|    |
E3   |
 \  /
  E5
```

Suppose E3 and E4 are both `m.room.name` events which set the name of the room. What should the name of the room be at E5?

The algorithm to be used for state resolution depends on the room version. For a description of each room version's algorithm, please see the [room version specification](https://spec.matrix.org/unstable/rooms/).

## Event Validation Pipeline

Whenever a server receives an event from a remote server, the receiving server must ensure that the event:

1. **[Changed in `v1.16`]** Is a valid event, otherwise it is dropped. For an event to be valid, it must comply with the event format of that [room version](https://spec.matrix.org/unstable/rooms/). For some room versions, a `room_id` may also be required on the event in order to determine the room version to check the event against. See the event format section of the [room version specifications](https://spec.matrix.org/unstable/rooms/) for details on when it is required.

2. Passes signature checks, otherwise it is dropped.

3. Passes hash checks, otherwise it is redacted before being processed further.

4. Passes authorization rules based on the event's auth events, otherwise it is rejected.

5. Passes authorization rules based on the state before the event, otherwise it is rejected.

6. Passes authorization rules based on the current state of the room, otherwise it is "soft failed".

Further details of these checks, and how to handle failures, are described below.

The [Signing Events](https://spec.matrix.org/unstable/server-server-api/#signing-events) section has more information on which hashes and signatures are expected on events, and how to calculate them.

## Authorization Definitions

### Required Power Level

A given event type has an associated *required power level*. This is given by the current [`m.room.power_levels`](https://spec.matrix.org/unstable/client-server-api/#mroompower_levels) event. The event type is either listed explicitly in the `events` section or given by either `state_default` or `events_default` depending on if the event is a state event or not.

### Invite Level, Kick Level, Ban Level, Redact Level

The levels given by the `invite`, `kick`, `ban`, and `redact` properties in the current [`m.room.power_levels`](https://spec.matrix.org/unstable/client-server-api/#mroompower_levels) state. The invite level defaults to 0 if unspecified. The kick level, ban level and redact level each default to 50 if unspecified.

### Target User

For an [`m.room.member`](https://spec.matrix.org/unstable/client-server-api/#mroommember) state event, the user given by the `state_key` of the event.

## Authorization Rules

The rules governing whether an event is authorized depends on a set of state. A given event is checked multiple times against different sets of state, as specified above. Each room version can have a different algorithm for how the rules work, and which rules are applied. For more detailed information, please see the [room version specification](https://spec.matrix.org/unstable/rooms/).

### Auth Events Selection

The `auth_events` field of a PDU identifies the set of events which give the sender permission to send the event. The `auth_events` for the `m.room.create` event in a room is empty; for other events, it should be the following subset of the room state:

- **[Changed in `v1.16`]** Depending on the [room version](https://spec.matrix.org/unstable/rooms/), the `m.room.create` event.
- The current `m.room.power_levels` event, if any.
- The sender's current `m.room.member` event, if any.
- If type is `m.room.member`:
  - The target's current `m.room.member` event, if any.
  - If `membership` is `join`, `invite` or `knock`, the current `m.room.join_rules` event, if any.
  - If membership is `invite` and `content` contains a `third_party_invite` property, the current `m.room.third_party_invite` event with `state_key` matching `content.third_party_invite.signed.token`, if any.
  - If `membership` is `join`, `content.join_authorised_via_users_server` is present, and the [room version supports restricted rooms](https://spec.matrix.org/unstable/rooms/#feature-matrix), then the `m.room.member` event with `state_key` matching `content.join_authorised_via_users_server`.

## Event Rejection

If an event is rejected it should neither be relayed to clients nor be included as a prev event in any new events generated by the server. Subsequent events from other servers that reference rejected events should be allowed if they still pass the auth rules. The state used in the checks should be calculated as normal, except not updating with the rejected event where it is a state event.

If an event in an incoming transaction is rejected, this should not cause the transaction request to be responded to with an error response.

### Soft Failure

Soft failure is a mechanism where events that fail authorization rules based on the current state of the room are still stored and can be referenced by other events, but are not included in the current room state and are not sent to clients.

When an event soft fails:
- The event is stored by the server
- The event can be used as a prev_event by subsequent events
- The event is not included in the room's current state
- The event is not sent to clients in sync responses
- The event can still be retrieved via the `/event/{eventId}` API

## State Federation Endpoints

### GET /_matrix/federation/v1/state/{roomId}

Retrieves a snapshot of a room's state at a given event.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

#### Request Parameters

| Name | Type | Description |
| --- | --- | --- |
| `roomId` | `string` | **Required:** The room ID to get state for. |

#### Query Parameters

| Name | Type | Description |
| --- | --- | --- |
| `event_id` | `string` | **Required:** An event ID in the room to retrieve the state at. |

#### Responses

| Status | Description |
| --- | --- |
| `200` | The fully resolved state for the room, prior to considering any state changes induced by the requested event. Includes the authorization chain for the events. |

#### 200 Response

| Name | Type | Description |
| --- | --- | --- |
| `auth_chain` | `[PDU]` | **Required:** The full set of authorization events that make up the state of the room, and their authorization events, recursively. Note that events have a different format depending on the room version - check the [room version specification](https://spec.matrix.org/unstable/rooms/) for precise event formats. |
| `pdus` | `[PDU]` | **Required:** The fully resolved state of the room at the given event. Note that events have a different format depending on the room version - check the [room version specification](https://spec.matrix.org/unstable/rooms/) for precise event formats. |

#### Example

```json
{
  "auth_chain": [
    {
      "content": {
        "see_room_version_spec": "The event format changes depending on the room version."
      },
      "room_id": "!somewhere:example.org",
      "type": "m.room.minimal_pdu"
    }
  ],
  "pdus": [
    {
      "content": {
        "see_room_version_spec": "The event format changes depending on the room version."
      },
      "room_id": "!somewhere:example.org",
      "type": "m.room.minimal_pdu"
    }
  ]
}
```

## State Event Types

### Core State Events

#### m.room.create
- **Purpose**: Defines the room creation parameters and version
- **Required**: Yes (must be first event in room)
- **Authorization**: No authorization required (root of auth chain)

#### m.room.power_levels
- **Purpose**: Defines user power levels and required levels for actions
- **Default Power Levels**: 
  - Users: 0
  - State events: 50
  - Non-state events: 0
  - Invite: 0
  - Kick: 50
  - Ban: 50
  - Redact: 50
- **Authorization**: Requires appropriate power level to modify

#### m.room.join_rules
- **Purpose**: Defines how users can join the room
- **Values**: `public`, `invite`, `restricted`, `knock_restricted`, `knock`
- **Authorization**: Requires appropriate power level to modify

#### m.room.member
- **Purpose**: Defines user membership in the room
- **Values**: `join`, `leave`, `invite`, `ban`, `knock`
- **Authorization**: Complex rules based on membership transition and power levels

### Optional State Events

#### m.room.name
- **Purpose**: Human-readable room name
- **Authorization**: Requires appropriate power level

#### m.room.topic
- **Purpose**: Room topic/description
- **Authorization**: Requires appropriate power level

#### m.room.avatar
- **Purpose**: Room avatar image
- **Authorization**: Requires appropriate power level

#### m.room.canonical_alias
- **Purpose**: Primary room alias
- **Authorization**: Requires appropriate power level

#### m.room.guest_access
- **Purpose**: Whether guests can join
- **Values**: `can_join`, `forbidden`
- **Authorization**: Requires appropriate power level

#### m.room.history_visibility
- **Purpose**: Who can see room history
- **Values**: `world_readable`, `shared`, `invited`, `joined`
- **Authorization**: Requires appropriate power level

## State Resolution Algorithms

Different room versions use different state resolution algorithms:

### Version 1 (Deprecated)
- Simple power level ordering
- No protection against certain attack vectors
- Deprecated due to security issues

### Version 2+ (Current)
- Power level ordering with additional protections
- Proper handling of conflicted state
- Protection against state reset attacks
- Iterative conflict resolution

### State Resolution Process

1. **Separate State**: Identify conflicted vs unconflicted state
2. **Resolve Conflicts**: Apply authorization rules and power level ordering
3. **Merge State**: Combine resolved and unconflicted state
4. **Validation**: Ensure final state passes authorization

## Implementation Considerations

### State Storage
- Store current room state efficiently
- Maintain historical state for backfill
- Index by `(event_type, state_key)` tuples
- Cache resolved state to avoid recomputation

### State Queries
- Implement efficient state lookups
- Support historical state queries at specific events
- Provide authorization chain construction
- Handle missing events gracefully

### Federation Integration
- Validate incoming state events
- Handle state conflicts during federation
- Maintain state consistency across servers
- Support state synchronization during room joins

### Performance Optimization
- Batch state updates when possible
- Use incremental state resolution
- Cache authorization chains
- Minimize state resolution computations

### Security Considerations
- Validate all state changes against authorization rules
- Prevent state reset attacks
- Ensure proper power level enforcement
- Handle malicious state events safely