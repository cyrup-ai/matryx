//! Storage abstractions for Matrix data
//!
//! This module provides a clean, synchronous interface to Matrix SDK's StateStore trait
//! that hides the complexity of async/await.

pub mod cyrum_state_store;
pub mod surreal_state_store;

pub use cyrum_state_store::CyrumStateStore;
// Re-export the SurrealStateStore for public use
pub use self::surreal_state_store::SurrealStateStore;