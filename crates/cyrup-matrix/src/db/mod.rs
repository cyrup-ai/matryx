//! # SurrealDB Client
//!
//! A configurable SurrealDB client library that supports multiple storage engines
//! and provides a convenient Data Access Object (DAO) pattern implementation.
//!
//! ## Features
//!
//! - Support for multiple storage engines: Memory, Local KV, TiKV, WebSocket
//! - Configuration-based engine selection
//! - Generic DAO pattern implementation
//! - Convenient error handling
//! - Transaction support
//! - Metrics collection

pub mod client;
pub mod config;
pub mod dao;
pub mod error;
pub mod metric;
pub mod migration;
pub mod db;
pub mod entity;
pub mod generic_dao;

// Generic DAO and Entity
pub use generic_dao::{Dao, Entity};

// Re-export main components
pub use client::{connect_database, DatabaseClient};
pub use config::{DbConfig, StorageEngine};
pub use dao::*;
pub use error::{Error, ErrorContext, Result};

// Export migration functionality
pub use migration::{get_hardcoded_migration, run_migration, run_migration_from_directory};

// Export entity types
pub use entity::*;

// Export common SurrealDB types for convenience
pub use surrealdb::sql::{Array, Id, Object, Thing, Value};
pub use surrealdb::Surreal;