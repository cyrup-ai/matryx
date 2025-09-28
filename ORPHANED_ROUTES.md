# Orphaned Routes - Functions Not Wired to Router

## Functions with "never used" warnings (295 total)

These are endpoint implementations that exist but are not connected to the router.

### Matrix Client-Server API Endpoints (Not in Router)

#### Third Party / Location Services
- `packages/server/src/_matrix/client/v3/thirdparty/location/by_alias.rs:5` - `get` function
- `packages/server/src/_matrix/client/v3/thirdparty/user/by_userid.rs:5` - `get` function

#### User Account Data
- `packages/server/src/_matrix/client/v3/user/by_user_id/account_data/by_type.rs` - Multiple structs: `AccountData`, `DirectMessageData`, `IgnoredUserList`
- `packages/server/src/_matrix/client/v3/user/by_user_id/rooms/by_room_id/account_data/by_type.rs` - `AccountData` struct

### Matrix Sync System (Not Integrated)

#### Sync Filtering Functions
- `packages/server/src/_matrix/client/v3/sync/filters/live_filters.rs:14` - `handle_filter_live_updates`
- `packages/server/src/_matrix/client/v3/sync/filters/live_filters.rs:42` - `get_with_live_filters`
- `packages/server/src/_matrix/client/v3/sync/filters/room_filters.rs:11` - `apply_room_event_filter`
- `packages/server/src/_matrix/client/v3/sync/filters/url_filters.rs:4` - `apply_contains_url_filter`
- `packages/server/src/_matrix/client/v3/sync/filters/url_filters.rs:20` - `detect_urls_in_event`
- `packages/server/src/_matrix/client/v3/sync/filters/url_filters.rs:30` - `detect_urls_in_json`

#### Sync Streaming Functions
- `packages/server/src/_matrix/client/v3/sync/streaming/filter_streams.rs:14` - `handle_filter_live_updates`
- `packages/server/src/_matrix/client/v3/sync/streaming/filter_streams.rs:42` - `get_with_live_filters`
- `packages/server/src/_matrix/client/v3/sync/streaming/membership_streams.rs:76` - `integrate_live_membership_with_lazy_loading`

#### Sync Utilities
- `packages/server/src/_matrix/client/v3/sync/utils.rs:5` - `convert_events_to_matrix_format`

### Matrix Federation (Not Integrated)

#### Public Rooms
- `packages/server/src/_matrix/federation/v1/public_rooms.rs:374` - `get_room_visibility_settings`
- `packages/server/src/_matrix/federation/v1/public_rooms.rs:385` - `get_total_public_rooms_count`

### Matrix Media Repository (Not Integrated)

#### Media Download
- `packages/server/src/_matrix/media/v3/download/mod.rs:20` - `download_media`

#### Media Thumbnails
- `packages/server/src/_matrix/media/v3/thumbnail/mod.rs` - Multiple functions:
  - `default_method`, `generate_thumbnail`, `get_thumbnail`
  - `is_image_content_type`, `get_image_format`
  - Structs: `ThumbnailQuery`, `ThumbnailMethod`

#### Media Upload
- `packages/server/src/_matrix/media/v3/upload/mod.rs:30` - `MediaFile` struct

#### Media Preview URL
- `packages/server/src/_matrix/media/v3/preview_url.rs:21` - `ts` field never read

### WebSocket Support (Not Integrated)
- `packages/server/src/_matrix/websocket.rs` - Complete WebSocket implementation not wired up:
  - `websocket_handler`, `handle_websocket_connection`
  - `handle_outgoing_messages`, `handle_incoming_messages`
  - `handle_ping`, `handle_sync_with_service`
  - Multiple structs: `SyncWebSocketQuery`, `WebSocketMessage`, `PingMessage`, `PongMessage`

### Authentication & Authorization (Not Integrated)
- `packages/server/src/auth/authenticated_user.rs:38` - `is_admin` method
- `packages/server/src/auth/captcha.rs` - Complete CAPTCHA system not integrated
- `packages/server/src/auth/matrix_auth.rs` - Multiple unused fields and methods
- `packages/server/src/auth/middleware.rs` - Multiple auth middleware functions not used

### Core Matrix Services (Not Integrated)

#### Push Notifications
- `packages/server/src/push/` - Complete push notification system not integrated

#### Reactions System
- `packages/server/src/reactions.rs` - Complete reaction system not integrated

#### Threading System  
- `packages/server/src/threading.rs` - Complete threading system not integrated

#### Server Notices
- `packages/server/src/server_notices.rs` - Server notices system not integrated

#### Room Management
- `packages/server/src/room/` - Multiple room management functions not integrated

#### Security & Crypto
- `packages/server/src/security/cross_signing.rs` - Cross-signing system not integrated

### Utility Functions (Not Used)
- `packages/server/src/utils/` - Multiple utility functions not integrated
- `packages/server/src/response/mod.rs` - Response helper functions not used

### AppState Fields (Not Used)
- `packages/server/src/state.rs` - Multiple fields in AppState not being used

## Action Plan

1. **Identify**: Get complete list from cargo check warnings
2. **Categorize**: Group by functionality (client API, federation, sync, etc.)
3. **Research**: Check Matrix specification for proper implementation
4. **Implement**: Wire up to router or integrate into existing flows
5. **Test**: Ensure Matrix protocol compliance

## Matrix Specification References

- Client-Server API: `./spec/client/*.md`
- Server-Server API: `./spec/server/*.md`
- Current router: `./packages/server/src/main.rs`