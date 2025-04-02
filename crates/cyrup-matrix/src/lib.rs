//! A wrapper around matrix-sdk providing synchronous interfaces with hidden async complexity.
//!
//! This crate avoids using async_trait and returning Box<dyn Future> in favor of
//! synchronous interfaces that hide all the async complexity behind channels and
//! task spawning mechanisms.

pub mod error;
pub mod future;
pub mod store;
pub mod client;
pub mod room;
pub mod member;
pub mod media;
pub mod encryption;
pub mod sync;
pub mod notifications;
pub mod commands;

// Implementation modules
pub mod db;