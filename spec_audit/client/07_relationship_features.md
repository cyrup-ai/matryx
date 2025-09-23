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

