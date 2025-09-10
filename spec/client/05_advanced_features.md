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

## GET /\_matrix/client/v3/pushrules/

---

Retrieve all push rulesets for this user. Currently the only push ruleset defined is `global`.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

No request parameters or request body.

---

## Responses

| Status | Description |
| --- | --- |
| `200` | All the push rulesets for this user. |

### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `global` | `[Ruleset](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3pushrules_response-200_ruleset)` | **Required:** The global ruleset. |

| Name | Type | Description |
| --- | --- | --- |
| `content` | `[[PushRule](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3pushrules_response-200_pushrule)]` |  |
| `override` | `[[PushRule](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3pushrules_response-200_pushrule)]` |  |
| `room` | `[[PushRule](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3pushrules_response-200_pushrule)]` |  |
| `sender` | `[[PushRule](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3pushrules_response-200_pushrule)]` |  |
| `underride` | `[[PushRule](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3pushrules_response-200_pushrule)]` |  |

| Name | Type | Description |
| --- | --- | --- |
| `actions` | `[string\|object]` | **Required:** The actions to perform when this rule is matched. |
| `conditions` | `[[PushCondition](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3pushrules_response-200_pushcondition)]` | The conditions that must hold true for an event in order for a rule to be applied to an event. A rule with no conditions always matches. Only applicable to `underride` and `override` rules. |
| `default` | `boolean` | **Required:** Whether this is a default rule, or has been set explicitly. |
| `enabled` | `boolean` | **Required:** Whether the push rule is enabled or not. |
| `pattern` | `string` | The [glob-style pattern](https://spec.matrix.org/unstable/appendices/#glob-style-matching) to match against. Only applicable to `content` rules. |
| `rule_id` | `string` | **Required:** The ID of this rule. |

| Name | Type | Description |
| --- | --- | --- |
| `is` | `string` | Required for `room_member_count` conditions. A decimal integer optionally prefixed by one of, ==, <, >, >= or <=. A prefix of < matches rooms where the member count is strictly less than the given number and so forth. If no prefix is present, this parameter defaults to ==. |
| `key` | `string` | Required for `event_match`, `event_property_is` and `event_property_contains` conditions. The dot-separated field of the event to match.  Required for `sender_notification_permission` conditions. The field in the power level event the user needs a minimum power level for. Fields must be specified under the `notifications` property in the power level event's `content`. |
| `kind` | `string` | **Required:** The kind of condition to apply. See [conditions](https://spec.matrix.org/unstable/client-server-api/#conditions-1) for more information on the allowed kinds and how they work. |
| `pattern` | `string` | Required for `event_match` conditions. The [glob-style pattern](https://spec.matrix.org/unstable/appendices/#glob-style-matching) to match against. |
| `value` | `string\|integer\|boolean\|null` | Required for `event_property_is` and `event_property_contains` conditions. A non-compound [canonical JSON](https://spec.matrix.org/unstable/appendices/#canonical-json) value to match against. |

```json
{

  "global": {

    "content": [

      {

        "actions": [

          "notify",

          {

            "set_tweak": "sound",

            "value": "default"

          },

          {

            "set_tweak": "highlight"

          }

        ],

        "default": true,

        "enabled": true,

        "pattern": "alice",

        "rule_id": ".m.rule.contains_user_name"

      }

    ],

    "override": [

      {

        "actions": [],

        "conditions": [],

        "default": true,

        "enabled": false,

        "rule_id": ".m.rule.master"

      },

      {

        "actions": [],

        "conditions": [

          {

            "key": "content.msgtype",

            "kind": "event_match",

            "pattern": "m.notice"

          }

        ],

        "default": true,

        "enabled": true,

        "rule_id": ".m.rule.suppress_notices"

      }

    ],

    "room": [],

    "sender": [],

    "underride": [

      {

        "actions": [          "notify",

          {

            "set_tweak": "sound",

            "value": "ring"

          },

          {

            "set_tweak": "highlight",

            "value": false

          }

        ],

        "conditions": [

          {

            "key": "type",

            "kind": "event_match",

            "pattern": "m.call.invite"

          }

        ],

        "default": true,

        "enabled": true,

        "rule_id": ".m.rule.call"

      },

      {

        "actions": [

          "notify",

          {

            "set_tweak": "sound",

            "value": "default"

          },

          {

            "set_tweak": "highlight"

          }

        ],

        "conditions": [

          {

            "kind": "contains_display_name"

          }

        ],

        "default": true,

        "enabled": true,

        "rule_id": ".m.rule.contains_display_name"

      },

      {

        "actions": [

          "notify",

          {

            "set_tweak": "sound",

            "value": "default"

          },

          {

            "set_tweak": "highlight",

            "value": false

          }

        ],

        "conditions": [

          {

            "is": "2",

            "kind": "room_member_count"

          },

          {

            "key": "type",

            "kind": "event_match",

            "pattern": "m.room.message"

          }

        ],

        "default": true,

        "enabled": true,

        "rule_id": ".m.rule.room_one_to_one"

      },

      {

        "actions": [

          "notify",

          {

            "set_tweak": "sound",

            "value": "default"

          },

          {

            "set_tweak": "highlight",

            "value": false

          }

        ],

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

            "pattern": "@alice:example.com"

          }

        ],

        "default": true,

        "enabled": true,

        "rule_id": ".m.rule.invite_for_me"

      },

      {

        "actions": [

          "notify",

          {

            "set_tweak": "highlight",

            "value": false

          }

        ],

        "conditions": [

          {

            "key": "type",

            "kind": "event_match",

            "pattern": "m.room.member"

          }

        ],

        "default": true,

        "enabled": true,

        "rule_id": ".m.rule.member_event"

      },

      {

        "actions": [

          "notify",

          {

            "set_tweak": "highlight",

            "value": false

          }

        ],

        "conditions": [

          {

            "key": "type",

            "kind": "event_match",

            "pattern": "m.room.message"

          }

        ],

        "default": true,

        "enabled": true,

        "rule_id": ".m.rule.message"

      }

    ]

  }

}
```

## GET /\_matrix/client/v3/pushrules/global/

---

Retrieve all push rules for this user.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

No request parameters or request body.

---

## Responses

| Status | Description |
| --- | --- |
| `200` | All the push rules for this user. |

### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `content` | `[[PushRule](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3pushrulesglobal_response-200_pushrule)]` |  |
| `override` | `[[PushRule](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3pushrulesglobal_response-200_pushrule)]` |  |
| `room` | `[[PushRule](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3pushrulesglobal_response-200_pushrule)]` |  |
| `sender` | `[[PushRule](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3pushrulesglobal_response-200_pushrule)]` |  |
| `underride` | `[[PushRule](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3pushrulesglobal_response-200_pushrule)]` |  |

## GET /\_matrix/client/v3/pushrules/global/{kind}/{ruleId}

---

Retrieve a single specified push rule.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `kind` | `string` | **Required:** The kind of rule  One of: `[override, underride, sender, room, content]`. |
| `ruleId` | `string` | **Required:** The identifier for the rule. |

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The specific push rule. This will also include keys specific to the rule itself such as the rule's `actions` and `conditions` if set. |
| `404` | The push rule does not exist. |

### 200 response

```json
{

  "actions": [],

  "default": false,

  "enabled": true,

  "pattern": "cake*lie",

  "rule_id": "nocake"

}
```

### 404 response

```json
{

  "errcode": "M_NOT_FOUND",

  "error": "The push rule was not found."

}
```

## PUT /\_matrix/client/v3/pushrules/global/{kind}/{ruleId}

---

This endpoint allows the creation and modification of user defined push rules.

If a rule with the same `rule_id` already exists among rules of the same kind, it is updated with the new parameters, otherwise a new rule is created.

If both `after` and `before` are provided, the new or updated rule must be the next most important rule with respect to the rule identified by `before`.

If neither `after` nor `before` are provided and the rule is created, it should be added as the most important user defined rule among rules of the same kind.

When creating push rules, they MUST be enabled by default.

| Rate-limited: | Yes |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `kind` | `string` | **Required:** The kind of rule  One of: `[override, underride, sender, room, content]`. |
| `ruleId` | `string` | **Required:** The identifier for the rule. If the string starts with a dot ("."), the request MUST be rejected as this is reserved for server-default rules. Slashes ("/") and backslashes ("\\") are also not allowed. |

| Name | Type | Description |
| --- | --- | --- |
| `after` | `string` | This makes the new rule the next-less important rule relative to the given user defined rule. It is not possible to add a rule relative to a predefined server rule. |
| `before` | `string` | Use 'before' with a `rule_id` as its value to make the new rule the next-most important rule with respect to the given user defined rule. It is not possible to add a rule relative to a predefined server rule. |

### Request body

| Name | Type | Description |
| --- | --- | --- |
| `actions` | `[string\|object]` | **Required:** The action(s) to perform when the conditions for this rule are met. |
| `conditions` | `[[PushCondition](https://spec.matrix.org/unstable/client-server-api/#put_matrixclientv3pushrulesglobalkindruleid_request_pushcondition)]` | The conditions that must hold true for an event in order for a rule to be applied to an event. A rule with no conditions always matches. Only applicable to `underride` and `override` rules. |
| `pattern` | `string` | Only applicable to `content` rules. The glob-style pattern to match against. |

### Request body example

```json
{

  "actions": [

    "notify"

  ],

  "pattern": "cake*lie"

}
```

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The push rule was created/updated. |
| `400` | There was a problem configuring this push rule. |
| `404` | The push rule does not exist (when updating a push rule). |
| `429` | This request was rate-limited. |

### 200 response

```json
{}
```

### 400 response

```json
{

  "errcode": "M_UNKNOWN",

  "error": "before/after rule not found: someRuleId"

}
```

### 404 response

```json
{

  "errcode": "M_NOT_FOUND",

  "error": "The push rule was not found."

}
```

### 429 response

```json
{

  "errcode": "M_LIMIT_EXCEEDED",

  "error": "Too many requests",

  "retry_after_ms": 2000

}
```

## DELETE /\_matrix/client/v3/pushrules/global/{kind}/{ruleId}

---

This endpoint removes the push rule defined in the path.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `kind` | `string` | **Required:** The kind of rule  One of: `[override, underride, sender, room, content]`. |
| `ruleId` | `string` | **Required:** The identifier for the rule. |

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The push rule was deleted. |
| `404` | The push rule does not exist. |

### 200 response

```json
{}
```

### 404 response

```json
{

  "errcode": "M_NOT_FOUND",

  "error": "The push rule was not found."

}
```

## GET /\_matrix/client/v3/pushrules/global/{kind}/{ruleId}/actions

---

This endpoint get the actions for the specified push rule.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `kind` | `string` | **Required:** The kind of rule  One of: `[override, underride, sender, room, content]`. |
| `ruleId` | `string` | **Required:** The identifier for the rule. |

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The actions for this push rule. |
| `404` | The push rule does not exist. |

### 200 response

```json
{

  "actions": [

    "notify",

    {

      "set_tweak": "sound",

      "value": "bing"

    }

  ]

}
```

### 404 response

```json
{

  "errcode": "M_NOT_FOUND",

  "error": "The push rule was not found."

}
```

## PUT /\_matrix/client/v3/pushrules/global/{kind}/{ruleId}/actions

---

This endpoint allows clients to change the actions of a push rule. This can be used to change the actions of builtin rules.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `kind` | `string` | **Required:** The kind of rule  One of: `[override, underride, sender, room, content]`. |
| `ruleId` | `string` | **Required:** The identifier for the rule. |

### Request body

| Name | Type | Description |
| --- | --- | --- |
| `actions` | `[string\|object]` | **Required:** The action(s) to perform for this rule. |

### Request body example

```json
{

  "actions": [

    "notify",

    {

      "set_tweak": "highlight"

    }

  ]

}
```

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The actions for the push rule were set. |
| `404` | The push rule does not exist. |

### 200 response

```json
{}
```

### 404 response

```json
{

  "errcode": "M_NOT_FOUND",

  "error": "The push rule was not found."

}
```

## GET /\_matrix/client/v3/pushrules/global/{kind}/{ruleId}/enabled

---

This endpoint gets whether the specified push rule is enabled.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `kind` | `string` | **Required:** The kind of rule  One of: `[override, underride, sender, room, content]`. |
| `ruleId` | `string` | **Required:** The identifier for the rule. |

---

## Responses

| Status | Description |
| --- | --- |
| `200` | Whether the push rule is enabled. |
| `404` | The push rule does not exist. |

### 200 response

```json
{

  "enabled": true

}
```

### 404 response

```json
{

  "errcode": "M_NOT_FOUND",

  "error": "The push rule was not found."

}
```

## PUT /\_matrix/client/v3/pushrules/global/{kind}/{ruleId}/enabled---

This endpoint allows clients to enable or disable the specified push rule.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `kind` | `string` | **Required:** The kind of rule  One of: `[override, underride, sender, room, content]`. |
| `ruleId` | `string` | **Required:** The identifier for the rule. |

### Request body

| Name | Type | Description |
| --- | --- | --- |
| `enabled` | `boolean` | **Required:** Whether the push rule is enabled or not. |

### Request body example

```json
{

  "enabled": true

}
```

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The push rule was enabled or disabled. |
| `404` | The push rule does not exist. |

### 200 response

```json
{}
```

### 404 response

```json
{

  "errcode": "M_NOT_FOUND",

  "error": "The push rule was not found."

}
```

### Push Rules: Events

When a user changes their push rules a `m.push_rules` event is sent to all clients in the `account_data` section of their next [`/sync`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3sync) request. The content of the event is the current push rules for the user.

## m.push\_rules

---

Describes all push rules for this user.

| Event type: | Message event |
| --- | --- |

## Content

| Name | Type | Description |
| --- | --- | --- |
| `global` | `[Ruleset](https://spec.matrix.org/unstable/client-server-api/#mpush_rules_ruleset)` | The global ruleset |

## Examples

```json
{

  "content": {

    "global": {

      "content": [

        {

          "actions": [

            "notify",

            {

              "set_tweak": "sound",

              "value": "default"

            },

            {

              "set_tweak": "highlight"

            }

          ],

          "default": true,

          "enabled": true,

          "pattern": "alice",

          "rule_id": ".m.rule.contains_user_name"

        }

      ],

      "override": [

        {

          "actions": [],

          "conditions": [],

          "default": true,

          "enabled": false,

          "rule_id": ".m.rule.master"

        }

      ],

      "room": [],

      "sender": [],

      "underride": []

    }

  }

}
```## Third-party Invites

This module adds in support for inviting new members to a room where their Matrix user ID is not known, instead addressing them by a third-party identifier such as an email address. There are two flows here; one if a Matrix user ID is known for the third-party identifier, and one if not. Either way, the client calls [`/invite`](https://spec.matrix.org/unstable/client-server-api/#thirdparty_post_matrixclientv3roomsroomidinvite) with the details of the third-party identifier.

The homeserver asks the identity server whether a Matrix user ID is known for that identifier:

- If it is, an invite is simply issued for that user.
- If it is not, the homeserver asks the identity server to record the details of the invitation, and to notify the invitee's homeserver of this pending invitation if it gets a binding for this identifier in the future. The identity server returns a token and public key to the inviting homeserver.

When the invitee's homeserver receives the notification of the binding, it should insert an `m.room.member` event into the room's graph for that user, with `content.membership` = `invite`, as well as a `content.third_party_invite` property which contains proof that the invitee does indeed own that third-party identifier.

### Events

## m.room.third\_party\_invite

---

Acts as an `m.room.member` invite event, where there isn't a target user\_id to invite. This event contains a token and a public key whose private key must be used to sign the token. Any user who can present that signature may use this invitation to join the target room.

| Event type: | State event |
| --- | --- |
| State key | The token, of which a signature must be produced in order to join the room. |

## Content

| Name | Type | Description |
| --- | --- | --- |
| `display_name` | `string` | **Required:** A user-readable string which represents the user who has been invited. This should not contain the user's third-party ID, as otherwise when the invite is accepted it would leak the association between the matrix ID and the third-party ID. |
| `key_validity_url` | `[URI](https://datatracker.ietf.org/doc/html/rfc3986)` | **Required:** A URL which can be fetched, with querystring public\_key=public\_key, to validate whether the key has been revoked. The URL must return a JSON object containing a boolean property named 'valid'. |
| `public_key` | `string` | **Required:** An Ed25519 key with which the token must be signed (though a signature from any entry in `public_keys` is also sufficient).  The key is encoded using [Unpadded Base64](https://spec.matrix.org/unstable/appendices/#unpadded-base64), using the standard or URL-safe alphabets.  This exists for backwards compatibility. |
| `public_keys` | `[[PublicKeys](https://spec.matrix.org/unstable/client-server-api/#mroomthird_party_invite_publickeys)]` | Keys with which the token may be signed. |

### Client behaviour

## POST /\_matrix/client/v3/rooms/{roomId}/invite

---

*Note that there are two forms of this API, which are documented separately. This version of the API does not require that the inviter know the Matrix identifier of the invitee, and instead relies on third-party identifiers.*

This API invites a user to participate in a particular room. They do not start participating in the room until they actually join the room.

Only users currently in a particular room can invite other users to join that room.

| Rate-limited: | Yes |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `roomId` | `[Room ID](https://spec.matrix.org/unstable/appendices#room-ids)` | **Required:** The room identifier (not alias) to which to invite the user. |

### Request body

| Name | Type | Description |
| --- | --- | --- |
| `address` | `string` | **Required:** The invitee's third-party identifier. |
| `id_access_token` | `string` | **Required:** An access token previously registered with the identity server. |
| `id_server` | `string` | **Required:** The hostname+port of the identity server which should be used for third-party identifier lookups. |
| `medium` | `string` | **Required:** The kind of address being passed in the address field, for example `email`. |

### Request body example

```json
{

  "address": "cheeky@monkey.com",

  "id_access_token": "abc123_OpaqueString",

  "id_server": "matrix.org",

  "medium": "email"

}
```

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The user has been invited to join the room. |
| `403` | You do not have permission to invite the user to the room. A meaningful `errcode` and description error text will be returned. Example reasons for rejections are:  - The invitee has been banned from the room. - The invitee is already a member of the room. - The inviter is not currently in the room. - The inviter's power level is insufficient to invite users to the room. |
| `429` | This request was rate-limited. |

### 200 response

```json
{}
```

### 403 response

```json
{

  "errcode": "M_FORBIDDEN",

  "error": "@cheeky_monkey:matrix.org is banned from the room"

}
```

## Server Side Search

The search API allows clients to perform full text search across events in all rooms that the user has been in, including those that they have left. Only events that the user is allowed to see will be searched, e.g. it won't include events in rooms that happened after you left.

### Client behaviour

There is a single HTTP API for performing server-side search, documented below.

## POST /\_matrix/client/v3/search

---

Performs a full text search across different categories.

| Rate-limited: | Yes |
| --- | --- |
| Requires authentication: | Yes |