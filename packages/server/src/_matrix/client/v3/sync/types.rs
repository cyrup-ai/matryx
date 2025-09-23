// Re-export all sync types from the entity package to eliminate duplication
// All these types are now properly defined in packages/entity/src/types/sync.rs

pub use matryx_entity::types::{
    AccountDataResponse,
    AccountDataUpdate,
    DeviceListsResponse,
    DeviceListsUpdate,
    EphemeralResponse,
    EphemeralUpdate,
    InvitedRoomResponse,
    InvitedRoomUpdate,
    JoinedRoomResponse,
    JoinedRoomUpdate,
    LeftRoomResponse,
    LeftRoomUpdate,

    LiveSyncUpdate,

    PresenceResponse,
    // Component response types
    RoomSummary,
    // Room response types
    RoomsResponse,
    RoomsUpdate,
    StateResponse,
    StateUpdate,
    // Sync request/response types
    SyncQuery,
    SyncResponse,
    TimelineResponse,
    TimelineUpdate,
    ToDeviceResponse,
    ToDeviceUpdate,
    UnreadNotifications,
    UnreadNotificationsUpdate,
};
