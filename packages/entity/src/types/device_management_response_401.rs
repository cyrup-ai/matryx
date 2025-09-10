use crate::types::{AuthenticationParameters, FlowInformation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Device management response 401
/// Source: spec/client/04_security_md:163-169
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceManagementResponse401 {
    pub completed: Vec<String>,
    pub flows: Vec<FlowInformation>,
    pub params: HashMap<String, AuthenticationParameters>,
    pub session: String,
}

impl DeviceManagementResponse401 {
    pub fn new(
        completed: Vec<String>,
        flows: Vec<FlowInformation>,
        params: HashMap<String, AuthenticationParameters>,
        session: String,
    ) -> Self {
        Self { completed, flows, params, session }
    }
}
