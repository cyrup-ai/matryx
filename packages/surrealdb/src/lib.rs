#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
// Allow unwrap/expect in test code for convenience
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod pagination;
pub mod repository;
pub mod test_utils;

pub use repository::*;
