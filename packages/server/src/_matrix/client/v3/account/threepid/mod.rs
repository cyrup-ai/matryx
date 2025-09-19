use axum::{Json, http::StatusCode};
// Re-export the implementations from the 3pid module
pub use crate::_matrix::client::v3::account::threepid_3pid::{get_threepids as get, add_threepid as post};

pub mod add;
pub mod bind;
pub mod delete;
pub mod email;
pub mod msisdn;
pub mod unbind;
