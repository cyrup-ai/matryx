use serde::{Deserialize, Serialize};

/// VerificationRequestInRoom
/// Source: spec/client/04_security_md:763-770
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationRequestInRoom {
    pub body: String,
    pub format: Option<String>,
    pub formatted_body: Option<String>,
    pub from_device: String,
    pub methods: Vec<String>,
    pub msgtype: String,
    pub to: String,
}

impl VerificationRequestInRoom {
    pub fn new(
        body: String,
        format: Option<String>,
        formatted_body: Option<String>,
        from_device: String,
        methods: Vec<String>,
        msgtype: String,
        to: String,
    ) -> Self {
        Self {
            body,
            format,
            formatted_body,
            from_device,
            methods,
            msgtype,
            to,
        }
    }
}
