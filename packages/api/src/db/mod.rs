//! SurrealDB client implementation for cyrup
//!
//! Features:
//! - Support for multiple storage engines (SurrealKV)
//! - Configuration-based engine selection
//! - Generic DAO pattern implementation
//! - Error handling with context
//! - Transaction support
//! - Metrics collection
//! - Live query support

// Modules
pub mod client;
pub mod config;
pub mod dao;
pub mod db;
pub mod entity;
pub mod error;
pub mod generic_dao;
pub mod metric;
pub mod migration;

// Re-export main components
pub use client::{
    connect_database,
    DatabaseClient,
    LiveAction,
    LiveQueryStream,
    MultiQueryStream,
    OptionalQueryStream,
    QueryStream,
    TransactionManager,
    TransactionStream,
};
pub use config::{DbConfig, StorageEngine};
pub use error::{Error, Result};
pub use generic_dao::{BaseDao, Dao, Entity};

// Re-export DAO types
pub use dao::account_data::AccountDataDao;
pub use dao::api_cache::ApiCacheDao;
pub use dao::custom::CustomDao;
pub use dao::key_value::KeyValueDao;
pub use dao::media_upload::MediaUploadDao;
pub use dao::message::MessageDao;
pub use dao::presence::PresenceDao;
pub use dao::receipt::ReceiptDao;
pub use dao::request_dependency::RequestDependencyDao;
pub use dao::room_membership::RoomMembershipDao;
pub use dao::room_state::RoomStateDao;
pub use dao::send_queue::SendQueueDao;

// Re-export entity types
pub use entity::account_data::AccountDataEntity;
pub use entity::api_cache::ApiCacheEntry;
pub use entity::custom_value::CustomValue;
pub use entity::key_value::KeyValue;
pub use entity::media_upload::MediaUpload;
pub use entity::message::Message;
pub use entity::presence::PresenceData;
pub use entity::receipt::Receipt;
pub use entity::request_dependency::RequestDependency;
pub use entity::room_membership::RoomMembership;
pub use entity::room_state::RoomState;
pub use entity::send_queue::SendQueueEntry;
