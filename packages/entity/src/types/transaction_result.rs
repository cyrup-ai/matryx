use serde::{Deserialize, Serialize};

/// Result of a transaction operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResult {
    /// Transaction ID
    pub txn_id: String,

    /// Success status
    pub success: bool,

    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Additional result data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl TransactionResult {
    pub fn success(txn_id: String) -> Self {
        Self { txn_id, success: true, error: None, data: None }
    }

    pub fn failure(txn_id: String, error: String) -> Self {
        Self {
            txn_id,
            success: false,
            error: Some(error),
            data: None,
        }
    }
}
