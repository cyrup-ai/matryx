// Re-export all sync types from the entity package to eliminate duplication
// All these types are now properly defined in packages/entity/src/types/sync.rs

pub use matryx_entity::types::{
    LiveSyncUpdate,
    RoomsUpdate,
    SyncQuery,
};
