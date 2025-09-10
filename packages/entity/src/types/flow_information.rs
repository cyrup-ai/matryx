use crate::types::AuthenticationParameters;
use serde::{Deserialize, Serialize};

/// Flow information for interactive authentication flows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowInformation {
    /// The authentication flow type
    #[serde(rename = "type")]
    pub flow_type: String,

    /// Additional parameters for the flow
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<AuthenticationParameters>,
}

impl FlowInformation {
    pub fn new(flow_type: String) -> Self {
        Self { flow_type, params: None }
    }
}
