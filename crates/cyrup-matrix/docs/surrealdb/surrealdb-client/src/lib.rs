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

mod client;
mod config;
mod dao;
mod error;
mod metrics;
mod migrations;

// Re-export main components
pub use client::{connect_database, DatabaseClient};
pub use config::{DbConfig, StorageEngine};
pub use dao::{BaseDao, Dao, Entity};
pub use error::{Error, ErrorContext, Result};

// Export migration functionality
pub use migrations::{get_hardcoded_migrations, run_migrations, run_migrations_from_directory};

// Export common SurrealDB types for convenience
pub use surrealdb::sql::{Array, Id, Object, Thing, Value};
pub use surrealdb::Surreal;
