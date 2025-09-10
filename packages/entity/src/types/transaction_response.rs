use crate::types::TransactionResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// TransactionResponse
/// Source: spec/server/05-md:51-59
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResponse {
    pub pdus: HashMap<String, TransactionResult>,
}

impl TransactionResponse {
    pub fn new(pdus: HashMap<String, TransactionResult>) -> Self {
        Self { pdus }
    }
}
