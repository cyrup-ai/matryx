# Matrix Protocol Domain Objects

*Comprehensive catalog of all domain objects extracted from Matrix specifications*

---

EventRetrievalTransaction - spec/server/23-retrieving-events.md:47-49
- origin: string
- origin_server_ts: integer
- pdus: array<PDU>

StateRetrievalRequest - spec/server/23-retrieving-events.md:101
- event_id: string

OpenIdUserInfoResponse - spec/server/26-openid.md:52
- sub: string

OpenIdErrorResponse - spec/server/26-openid.md:66-67
- errcode: string
- error: string (optional)

KeyClaimRequest - spec/server/27-end-to-end-encryption.md:39
- one_time_keys: map<string, map<string, string>>

KeyClaimResponse - spec/server/27-end-to-end-encryption.md:71
- one_time_keys: map<string, map<string, map<string, KeyObject>>>

KeyObject - spec/server/27-end-to-end-encryption.md:75-76
- key: string
- signatures: map<string, map<string, string>>

KeyQueryRequest - spec/server/27-end-to-end-encryption.md:130
- device_keys: map<string, array<string>>

KeyQueryResponse - spec/server/27-end-to-end-encryption.md:158-160
- device_keys: map<string, map<string, DeviceKeys>>
- master_keys: map<string, CrossSigningKey> (optional)
- self_signing_keys: map<string, CrossSigningKey> (optional)

DeviceKeys - spec/server/27-end-to-end-encryption.md:164-169
- algorithms: array<string>
- device_id: string
- keys: map<string, string>
- signatures: map<string, map<string, string>>
- unsigned: UnsignedDeviceInfo (optional)
- user_id: string

UnsignedDeviceInfo - spec/server/27-end-to-end-encryption.md:173
- device_display_name: string (optional)

CrossSigningKey - spec/server/27-end-to-end-encryption.md:177-180
- keys: map<string, string>
- signatures: map<string, map<string, string>> (optional)
- usage: array<string>
- user_id: string

SigningKeyUpdateEDU - spec/server/27-end-to-end-encryption.md:246-247
- content: SigningKeyUpdate
- edu_type: string

SigningKeyUpdate - spec/server/27-end-to-end-encryption.md:251-253
- master_key: CrossSigningKey (optional)
- self_signing_key: CrossSigningKey (optional)
- user_id: string

RoomAliasResponse - spec/client/02_rooms_users.md:165-167
- room_id: string
- servers: array<string>

RoomAliasMapping - spec/client/02_rooms_users.md:217
- room_id: string

RoomAliasesResponse - spec/client/02_rooms_users.md:365
- aliases: array<string>

RateLimitResponse - spec/client/02_rooms_users.md:418-420
- errcode: string
- error: string
- retry_after_ms: integer

JoinedRoomsResponse - spec/client/02_rooms_users.md:484
- joined_rooms: array<string>

InviteUserRequest - spec/client/02_rooms_users.md:523-524
- user_id: string
- reason: string (optional)

JoinRoomRequest - spec/client/02_rooms_users.md:619-620
- reason: string (optional)
- third_party_signed: ThirdPartySigned (optional)

ThirdPartySigned - spec/client/02_rooms_users.md:624-627
- mxid: string
- sender: string
- signatures: map<string, map<string, string>>
- token: string

JoinRoomResponse - spec/client/02_rooms_users.md:661
- room_id: string

JoinRoomByIdRequest - spec/client/02_rooms_users.md:751-752
- reason: string (optional)
- third_party_signed: ThirdPartySigned (optional)

KnockRoomRequest - spec/client/02_rooms_users.md:885
- reason: string (optional)

KnockRoomResponse - spec/client/02_rooms_users.md:901
- room_id: string

ToDevice - spec/client/04_security_encryption.md:86
- events: array<Event>

Event - spec/client/04_security_encryption.md:92-96
- content: EventContent
- sender: string
- type: string

SendToDeviceRequest - spec/client/04_security_encryption.md:54
- messages: map<string, map<string, EventContent>>

DeleteDevicesRequest - spec/client/04_security_encryption.md:145-146
- auth: AuthenticationData
- devices: array<string>

AuthenticationData - spec/client/04_security_encryption.md:148-152
- session: string (optional)
- type: string (optional)

DeviceManagementResponse401 - spec/client/04_security_encryption.md:163-169
- completed: array<string>
- flows: array<FlowInformation>
- params: map<string, object>
- session: string

FlowInformation - spec/client/04_security_encryption.md:171-172
- stages: array<string>

DevicesListResponse - spec/client/04_security_encryption.md:198
- devices: array<Device>

Device - spec/client/04_security_encryption.md:200-205
- device_id: string
- display_name: string (optional)
- last_seen_ip: string (optional)
- last_seen_ts: integer (optional)

UpdateDeviceRequest - spec/client/04_security_encryption.md:324
- display_name: string (optional)

DeleteDeviceRequest - spec/client/04_security_encryption.md:363
- auth: AuthenticationData

KeyObject - spec/client/04_security_encryption.md:507-511
- key: string
- signatures: Signatures
- fallback: boolean (optional)

EncryptedFile - spec/client/04_security_encryption.md:612-617
- url: string
- key: JWK
- iv: string
- hashes: map<string, string>
- v: string

JWK - spec/client/04_security_encryption.md:619-624
- kty: string
- key_ops: array<string>
- alg: string
- k: string
- ext: boolean

VerificationRequestInRoom - spec/client/04_security_encryption.md:763-770
- body: string
- format: string (optional)
- formatted_body: string (optional)
- from_device: string
- methods: array<string>
- msgtype: string
- to: string

VerificationRequestToDevice - spec/client/04_security_encryption.md:787-791
- from_device: string
- methods: array<string>
- timestamp: integer
- transaction_id: string

VerificationReady - spec/client/04_security_encryption.md:806-810
- from_device: string
- m_relates_to: VerificationRelatesTo (optional)
- methods: array<string>
- transaction_id: string (optional)

VerificationRelatesTo - spec/client/04_security_encryption.md:812-814
- event_id: string
- rel_type: string

VerificationStart - spec/client/04_security_encryption.md:828-834
- from_device: string
- m_relates_to: VerificationRelatesTo (optional)
- method: string
- next_method: string (optional)
- transaction_id: string (optional)

VerificationDone - spec/client/04_security_encryption.md:861-863
- m_relates_to: VerificationRelatesTo (optional)
- transaction_id: string (optional)

VerificationCancel - spec/client/04_security_encryption.md:878-882
- code: string
- m_relates_to: VerificationRelatesTo (optional)
- reason: string
- transaction_id: string (optional)

SASVerificationStart - spec/client/04_security_encryption.md:1157-1165
- from_device: string
- hashes: array<string>
- key_agreement_protocols: array<string>
- m_relates_to: VerificationRelatesTo (optional)
- message_authentication_codes: array<string>
- method: string
- short_authentication_string: array<string>
- transaction_id: string (optional)

VerificationAccept - spec/client/04_security_encryption.md:1188-1196
- commitment: string
- hash: string
- key_agreement_protocol: string
- m_relates_to: VerificationRelatesTo (optional)
- message_authentication_code: string
- short_authentication_string: array<string>
- transaction_id: string (optional)

VerificationKey - spec/client/04_security_encryption.md:1223-1226
- key: string
- m_relates_to: VerificationRelatesTo (optional)
- transaction_id: string (optional)

VerificationMAC - spec/client/04_security_encryption.md:1245-1249
- keys: string
- m_relates_to: VerificationRelatesTo (optional)
- mac: map<string, string>
- transaction_id: string (optional)

CrossSigningUploadRequest - spec/client/04_security_encryption.md:1506-1510
- auth: AuthenticationData
- master_key: CrossSigningKey (optional)
- self_signing_key: CrossSigningKey (optional)
- user_signing_key: CrossSigningKey (optional)

CrossSigningKey - spec/client/04_security_encryption.md:1516-1521
- keys: map<string, string>
- signatures: Signatures (optional)
- usage: array<string>
- user_id: string

SignaturesUploadRequest - spec/client/04_security_encryption.md:1547
- signatures: map<string, map<string, object>>

SignaturesUploadResponse - spec/client/04_security_encryption.md:1668
- failures: map<string, map<string, Error>>

QRReciprocateStart - spec/client/04_security_encryption.md:1818-1822
- from_device: string
- m_relates_to: VerificationRelatesTo (optional)
- method: string
- secret: string
- transaction_id: string (optional)

BackupAuthData - spec/client/04_security_encryption.md:1895-1897
- public_key: string
- signatures: object

BackedUpSessionData - spec/client/04_security_encryption.md:1910-1915
- algorithm: string
- forwarding_curve25519_key_chain: array<string>
- sender_claimed_keys: map<string, string>
- sender_key: string
- session_key: string

RoomKeysGetResponse - spec/client/04_security_encryption.md:1943
- rooms: map<string, RoomKeyBackup>

RoomKeyBackup - spec/client/04_security_encryption.md:1945
- sessions: map<string, KeyBackupData>

KeyBackupData - spec/client/04_security_encryption.md:1947-1951
- first_message_index: integer
- forwarded_count: integer
- is_verified: boolean
- session_data: object

RoomKeysPutRequest - spec/client/04_security_encryption.md:2020
- rooms: map<string, RoomKeyBackup>

RoomKeysPutResponse - spec/client/04_security_encryption.md:2070-2072
- count: integer
- etag: string

RoomKeysDeleteResponse - spec/client/04_security_encryption.md:2245-2247
- count: integer
- etag: string

RoomKeysByRoomGetResponse - spec/client/04_security_encryption.md:2307
- sessions: map<string, KeyBackupData>

RoomKeysByRoomPutRequest - spec/client/04_security_encryption.md:2394
- sessions: map<string, KeyBackupData>

RoomKeysByRoomPutResponse - spec/client/04_security_encryption.md:2455-2457
- count: integer
- etag: string

HistoryVisibilityEvent - spec/client/05_advanced_features.md:47
- history_visibility: string

PushRulesGetResponse - spec/client/05_advanced_features.md:1018
- global: Ruleset

Ruleset - spec/client/05_advanced_features.md:1020-1024
- content: array<PushRule>
- override: array<PushRule>
- room: array<PushRule>
- sender: array<PushRule>
- underride: array<PushRule>

PushRule - spec/client/05_advanced_features.md:1026-1032
- actions: array<object|string>
- conditions: array<PushCondition> (optional)
- default: boolean
- enabled: boolean
- pattern: string (optional)
- rule_id: string

PushCondition - spec/client/05_advanced_features.md:1034-1040
- is: string (optional)
- key: string (optional)
- kind: string
- pattern: string (optional)
- value: object|string|integer|boolean|null (optional)

PushRuleCreateUpdateRequest - spec/client/05_advanced_features.md:1638-1641
- actions: array<object|string>
- conditions: array<PushCondition> (optional)
- pattern: string (optional)

PushRuleActionsUpdateRequest - spec/client/05_advanced_features.md:1848
- actions: array<object|string>

PushRuleEnabledGetResponse - spec/client/05_advanced_features.md:1892
- enabled: boolean

PushRuleEnabledUpdateRequest - spec/client/05_advanced_features.md:1936
- enabled: boolean

PushRulesEvent - spec/client/05_advanced_features.md:1978
- global: Ruleset

ThirdPartyInviteEvent - spec/client/05_advanced_features.md:2065-2069
- display_name: string
- key_validity_url: string
- public_key: string
- public_keys: array<PublicKeys>

PublicKeys - spec/client/05_advanced_features.md:2069
- key: string

ThirdPartyInviteRequest - spec/client/05_advanced_features.md:2120-2124
- address: string
- id_access_token: string
- id_server: string
- medium: string

RoomTag - spec/client/06_user_experience.md:74
- order: number

TagCollection - spec/client/06_user_experience.md:70
- tags: map<string, RoomTag>

GlobalAccountData - spec/client/06_user_experience.md:267-306
- type: string
- content: any

RoomAccountData - spec/client/06_user_experience.md:418-495
- type: string
- room_id: string
- content: any

SpaceChildEvent - spec/client/07_relationship_features.md:102-106
- order: string (optional)
- suggested: boolean (optional)
- via: array<string>

SpaceParentEvent - spec/client/07_relationship_features.md:182-185
- canonical: boolean (optional)
- via: array<string>

SpaceHierarchyRequest - spec/client/07_relationship_features.md:264-268
- from: string (optional)
- limit: integer (optional)
- max_depth: integer (optional)
- suggested_only: boolean (optional)

SpaceHierarchyResponse - spec/client/07_relationship_features.md:284-286
- next_batch: string (optional)
- rooms: array<SpaceHierarchyRoom>

SpaceHierarchyRoom - spec/client/07_relationship_features.md:290-338
- avatar_url: string (optional)
- canonical_alias: string (optional)
- children_state: array<object>
- guest_can_join: boolean
- join_rule: string
- name: string (optional)
- num_joined_members: integer
- room_id: string
- room_type: string (optional)
- topic: string (optional)
- world_readable: boolean

EventReplacementContent - spec/client/07_relationship_features.md:364-378
- m.new_content: object
- m.relates_to: EventRelatesTo

EventRelatesTo - spec/client/07_relationship_features.md:372-377
- rel_type: string
- event_id: string

PDU - spec/server/01-introduction.md:11-14
- broadcast_event: object
- room_context: string
- persistent: boolean

EDU - spec/server/01-introduction.md:16-17
- ephemeral_event: object
- non_persistent: boolean

Query - spec/server/01-introduction.md:19-20
- request: object
- response: object
- snapshot_state: boolean

Transaction - spec/server/01-introduction.md:22
- pdus: array<PDU> (optional)
- edus: array<EDU> (optional)
- envelope: object

ServerInfo - spec/server/01-introduction.md:72-78
- server: ServerDetails

ServerDetails - spec/server/01-introduction.md:74-77
- name: string
- version: string

WellKnownServerResponse - spec/server/02-server-discovery.md:54-60
- m.server: string

ServerKeysResponse - spec/server/03-server-keys.md:31-51
- old_verify_keys: map<string, OldVerifyKey> (optional)
- server_name: string
- signatures: map<string, map<string, string>>
- valid_until_ts: integer
- verify_keys: map<string, VerifyKey>

VerifyKey - spec/server/03-server-keys.md:61-62
- key: string

OldVerifyKey - spec/server/03-server-keys.md:64-66
- key: string
- expired_ts: integer

KeyQueryRequest - spec/server/03-server-keys.md:82-91
- server_keys: map<string, map<string, QueryCriteria>>

QueryCriteria - spec/server/03-server-keys.md:97-98
- minimum_valid_until_ts: integer (optional)

KeyQueryResponse - spec/server/03-server-keys.md:104-131
- server_keys: array<ServerKeysResponse>

AuthenticationRequest - spec/server/04-authentication.md:12-25
- method: string
- uri: string
- origin: string
- destination: string
- content: object
- signatures: map<string, map<string, string>>

AuthorizationHeader - spec/server/04-authentication.md:52-59
- origin: string
- destination: string (optional)
- key: string
- signature: string

FederationTransaction - spec/server/05-transactions.md:22-35
- origin: string
- origin_server_ts: integer
- pdus: array<PDU>
- edus: array<EDU> (optional)

FederationEDU - spec/server/05-transactions.md:44-46
- edu_type: string
- content: object

TransactionResponse - spec/server/05-transactions.md:51-59
- pdus: map<string, TransactionResult>

TransactionResult - spec/server/05-transactions.md:62-64
- error: string (optional)

AuthChainResponse - spec/server/06-pdus.md:156-173
- auth_chain: array<PDU>

TypingNotificationEDU - spec/server/07-edus.md:19-25
- content: TypingNotification
- edu_type: string

TypingNotification - spec/server/07-edus.md:27-31
- room_id: string
- typing: boolean
- user_id: string

PresenceEDU - spec/server/07-edus.md:46-50
- content: PresenceUpdate
- edu_type: string

PresenceUpdate - spec/server/07-edus.md:52-56
- push: array<UserPresenceUpdate>

UserPresenceUpdate - spec/server/07-edus.md:58-66
- currently_active: boolean (optional)
- last_active_ago: integer
- presence: string
- status_msg: string (optional)
- user_id: string

ReceiptEDU - spec/server/07-edus.md:87-91
- content: map<string, RoomReceipts>
- edu_type: string

RoomReceipts - spec/server/07-edus.md:93-97
- m.read: map<string, UserReadReceipt>

UserReadReceipt - spec/server/07-edus.md:99-103
- data: ReadReceiptMetadata
- event_ids: array<string>

ReadReceiptMetadata - spec/server/07-edus.md:105-109
- ts: integer

DeviceListUpdateEDU - spec/server/07-edus.md:128-132
- content: DeviceListUpdate
- edu_type: string

DeviceListUpdate - spec/server/07-edus.md:134-144
- device_display_name: string (optional)
- device_id: string
- deleted: boolean (optional)
- keys: object (optional)
- prev_id: array<string>
- stream_id: integer
- user_id: string

RoomStateResponse - spec/server/08-room-state.md:150-181
- auth_chain: array<PDU>
- pdus: array<PDU>

MakeJoinResponse - spec/server/09-room-joins.md:96-105
- event: EventTemplate
- room_version: string

EventTemplate - spec/server/09-room-joins.md:107-115
- content: MembershipEventContent
- origin: string
- origin_server_ts: integer
- sender: string
- state_key: string
- type: string

MembershipEventContent - spec/server/09-room-joins.md:117-122
- join_authorised_via_users_server: string (optional)
- membership: string

SendJoinRequest - spec/server/09-room-joins.md:151-159
- content: MembershipEventContent
- origin: string
- origin_server_ts: integer
- sender: string
- state_key: string
- type: string

SendJoinResponse - spec/server/09-room-joins.md:174-180
- response: array<integer, SendJoinRoomState>

SendJoinRoomState - spec/server/09-room-joins.md:174-180
- auth_chain: array<PDU>
- state: array<PDU>

MakeLeaveResponse - spec/server/10-room-leaves.md:34-43
- event: LeaveEventTemplate
- room_version: string

LeaveEventTemplate - spec/server/10-room-leaves.md:45-53
- content: LeaveMembershipEventContent
- origin: string
- origin_server_ts: integer
- sender: string
- state_key: string
- type: string

LeaveMembershipEventContent - spec/server/10-room-leaves.md:55-59
- membership: string

SendLeaveRequest - spec/server/10-room-leaves.md:108-116
- content: LeaveMembershipEventContent
- depth: integer
- origin: string
- origin_server_ts: integer
- sender: string
- state_key: string
- type: string

SendLeaveV1Response - spec/server/10-room-leaves.md:130-135
- response: array<integer, object>

SendLeaveV2Response - spec/server/10-room-leaves.md:197-201
- response: object

InviteV1Request - spec/server/11-room-invites.md:45-70
- content: InviteMembershipEventContent
- origin: string
- origin_server_ts: integer
- sender: string
- state_key: string
- type: string
- unsigned: UnsignedData

InviteMembershipEventContent - spec/server/11-room-invites.md:72-75
- membership: string

UnsignedData - spec/server/11-room-invites.md:77-80
- invite_room_state: array<StrippedStateEvent> (optional)

StrippedStateEvent - spec/server/11-room-invites.md:82-88
- content: EventContent
- sender: string
- state_key: string
- type: string

InviteV1Response - spec/server/11-room-invites.md:130-135
- response: array<integer, InviteEventContainer>

InviteEventContainer - spec/server/11-room-invites.md:137-145
- event: InviteEvent

InviteEvent - spec/server/11-room-invites.md:147-155
- content: InviteMembershipEventContent
- origin: string
- origin_server_ts: integer
- sender: string
- state_key: string
- type: string

InviteV2Request - spec/server/11-room-invites.md:195-205
- event: InviteEvent
- invite_room_state: array<StrippedStateEvent> (optional)
- room_version: string

InviteV2Response - spec/server/11-room-invites.md:280-285
- event: InviteEvent

ThirdPartyBindRequest - spec/server/11-room-invites.md:365-375
- address: string
- invites: array<ThirdPartyInvite>
- medium: string
- mxid: string

ThirdPartyInvite - spec/server/11-room-invites.md:377-385
- address: string
- medium: string
- mxid: string
- room_id: string
- sender: string
- signed: SignedThirdPartyInvite

SignedThirdPartyInvite - spec/server/11-room-invites.md:387-395
- mxid: string
- signatures: map<string, map<string, string>>
- token: string

ExchangeThirdPartyInviteRequest - spec/server/11-room-invites.md:475-485
- content: ThirdPartyInviteEventContent
- room_id: string
- sender: string
- state_key: string
- type: string

ThirdPartyInviteEventContent - spec/server/11-room-invites.md:487-492
- membership: string
- third_party_invite: ThirdPartyInviteData

ThirdPartyInviteData - spec/server/11-room-invites.md:494-498
- display_name: string
- signed: SignedThirdPartyInvite

MakeKnockResponse - spec/server/12-room-knocking.md:65-70
- event: KnockEventTemplate
- room_version: string

KnockEventTemplate - spec/server/12-room-knocking.md:72-80
- content: KnockMembershipEventContent
- origin: string
- origin_server_ts: integer
- sender: string
- state_key: string
- type: string

KnockMembershipEventContent - spec/server/12-room-knocking.md:82-85
- membership: string

SendKnockRequest - spec/server/12-room-knocking.md:200-210
- content: KnockMembershipEventContent
- origin: string
- origin_server_ts: integer
- sender: string
- state_key: string
- type: string

SendKnockResponse - spec/server/12-room-knocking.md:250-255
- knock_room_state: array<KnockStrippedStateEvent>

KnockStrippedStateEvent - spec/server/12-room-knocking.md:257-265
- content: EventContent
- sender: string
- state_key: string
- type: string

PublicRoomsGetResponse - spec/server/13-public-rooms.md:45-55
- chunk: array<PublishedRoomsChunk>
- next_batch: string (optional)
- prev_batch: string (optional)  
- total_room_count_estimate: integer (optional)

PublishedRoomsChunk - spec/server/13-public-rooms.md:57-75
- avatar_url: string (optional)
- canonical_alias: string (optional)
- guest_can_join: boolean
- join_rule: string (optional)
- name: string (optional)
- num_joined_members: integer
- room_id: string
- room_type: string (optional)
- topic: string (optional)
- world_readable: boolean

PublicRoomsPostRequest - spec/server/13-public-rooms.md:120-135
- filter: PublicRoomsFilter (optional)
- include_all_networks: boolean (optional)
- limit: integer (optional)
- since: string (optional)
- third_party_instance_id: string (optional)

PublicRoomsFilter - spec/server/13-public-rooms.md:137-145
- generic_search_term: string (optional)
- room_types: array<string|null> (optional)

SpaceHierarchyResponse - spec/server/13-public-rooms.md:250-260
- children: array<SpaceHierarchyChildRoomsChunk>
- inaccessible_children: array<string>
- room: SpaceHierarchyParentRoom

SpaceHierarchyChildRoomsChunk - spec/server/13-public-rooms.md:262-285
- allowed_room_ids: array<string> (optional)
- avatar_url: string (optional)
- canonical_alias: string (optional)
- children_state: array<SpaceHierarchyStrippedStateEvent>
- encryption: string (optional)
- guest_can_join: boolean
- join_rule: string (optional)
- name: string (optional)
- num_joined_members: integer
- room_id: string
- room_type: string (optional)
- room_version: string (optional)
- topic: string (optional)
- world_readable: boolean

SpaceHierarchyParentRoom - spec/server/13-public-rooms.md:287-310
- allowed_room_ids: array<string> (optional)
- avatar_url: string (optional)
- canonical_alias: string (optional)
- children_state: array<SpaceHierarchyStrippedStateEvent>
- encryption: string (optional)
- guest_can_join: boolean
- join_rule: string (optional)
- name: string (optional)
- num_joined_members: integer
- room_id: string
- room_type: string (optional)
- room_version: string (optional)
- topic: string (optional)
- world_readable: boolean

SpaceHierarchyStrippedStateEvent - spec/server/13-public-rooms.md:312-320
- content: EventContent
- origin_server_ts: integer
- sender: string
- state_key: string
- type: string

DeviceListResponse - spec/server/17-device-management.md:34-58
- devices: array<DeviceInfo>
- master_key: CrossSigningKey (optional)
- self_signing_key: CrossSigningKey (optional)
- stream_id: integer
- user_id: string

DeviceInfo - spec/server/17-device-management.md:36-52
- device_display_name: string (optional)
- device_id: string
- keys: DeviceKeys

OneTimeKeyClaimRequest - spec/server/17-device-management.md:84-91
- one_time_keys: map<string, map<string, string>>

OneTimeKeyClaimResponse - spec/server/17-device-management.md:93-111
- one_time_keys: map<string, map<string, map<string, OneTimeKeyObject>>>

OneTimeKeyObject - spec/server/17-device-management.md:107-111
- key: string
- signatures: map<string, map<string, string>>

KeyQueryRequest - spec/server/17-device-management.md:115-121
- device_keys: map<string, array<string>>

KeyQueryResponse - spec/server/17-device-management.md:123-175
- device_keys: map<string, map<string, DeviceKeys>>
- master_keys: map<string, CrossSigningKey> (optional)
- self_signing_keys: map<string, CrossSigningKey> (optional)

DirectToDeviceEDU - spec/server/17-device-management.md:311-327
- content: DirectToDeviceContent
- edu_type: string

DirectToDeviceContent - spec/server/17-device-management.md:313-327
- sender: string
- message_id: string
- messages: map<string, map<string, EncryptedContent>>

ToDeviceMessage - spec/server/18-send-to-device.md:18-27
- message_id: string
- messages: map<string, map<string, object>>
- sender: string
- type: string

BackfillResponse - spec/server/22-backfill-events.md:52-63
- origin: string
- origin_server_ts: integer
- pdus: array<PDU>

MissingEventsRequest - spec/server/22-backfill-events.md:97-107
- earliest_events: array<string>
- latest_events: array<string>
- limit: integer (optional)
- min_depth: integer (optional)

MissingEventsResponse - spec/server/22-backfill-events.md:138-148
- events: array<PDU>

PublishedRoomsResponse - spec/server/13-public-rooms.md:45-64
- chunk: array<PublishedRoomsChunk>
- next_batch: string (optional)
- prev_batch: string (optional)
- total_room_count_estimate: integer (optional)

PublishedRoomsChunk - spec/server/13-public-rooms.md:66-85
- avatar_url: string (optional)
- canonical_alias: string (optional)
- guest_can_join: boolean
- join_rule: string (optional)
- name: string (optional)
- num_joined_members: integer
- room_id: string
- room_type: string (optional)
- topic: string (optional)
- world_readable: boolean

PublicRoomsFilterRequest - spec/server/13-public-rooms.md:175-185
- filter: Filter (optional)
- include_all_networks: boolean (optional)
- limit: integer (optional)
- since: string (optional)
- third_party_instance_id: string (optional)

Filter - spec/server/13-public-rooms.md:187-194
- generic_search_term: string (optional)
- room_types: array<string> (optional)

SpaceHierarchyResponse - spec/server/13-public-rooms.md:300-316
- children: array<SpaceHierarchyChildRoomsChunk>
- inaccessible_children: array<string>
- room: SpaceHierarchyParentRoom

SpaceHierarchyChildRoomsChunk - spec/server/13-public-rooms.md:318-345
- allowed_room_ids: array<string> (optional)
- avatar_url: string (optional)
- canonical_alias: string (optional)
- children_state: array<StrippedStateEvent>
- encryption: string (optional)
- guest_can_join: boolean
- join_rule: string (optional)
- name: string (optional)
- num_joined_members: integer
- room_id: string
- room_type: string (optional)
- room_version: string (optional)
- topic: string (optional)
- world_readable: boolean

SpaceHierarchyParentRoom - spec/server/13-public-rooms.md:347-374
- allowed_room_ids: array<string> (optional)
- avatar_url: string (optional)
- canonical_alias: string (optional)
- children_state: array<StrippedStateEvent>
- encryption: string (optional)
- guest_can_join: boolean
- join_rule: string (optional)
- name: string (optional)
- num_joined_members: integer
- room_id: string
- room_type: string (optional)
- room_version: string (optional)
- topic: string (optional)
- world_readable: boolean

StrippedStateEvent - spec/server/13-public-rooms.md:376-383
- content: EventContent
- origin_server_ts: integer
- sender: string
- state_key: string
- type: string