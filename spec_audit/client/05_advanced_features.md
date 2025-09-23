---
title: "Matrix Client-Server API: Advanced Features"
description: "Advanced room and user features including history visibility, push notifications, search, guest access, and room organization"
---

# Advanced Features

*This file contains content from approximately lines 21310-25586 of the original Matrix Client-Server API specification, covering advanced room and user features.*

## Table of Contents

- [Room History Visibility](#room-history-visibility)
- [Push Notifications](#push-notifications)
- [Third-party Invites](#third-party-invites)
- [Server Side Search](#server-side-search)
- [Guest Access](#guest-access)
- [Room Previews](#room-previews)
- [Room Tagging](#room-tagging)

## Room History Visibility

This module adds support for controlling the visibility of previous events in a room.

In all cases except `world_readable`, a user needs to join a room to view events in that room. Once they have joined a room, they will gain access to a subset of events in the room. How this subset is chosen is controlled by the `m.room.history_visibility` event outlined below. After a user has left a room, they may see any events which they were allowed to see before they left the room, but no events received after they left.

The four options for the `m.room.history_visibility` event are:

- `world_readable` - All events while this is the `m.room.history_visibility` value may be shared by any participating homeserver with anyone, regardless of whether they have ever joined the room.
- `shared` - Previous events are always accessible to newly joined members. All events in the room are accessible, even those sent when the member was not a part of the room.
- `invited` - Events are accessible to newly joined members from the point they were invited onwards. Events stop being accessible when the member's state changes to something other than `invite` or `join`.
- `joined` - Events are accessible to newly joined members from the point they joined the room onwards. Events stop being accessible when the member's state changes to something other than `join`.

#### Events

## m.room.history\_visibility

---

This event controls whether a user can see the events that happened in a room from before they joined.

| Event type: | State event |
| --- | --- |
| State key | A zero-length string. |

## Content

| Name | Type | Description |
| --- | --- | --- |
| `history_visibility` | `string` | **Required:** Who can see the room history.  One of: `[invited, joined, shared, world_readable]`. |

## Examples

#### Client behaviour

Clients may want to display a notice that events may be read by non-joined people if the history visibility is set to `world_readable`.

#### Server behaviour

By default if no `history_visibility` is set, or if the value is not understood, the visibility is assumed to be `shared`. The rules governing whether a user is allowed to see an event depend on the state of the room *at that event*.

1. If the `history_visibility` was set to `world_readable`, allow.
2. If the user's `membership` was `join`, allow.
3. If `history_visibility` was set to `shared`, and the user joined the room at any point after the event was sent, allow.
4. If the user's `membership` was `invite`, and the `history_visibility` was set to `invited`, allow.
5. Otherwise, deny.

For `m.room.history_visibility` events themselves, the user should be allowed to see the event if the `history_visibility` before *or* after the event would allow them to see it. (For example, a user should be able to see `m.room.history_visibility` events which change the `history_visibility` from `world_readable` to `joined` *or* from `joined` to `world_readable`, even if that user was not a member of the room.)

Likewise, for the user's own `m.room.member` events, the user should be allowed to see the event if their `membership` before *or* after the event would allow them to see it. (For example, a user can always see `m.room.member` events which set their membership to `join`, or which change their membership from `join` to any other value, even if `history_visibility` is `joined`.)

#### Security considerations

The default value for `history_visibility` is `shared` for backwards-compatibility reasons. Clients need to be aware that by not setting this event they are exposing all of their room history to anyone in the room.

## Push Notifications

```
+--------------------+  +-------------------+
                  Matrix HTTP      |                    |  |                   |
             Notification Protocol |   App Developer    |  |   Device Vendor   |
                                   |                    |  |                   |
           +-------------------+   | +----------------+ |  | +---------------+ |
           |                   |   | |                | |  | |               | |
           | Matrix homeserver +----->  Push Gateway  +------> Push Provider | |
           |                   |   | |                | |  | |               | |
           +-^-----------------+   | +----------------+ |  | +----+----------+ |
             |                     |                    |  |      |            |
    Matrix   |                     |                    |  |      |            |
 Client/Server API  +              |                    |  |      |            |
             |      |              +--------------------+  +-------------------+
             |   +--+-+                                           |
             |   |    <-------------------------------------------+
             +---+    |
                 |    |          Provider Push Protocol
                 +----+
          Mobile Device or Client
```

This module adds support for push notifications. Homeservers send notifications of events to user-configured HTTP endpoints. Users may also configure a number of rules that determine which events generate notifications. These are all stored and managed by the user's homeserver. This allows user-specific push settings to be reused between client applications.

The above diagram shows the flow of push notifications being sent to a handset where push notifications are submitted via the handset vendor, such as Apple's APNS or Google's GCM. This happens as follows:

1. The client app signs in to a homeserver.
2. The client app registers with its vendor's Push Provider and obtains a routing token of some kind.
3. The mobile app uses the Client/Server API to add a 'pusher', providing the URL of a specific Push Gateway which is configured for that application. It also provides the routing token it has acquired from the Push Provider.
4. The homeserver starts sending HTTP requests to the Push Gateway using the supplied URL. The Push Gateway relays this notification to the Push Provider, passing the routing token along with any necessary private credentials the provider requires to send push notifications.
5. The Push Provider sends the notification to the device.

Definitions for terms used in this section are below:

Push Provider

A push provider is a service managed by the device vendor which can send notifications directly to the device. Google Cloud Messaging (GCM) and Apple Push Notification Service (APNS) are two examples of push providers.

Push Gateway

A push gateway is a server that receives HTTP event notifications from homeservers and passes them on to a different protocol such as APNS for iOS devices or GCM for Android devices. Clients inform the homeserver which Push Gateway to send notifications to when it sets up a Pusher.

Pusher

A pusher is a worker on the homeserver that manages the sending of HTTP notifications for a user. A user can have multiple pushers: one per device.

Push Rule

A push rule is a single rule that states under what *conditions* an event should be passed onto a push gateway and *how* the notification should be presented. These rules are stored on the user's homeserver. They are manually configured by the user, who can create and view them via the Client/Server API.

Push Ruleset

A push ruleset *scopes a set of rules according to some criteria*. For example, some rules may only be applied for messages from a particular sender, a particular room, or by default. The push ruleset contains the entire set of scopes and rules.

### Push Rules

A push rule is a single rule that states under what *conditions* an event should be passed onto a push gateway and *how* the notification should be presented. There are different "kinds" of push rules and each rule has an associated priority. Every push rule MUST have a `kind` and `rule_id`. The `rule_id` is a unique string within the kind of rule and its' scope: `rule_ids` do not need to be unique between rules of the same kind on different devices. Rules may have extra keys depending on the value of `kind`.

The different `kind` s of rule, in the order that they are checked, are:

1. **Override rules (`override`).**The highest priority rules are user-configured overrides.
2. **Content-specific rules (`content`).**These configure behaviour for messages that match certain patterns. Content rules take one parameter: `pattern`, that gives the [glob-style pattern](https://spec.matrix.org/unstable/appendices/#glob-style-matching) to match against. The match is performed case-insensitively, and must match any substring of the `content.body` property which starts and ends at a word boundary. A word boundary is defined as the start or end of the value, or any character not in the sets `[A-Z]`, `[a-z]`, `[0-9]` or `_`.The exact meaning of "case insensitive" is defined by the implementation of the homeserver.
3. **Room-specific rules (`room`).**These rules change the behaviour of all messages for a given room. The `rule_id` of a room rule is always the ID of the room that it affects.
4. **Sender-specific rules (`sender`).**These rules configure notification behaviour for messages from a specific Matrix user ID. The `rule_id` of Sender rules is always the Matrix user ID of the user whose messages they'd apply to.
5. **Underride rules (`underride`).**These are identical to `override` rules, but have a lower priority than `content`, `room` and `sender` rules.

Rules with the same `kind` can specify an ordering priority. This determines which rule is selected in the event of multiple matches. For example, a rule matching "tea" and a separate rule matching "time" would both match the sentence "It's time for tea". The ordering of the rules would then resolve the tiebreak to determine which rule is executed. Only `actions` for highest priority rule will be sent to the Push Gateway.

Each rule can be enabled or disabled. Disabled rules never match. If no rules match an event, the homeserver MUST NOT notify the Push Gateway for that event. Homeservers MUST NOT notify the Push Gateway for events that the user has sent themselves.

#### Actions

All rules have an associated list of `actions`. An action affects if and how a notification is delivered for a matching event. The following actions are defined:

`notify`

This causes each matching event to generate a notification.

`set_tweak`

Sets an entry in the `tweaks` dictionary key that is sent in the notification request to the Push Gateway. This takes the form of a dictionary with a `set_tweak` key whose value is the name of the tweak to set. It may also have a `value` key which is the value to which it should be set.

The following tweaks are defined:

`sound`

A string representing the sound to be played when this notification arrives. A value of `default` means to play a default sound. A device may choose to alert the user by some other means if appropriate, eg. vibration.

`highlight`

A boolean representing whether or not this message should be highlighted in the UI. This will normally take the form of presenting the message in a different colour and/or style. The UI might also be adjusted to draw particular attention to the room in which the event occurred. If a `highlight` tweak is given with no value, its value is defined to be `true`. If no highlight tweak is given at all then the value of `highlight` is defined to be false.

Tweaks are passed transparently through the homeserver so client applications and Push Gateways may agree on additional tweaks. For example, a tweak may be added to specify how to flash the notification light on a mobile device.

Actions that have no parameters are represented as a string. Otherwise, they are represented as a dictionary with a key equal to their name and other keys as their parameters, e.g.`{ "set_tweak": "sound", "value": "default" }`.

##### Historical Actions

Older versions of the Matrix specification included the `dont_notify` and `coalesce` actions. Clients and homeservers MUST ignore these actions, for instance, by stripping them from actions arrays they encounter. This means, for example, that a rule with `["dont_notify"]` actions MUST be equivalent to a rule with an empty actions array.

#### Conditions

`override` and `underride` rules MAY have a list of 'conditions'. All conditions must hold true for an event in order for the rule to match. A rule with no conditions always matches.

Unrecognised conditions MUST NOT match any events, effectively making the push rule disabled.

`room`, `sender` and `content` rules do not have conditions in the same way, but instead have predefined conditions. In the cases of `room` and `sender` rules, the `rule_id` of the rule determines its behaviour.

The following conditions are defined:

**`event_match`**

This is a glob pattern match on a property of the event. Parameters:

- `key`: The [dot-separated path of the property](https://spec.matrix.org/unstable/appendices/#dot-separated-property-paths) of the event to match, e.g. `content.body`.
- `pattern`: The [glob-style pattern](https://spec.matrix.org/unstable/appendices/#glob-style-matching) to match against.

The match is performed case-insensitively, and must match the entire value of the event property given by `key` (though see below regarding `content.body`). The exact meaning of "case insensitive" is defined by the implementation of the homeserver.

If the property specified by `key` is completely absent from the event, or does not have a string value, then the condition will not match, even if `pattern` is `*`.

As a special case, if `key` is `content.body`, then `pattern` must instead match any substring of the value of the property which starts and ends at a word boundary. A word boundary is defined as the start or end of the value, or any character not in the sets `[A-Z]`, `[a-z]`, `[0-9]` or `_`.

**`event_property_is`**

This is an exact value match on a property of the event. Parameters:

- `key`: The [dot-separated path of the property](https://spec.matrix.org/unstable/appendices/#dot-separated-property-paths) of the event to match, e.g. `content.body`.
- `value`: The value to match against.

The match is performed exactly and only supports non-compound [canonical JSON](https://spec.matrix.org/unstable/appendices/#canonical-json) values: strings, integers in the range of `[-(2**53)+1, (2**53)-1]`, booleans, and `null`.

If the property specified by `key` is completely absent from the event, or does not have a string, integer, boolean, or `null` value, then the condition will not match.

**`event_property_contains`**

This matches if an array property of an event exactly contains a value. Parameters:

- `key`: The [dot-separated path of the property](https://spec.matrix.org/unstable/appendices/#dot-separated-property-paths) of the event to match, e.g. `content.body`.
- `value`: The value to match against.

The array values are matched exactly and only supports non-compound [canonical JSON](https://spec.matrix.org/unstable/appendices/#canonical-json) values: strings, integers in the range of `[-(2**53)+1, (2**53)-1]`, booleans, and `null`. Array values not of those types are ignored.

If the property specified by `key` is completely absent from the event, or is not an array, then the condition will not match.

**`contains_display_name`**

This matches messages where `content.body` contains the owner's display name in that room. This is a separate condition because display names may change and as such it would be hard to maintain a rule that matched the user's display name. This condition has no parameters.

**`room_member_count`**

This matches the current number of members in the room. Parameters:

- `is`: A decimal integer optionally prefixed by one of, `==`, `<`,`>`, `>=` or `<=`. A prefix of `<` matches rooms where the member count is strictly less than the given number and so forth. If no prefix is present, this parameter defaults to `==`.

**`sender_notification_permission`**

This takes into account the current power levels in the room, ensuring the sender of the event has high enough power to trigger the notification.

Parameters:

- `key`: A string that determines the power level the sender must have to trigger notifications of a given type, such as `room`. Refer to the [m.room.power\_levels](https://spec.matrix.org/unstable/client-server-api/#mroompower_levels) event schema for information about what the defaults are and how to interpret the event. The `key` is used to look up the power level required to send a notification type from the `notifications` object in the power level event content.

### Predefined Rules

Homeservers can specify "server-default rules". They operate at a lower priority than "user-defined rules", except for the `.m.rule.master` rule which has always a higher priority than any other rule. The `rule_id` for all server-default rules MUST start with a dot (".") to identify them as "server-default". The following server-default rules are specified:

#### Default Override Rules

**`.m.rule.master`**

Matches all events. This can be enabled to turn off all push notifications. Unlike other server-default rules, this one has always a higher priority than other rules, even user defined ones. By default this rule is disabled.

Definition:

```json
{

    "rule_id": ".m.rule.master",

    "default": true,

    "enabled": false,

    "conditions": [],

    "actions": []

}
```

**`.m.rule.suppress_notices`**

Matches messages with a `msgtype` of `notice`.

Definition:

```json
{

    "rule_id": ".m.rule.suppress_notices",

    "default": true,

    "enabled": true,

    "conditions": [

        {

            "kind": "event_match",

            "key": "content.msgtype",

            "pattern": "m.notice"

        }

    ],

    "actions": []

}
```

**`.m.rule.invite_for_me`**

Matches any invites to a new room for this user.

Definition:

```json
{

    "rule_id": ".m.rule.invite_for_me",

    "default": true,

    "enabled": true,

    "conditions": [

        {

            "key": "type",

            "kind": "event_match",

            "pattern": "m.room.member"

        },

        {

            "key": "content.membership",

            "kind": "event_match",

            "pattern": "invite"

        },

        {

            "key": "state_key",

            "kind": "event_match",

            "pattern": "[the user's Matrix ID]"

        }

    ],

    "actions": [

       "notify",

        {

            "set_tweak": "sound",

            "value": "default"

        }

    ]

}
```

**`.m.rule.member_event`**

Matches any `m.room.member_event`.

Definition:

```json
{

    "rule_id": ".m.rule.member_event",

    "default": true,

    "enabled": true,

    "conditions": [

        {

            "key": "type",

            "kind": "event_match",

            "pattern": "m.room.member"

        }

    ],

    "actions": []

}
```

**`.m.rule.is_user_mention`**

**\[Added in `v1.7`\]**

Matches any message which contains the user's Matrix ID in the list of `user_ids` under the `m.mentions` property.

Definition:

```json
{

    "rule_id": ".m.rule.is_user_mention",

    "default": true,

    "enabled": true,

    "conditions": [

        {

            "kind": "event_property_contains",

            "key": "content.m\\.mentions.user_ids",

            "value": "[the user's Matrix ID]"

        }

    ],

    "actions": [

        "notify",

        {

            "set_tweak": "sound",

            "value": "default"

        },

        {

            "set_tweak": "highlight"

        }

    ]

}
```

**`.m.rule.contains_display_name`**

**\[Changed in `v1.7`\]**

As of `v1.7`, this rule is deprecated and **should only be enabled if the event does not have an [`m.mentions` property](https://spec.matrix.org/unstable/client-server-api/#definition-mmentions)**.

Matches any message whose content contains the user's current display name in the room in which it was sent.

Definition:

```json
{

    "rule_id": ".m.rule.contains_display_name",

    "default": true,

    "enabled": true,

    "conditions": [

        {

            "kind": "contains_display_name"

        }

    ],

    "actions": [

        "notify",

        {

            "set_tweak": "sound",

            "value": "default"

        },

        {

            "set_tweak": "highlight"

        }

    ]

}
```

**`.m.rule.is_room_mention`**

**\[Added in `v1.7`\]**

Matches any message from a sender with the proper power level with the `room` property of the `m.mentions` property set to `true`.

Definition:

```json
{

    "rule_id": ".m.rule.is_room_mention",

    "default": true,

    "enabled": true,

    "conditions": [

        {

            "kind": "event_property_is",

            "key": "content.m\\.mentions.room",

            "value": true

        },

        {

            "kind": "sender_notification_permission",

            "key": "room"

        }

    ],

    "actions": [

        "notify",

        {

            "set_tweak": "highlight"

        }

    ]

}
```

**`.m.rule.roomnotif`**

**\[Changed in `v1.7`\]**

As of `v1.7`, this rule is deprecated and **should only be enabled if the event does not have an [`m.mentions` property](https://spec.matrix.org/unstable/client-server-api/#definition-mmentions)**.

Matches any message from a sender with the proper power level whose content contains the text `@room`, signifying the whole room should be notified of the event.

Definition:

```json
{

    "rule_id": ".m.rule.roomnotif",

    "default": true,

    "enabled": true,

    "conditions": [

        {

            "kind": "event_match",

            "key": "content.body",

            "pattern": "@room"

        },

        {

            "kind": "sender_notification_permission",

            "key": "room"

        }

    ],

    "actions": [

        "notify",

        {

            "set_tweak": "highlight"

        }

    ]

}
```

**`.m.rule.tombstone`**

Matches any state event whose type is `m.room.tombstone`. This is intended to notify users of a room when it is upgraded, similar to what an `@room` notification would accomplish.
Definition:

```json
{

    "rule_id": ".m.rule.tombstone",

    "default": true,

    "enabled": true,

    "conditions": [

        {

            "kind": "event_match",

            "key": "type",

            "pattern": "m.room.tombstone"

        },

        {

            "kind": "event_match",

            "key": "state_key",

            "pattern": ""

        }

    ],

    "actions": [

        "notify",

        {

            "set_tweak": "highlight"

        }

    ]

}
```

**`.m.rule.reaction`**

**\[Added in `v1.7`\]**

Matches any event whose type is `m.reaction`. This suppresses notifications for [`m.reaction`](https://spec.matrix.org/unstable/client-server-api/#mreaction) events.

Definition:

```json
{

    "rule_id": ".m.rule.reaction",

    "default": true,

    "enabled": true,

    "conditions": [

        {

            "kind": "event_match",

            "key": "type",

            "pattern": "m.reaction"

        }

    ],

    "actions": []

}
```

**`.m.rule.room.server_acl`**

**\[Added in `v1.4`\]**

Suppresses notifications for [`m.room.server_acl`](https://spec.matrix.org/unstable/client-server-api/#mroomserver_acl) events.

Definition:

```json
{

    "rule_id": ".m.rule.room.server_acl",

    "default": true,

    "enabled": true,

    "conditions": [

        {

            "kind": "event_match",

            "key": "type",

            "pattern": "m.room.server_acl"

        },

        {

            "kind": "event_match",

            "key": "state_key",

            "pattern": ""

        }

    ],

    "actions": []

}
```

**`.m.rule.suppress_edits`**

**\[Added in `v1.9`\]**

Suppresses notifications related to [event replacements](https://spec.matrix.org/unstable/client-server-api/#event-replacements).

Definition:

```json
{

    "rule_id": ".m.rule.suppress_edits",

    "default": true,

    "enabled": true,

    "conditions": [

        {

            "kind": "event_property_is",

            "key": "content.m\\.relates_to.rel_type",

            "value": "m.replace"

        }

    ],

    "actions": []

}
```

#### Default Content Rules

**`.m.rule.contains_user_name`**

**\[Changed in `v1.7`\]**

As of `v1.7`, this rule is deprecated and **should only be enabled if the event does not have an [`m.mentions` property](https://spec.matrix.org/unstable/client-server-api/#definition-mmentions)**.

Matches any message whose content contains the local part of the user's Matrix ID, separated by word boundaries.

Definition (as a `content` rule):

```json
{

    "rule_id": ".m.rule.contains_user_name",

    "default": true,

    "enabled": true,

    "pattern": "[the local part of the user's Matrix ID]",

    "actions": [

        "notify",

        {

            "set_tweak": "sound",

            "value": "default"

        },

        {

            "set_tweak": "highlight"

        }

    ]

}
```

#### Default Underride Rules

**`.m.rule.call`**

Matches any incoming VOIP call.

Definition:

```json
{

    "rule_id": ".m.rule.call",

    "default": true,

    "enabled": true,

    "conditions": [

        {

            "key": "type",

            "kind": "event_match",

            "pattern": "m.call.invite"

        }

    ],

    "actions": [

        "notify",

        {

            "set_tweak": "sound",

            "value": "ring"

        }

    ]

}
```

**`.m.rule.encrypted_room_one_to_one`**

Matches any encrypted event sent in a room with exactly two members. Unlike other push rules, this rule cannot be matched against the content of the event by nature of it being encrypted. This causes the rule to be an "all or nothing" match where it either matches *all* events that are encrypted (in 1:1 rooms) or none.

Definition:

```json
{

    "rule_id": ".m.rule.encrypted_room_one_to_one",

    "default": true,

    "enabled": true,

    "conditions": [

        {

            "kind": "room_member_count",

            "is": "2"

        },

        {

            "kind": "event_match",

            "key": "type",

            "pattern": "m.room.encrypted"

        }

    ],

    "actions": [

        "notify",

        {

            "set_tweak": "sound",

            "value": "default"

        }

    ]

}
```

**`.m.rule.room_one_to_one`**

Matches any message sent in a room with exactly two members.

Definition:

```json
{

    "rule_id": ".m.rule.room_one_to_one",

    "default": true,

    "enabled": true,

    "conditions": [

        {

            "kind": "room_member_count",

            "is": "2"

        },

        {

            "kind": "event_match",

            "key": "type",

            "pattern": "m.room.message"

        }

    ],

    "actions": [

        "notify",

        {

            "set_tweak": "sound",

            "value": "default"

        }

    ]

}
```

**`.m.rule.message`**

Matches all chat messages.

Definition:

```json
{

     "rule_id": ".m.rule.message",

     "default": true,

     "enabled": true,

     "conditions": [

         {

             "kind": "event_match",

             "key": "type",

             "pattern": "m.room.message"

         }

     ],

     "actions": [

         "notify"

     ]

}
```

**`.m.rule.encrypted`**

Matches all encrypted events. Unlike other push rules, this rule cannot be matched against the content of the event by nature of it being encrypted. This causes the rule to be an "all or nothing" match where it either matches *all* events that are encrypted (in group rooms) or none.

Definition:

```json
{

     "rule_id": ".m.rule.encrypted",

     "default": true,

     "enabled": true,

     "conditions": [

         {

             "kind": "event_match",

             "key": "type",

             "pattern": "m.room.encrypted"

         }

     ],

     "actions": [

         "notify"

     ]

}
```

### Push Rules: API

Clients can retrieve, add, modify and remove push rules globally or per-device using the APIs below.

