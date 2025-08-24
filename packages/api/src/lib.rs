//! A wrapper around matrix-sdk providing synchronous interfaces with hidden async complexity.
//!
//! This crate avoids using async_trait and returning Box<dyn Future> in favor of
//! synchronous interfaces that hide all the async complexity behind channels and
//! task spawning mechanisms.

// Core modules
pub mod commands;
pub mod db;

// The following modules need updates to work with Matrix SDK 0.10.0
// and will be enabled progressively as implementation is fixed
#[allow(dead_code)]
mod client;
#[allow(dead_code)]
mod encryption;
#[allow(dead_code)]
mod error;
pub mod future;
#[allow(dead_code)]
mod media;
#[allow(dead_code)]
mod member;
#[allow(dead_code)]
mod notifications;
#[allow(dead_code)]
mod room;
#[allow(dead_code)]
mod store;
#[allow(dead_code)]
mod sync;

