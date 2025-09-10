---
title: "Matrix Client-Server API: Relationship Features"
description: "Event relationships and advanced UX including spaces, message editing, reactions, threading, and reference relations"
---

# Relationship Features

*This file contains content from approximately lines 28737-30280 of the original Matrix Client-Server API specification, covering event relationships and advanced user experience features.*

## Table of Contents

- [Spaces](#spaces)
- [Event Replacements](#event-replacements)
- [Event Annotations/Reactions](#event-annotationsreactions)
- [Threading](#threading)
- [Reference Relations](#reference-relations)

## Spaces

**\[Added in `v1.2`\]**

Often used to group rooms of similar subject matter (such as an "Official matrix.org rooms" space or a "Work stuff" space), spaces are a way to organise rooms while being represented as rooms themselves.

A space is defined by the [`m.space` room type](https://spec.matrix.org/unstable/client-server-api/#types), making it known as a "space-room". The space's name, topic, avatar, aliases, etc are all defined through the existing relevant state events within the space-room.

Sending normal [`m.room.message`](https://spec.matrix.org/unstable/client-server-api/#mroommessage) events within the space-room is discouraged - clients are not generally expected to have a way to render the timeline of the room. As such, space-rooms should be created with [`m.room.power_levels`](https://spec.matrix.org/unstable/client-server-api/#mroompower_levels) which prohibit normal events by setting `events_default` to a suitably high number. In the default power level structure, this would be `100`. Clients might wish to go a step further and explicitly ignore notification counts on space-rooms.

Membership of a space is defined and controlled by the existing mechanisms which govern a room: [`m.room.member`](https://spec.matrix.org/unstable/client-server-api/#mroommember), [`m.room.history_visibility`](https://spec.matrix.org/unstable/client-server-api/#mroomhistory_visibility), and [`m.room.join_rules`](https://spec.matrix.org/unstable/client-server-api/#mroomjoin_rules). Canonical aliases and invites, including third-party invites, still work just as they do in normal rooms as well. Furthermore, spaces can also be published in the [room directory](https://spec.matrix.org/unstable/client-server-api/#published-room-directory) to make them discoverable.

All other aspects of regular rooms are additionally carried over, such as the ability to set arbitrary state events, hold room account data, etc. Spaces are just rooms with extra functionality on top.

### Managing rooms/spaces included in a space

Spaces form a hierarchy of rooms which clients can use to structure their room list into a tree-like view. The parent/child relationship can be defined in two ways: with [`m.space.child`](https://spec.matrix.org/unstable/client-server-api/#mspacechild) state events in the space-room, or with [`m.space.parent`](https://spec.matrix.org/unstable/client-server-api/#mspaceparent) state events in the child room.

In most cases, both the child and parent relationship should be defined to aid discovery of the space and its rooms. When only a `m.space.child` is used, the space is effectively a curated list of rooms which the rooms themselves might not be aware of. When only a `m.space.parent` is used, the rooms are "secretly" added to spaces with the effect of not being advertised directly by the space.

#### m.space.child relationship

When using this approach, the state events get sent into the space-room which is the parent to the room. The `state_key` for the event is the child room's ID.

For example, to achieve the following:

```
#space:example.org
    #general:example.org (!abcdefg:example.org)
    !private:example.org
```

the state of `#space:example.org` would consist of:

*Unimportant fields trimmed for brevity.*

```json
{

    "type": "m.space.child",

    "state_key": "!abcdefg:example.org",

    "content": {

        "via": ["example.org"]

    }

}
```
```json
{

    "type": "m.space.child",

    "state_key": "!private:example.org",

    "content": {

        "via": ["example.org"]

    }

}
```

No state events in the child rooms themselves would be required (though they can also be present). This allows for users to define spaces without needing explicit permission from the room moderators/admins.

Child rooms can be removed from a space by omitting the `via` key of `content` on the relevant state event, such as through redaction or otherwise clearing the `content`.

## m.space.child

---

Defines the relationship of a child room to a space-room. Has no effect in rooms which are not [spaces](https://spec.matrix.org/unstable/client-server-api/#spaces).

| Event type: | State event |
| --- | --- |
| State key | The child room ID being described. |

## Content

| Name | Type | Description |
| --- | --- | --- |
| `order` | `string` | Optional string to define ordering among space children. These are lexicographically compared against other children's `order`, if present.  Must consist of ASCII characters within the range `\x20` (space) and `\x7E` (`~`), inclusive. Must not exceed 50 characters.  `order` values with the wrong type, or otherwise invalid contents, are to be treated as though the `order` key was not provided.  See [Ordering of children within a space](https://spec.matrix.org/unstable/client-server-api/#ordering-of-children-within-a-space) for information on how the ordering works. |
| `suggested` | `boolean` | Optional (default `false`) flag to denote whether the child is "suggested" or of interest to members of the space. This is primarily intended as a rendering hint for clients to display the room differently, such as eagerly rendering them in the room list. |
| `via` | `[string]` | **Required:** A list of servers to try and join through. See also: [Routing](https://spec.matrix.org/unstable/appendices/#routing).  When not present or invalid, the child room is not considered to be part of the space. |

## Examples

```json
{

  "content": {

    "order": "lexicographically_compare_me",

    "suggested": true,

    "via": [

      "example.org",

      "other.example.org"

    ]

  },

  "event_id": "$143273582443PhrSn:example.org",

  "origin_server_ts": 1432735824653,

  "room_id": "!jEsUZKDJdhlrceRyVU:example.org",

  "sender": "@example:example.org",

  "state_key": "!roomid:example.org",

  "type": "m.space.child",

  "unsigned": {

    "age": 1234,

    "membership": "join"

  }

}
```

##### Ordering of children within a space

When the client is displaying the children of a space, the children should be ordered using the algorithm below. In some cases, like a traditional left side room list, the client may override the ordering to provide better user experience. A theoretical space summary view would however show the children ordered.

Taking the set of space children, first order the children with a valid `order` key lexicographically by Unicode code-points such that `\x20` (space) is sorted before `\x7E` (`~`). Then, take the remaining children and order them by the `origin_server_ts` of their `m.space.child` event in ascending numeric order, placing them after the children with a valid `order` key in the resulting set.

In cases where the `order` values are the same, the children are ordered by their timestamps. If the timestamps are the same, the children are ordered lexicographically by their room IDs (state keys) in ascending order.

#### m.space.parent relationships

Rooms can additionally claim to be part of a space by populating their own state with a parent event. Similar to child events within spaces, the parent event's `state_key` is the room ID of the parent space, and they have a similar `via` list within their `content` to denote both whether or not the link is valid and which servers might be possible to join through.

To avoid situations where a room falsely claims it is part of a given space,`m.space.parent` events should be ignored unless one of the following is true:

- A corresponding `m.space.child` event can be found in the supposed parent space.
- The sender of the `m.space.parent` event has sufficient power level in the supposed parent space to send `m.space.child` state events (there doesn't need to be a matching child event).

`m.space.parent` events can additionally include a `canonical` boolean key in their `content` to denote that the parent space is the main/primary space for the room. This can be used to, for example, have the client find other rooms by peeking into that space and suggesting them to the user. Only one canonical parent should exist, though this is not enforced. To tiebreak, use the lowest room ID sorted lexicographically by Unicode code-points.

## m.space.parent

---

Defines the relationship of a room to a parent space-room.

| Event type: | State event |
| --- | --- |
| State key | The parent room ID. |

## Content

| Name | Type | Description |
| --- | --- | --- |
| `canonical` | `boolean` | Optional (default `false`) flag to denote this parent is the primary parent for the room.  When multiple `canonical` parents are found, the lowest parent when ordering by room ID lexicographically by Unicode code-points should be used. |
| `via` | `[string]` | **Required:** A list of servers to try and join through. See also: [Routing](https://spec.matrix.org/unstable/appendices/#routing).  When not present or invalid, the room is not considered to be part of the parent space. |

## Examples

```json
{

  "content": {

    "canonical": true,

    "via": [

      "example.org",

      "other.example.org"

    ]

  },

  "event_id": "$143273582443PhrSn:example.org",

  "origin_server_ts": 1432735824653,

  "room_id": "!jEsUZKDJdhlrceRyVU:example.org",

  "sender": "@example:example.org",

  "state_key": "!parent_roomid:example.org",

  "type": "m.space.parent",

  "unsigned": {

    "age": 1234,

    "membership": "join"

  }

}
```

### Discovering rooms within spaces

Often the client will want to assist the user in exploring what rooms/spaces are part of a space. This can be done with crawling [`m.space.child`](https://spec.matrix.org/unstable/client-server-api/#mspacechild) state events in the client and peeking into the rooms to get information like the room name, though this is impractical for most cases.

Instead, a hierarchy API is provided to walk the space tree and discover the rooms with their aesthetic details.

The [`GET /hierarchy`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv1roomsroomidhierarchy) API works in a depth-first manner: when it encounters another space as a child it recurses into that space before returning non-space children.

## GET /\_matrix/client/v1/rooms/{roomId}/hierarchy

---

**Added in `v1.2`**

Paginates over the space tree in a depth-first manner to locate child rooms of a given space.

Where a child room is unknown to the local server, federation is used to fill in the details. The servers listed in the `via` array should be contacted to attempt to fill in missing rooms.

Only [`m.space.child`](https://spec.matrix.org/unstable/client-server-api/#mspacechild) state events of the room are considered. Invalid child rooms and parent events are not covered by this endpoint.

| Rate-limited: | Yes |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `roomId` | `string` | **Required:** The room ID of the space to get a hierarchy for. |

| Name | Type | Description |
| --- | --- | --- |
| `from` | `string` | A pagination token from a previous result. If specified, `max_depth` and `suggested_only` cannot be changed from the first request. |
| `limit` | `integer` | Optional limit for the maximum number of rooms to include per response. Must be an integer greater than zero.  Servers should apply a default value, and impose a maximum value to avoid resource exhaustion. |
| `max_depth` | `integer` | Optional limit for how far to go into the space. Must be a non-negative integer.  When reached, no further child rooms will be returned.  Servers should apply a default value, and impose a maximum value to avoid resource exhaustion. |
| `suggested_only` | `boolean` | Optional (default `false`) flag to indicate whether or not the server should only consider suggested rooms. Suggested rooms are annotated in their [`m.space.child`](https://spec.matrix.org/unstable/client-server-api/#mspacechild) event contents. |

---

## Responses

| Status | Description |
| --- | --- |
| `200` | A portion of the space tree, starting at the provided room ID. |
| `400` | The request was invalid in some way. A meaningful `errcode` and description error text will be returned. Example reasons for rejection are:  - The `from` token is unknown to the server. - `suggested_only` or `max_depth` changed during pagination. |
| `403` | The user cannot view or peek on the room. A meaningful `errcode` and description error text will be returned. Example reasons for rejection are:  - The room is not set up for peeking. - The user has been banned from the room. - The room does not exist. |
| `429` | This request was rate-limited. |

### 200 response

```json
{

  "next_batch": "next_batch_token",

  "rooms": [

    {

      "avatar_url": "mxc://example.org/abcdef",

      "canonical_alias": "#general:example.org",

      "children_state": [

        {

          "content": {

            "via": [

              "example.org"

            ]

          },

          "origin_server_ts": 1629413349153,

          "sender": "@alice:example.org",

          "state_key": "!a:example.org",

          "type": "m.space.child"

        }

      ],

      "guest_can_join": false,

      "join_rule": "public",

      "name": "The First Space",

      "num_joined_members": 42,

      "room_id": "!space:example.org",

      "room_type": "m.space",

      "topic": "No other spaces were created first, ever",

      "world_readable": true

    }

  ]

}
```

## Event Replacements

**\[Added in `v1.4`\]**

Event replacements, or "message edit events", are events that use an [event relationship](https://spec.matrix.org/unstable/client-server-api/#forming-relationships-between-events) with a `rel_type` of `m.replace`, which indicates that the original event is intended to be replaced.

An example of a message edit event might look like this:

```json
{

    "type": "m.room.message",

    "content": {

        "body": "* Hello! My name is bar",

        "msgtype": "m.text",

        "m.new_content": {

            "body": "Hello! My name is bar",

            "msgtype": "m.text"

        },

        "m.relates_to": {

            "rel_type": "m.replace",

            "event_id": "$some_event_id"

        }

    },

    // ... other fields required by events

}
```

The `content` of the replacement must contain a `m.new_content` property which defines the replacement `content`. The normal `content` properties (`body`,`msgtype` etc.) provide a fallback for clients which do not understand replacement events.

`m.new_content` can include any properties that would normally be found in an event's content property, such as `formatted_body` (see [`m.room.message` `msgtypes`](https://spec.matrix.org/unstable/client-server-api/#mroommessage-msgtypes)).

### Validity of replacement events

There are a number of requirements on replacement events, which must be satisfied for the replacement to be considered valid:

- As with all event relationships, the original event and replacement event must have the same `room_id` (i.e. you cannot send an event in one room and then an edited version in a different room).
- The original event and replacement event must have the same `sender` (i.e. you cannot edit someone else's messages).
- The replacement and original events must have the same `type` (i.e. you cannot change the original event's type).
- The replacement and original events must not have a `state_key` property (i.e. you cannot edit state events at all).
- The original event must not, itself, have a `rel_type` of `m.replace` (i.e. you cannot edit an edit — though you can send multiple edits for a single original event).
- The replacement event (once decrypted, if appropriate) must have an `m.new_content` property.

If any of these criteria are not satisfied, implementations should ignore the replacement event (the content of the original should not be replaced, and the edit should not be included in the server-side aggregation).

Note that the [`msgtype`](https://spec.matrix.org/unstable/client-server-api/#mroommessage-msgtypes) property of replacement `m.room.message` events does *not* need to be the same as in the original event. For example, it is legitimate to replace an `m.text` event with an `m.emote`.

### Editing encrypted events

If the original event was [encrypted](https://spec.matrix.org/unstable/client-server-api/#end-to-end-encryption), the replacement should be too. In that case, `m.new_content` is placed in the content of the encrypted payload. As with all event relationships, the `m.relates_to` property must be sent in the unencrypted (cleartext) part of the event.

For example, a replacement for an encrypted event might look like this:

```json
{

    "type": "m.room.encrypted",

    "content": {

        "m.relates_to": {

            "rel_type": "m.replace",

            "event_id": "$some_event_id"

        },

        "algorithm": "m.megolm.v1.aes-sha2",

        // ... other properties required by m.room.encrypted events

    }

}
```