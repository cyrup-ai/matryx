use serde::{Deserialize, Serialize};

/// Content for m.room.message events with msgtype m.server_notice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerNoticeContent {
    /// Message type - must be "m.server_notice"
    pub msgtype: String,
    
    /// The notice message body
    pub body: String,
    
    /// The type of server notice
    pub server_notice_type: String,
    
    /// Additional data specific to the notice type
    #[serde(flatten)]
    pub additional_data: serde_json::Map<String, serde_json::Value>,
}

/// Usage limit reached server notice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageLimitReachedNotice {
    /// Message type - "m.server_notice"
    pub msgtype: String,
    
    /// The notice message body
    pub body: String,
    
    /// Server notice type - "m.server_notice.usage_limit_reached"
    pub server_notice_type: String,
    
    /// The type of limit that was reached
    pub limit_type: String,
    
    /// Optional admin contact information
    pub admin_contact: Option<String>,
}

impl ServerNoticeContent {
    /// Create a new server notice content
    pub fn new(body: String, server_notice_type: String) -> Self {
        Self {
            msgtype: "m.server_notice".to_string(),
            body,
            server_notice_type,
            additional_data: serde_json::Map::new(),
        }
    }
    
    /// Create a usage limit reached notice
    pub fn usage_limit_reached(
        body: String,
        limit_type: String,
        admin_contact: Option<String>,
    ) -> Self {
        let mut additional_data = serde_json::Map::new();
        additional_data.insert("limit_type".to_string(), serde_json::Value::String(limit_type));
        
        if let Some(contact) = admin_contact {
            additional_data.insert("admin_contact".to_string(), serde_json::Value::String(contact));
        }
        
        Self {
            msgtype: "m.server_notice".to_string(),
            body,
            server_notice_type: "m.server_notice.usage_limit_reached".to_string(),
            additional_data,
        }
    }
    
    /// Validate server notice content
    pub fn validate(&self) -> Result<(), String> {
        if self.msgtype != "m.server_notice" {
            return Err("Server notice msgtype must be 'm.server_notice'".to_string());
        }
        
        if self.body.trim().is_empty() {
            return Err("Server notice body cannot be empty".to_string());
        }
        
        if self.server_notice_type.is_empty() {
            return Err("Server notice type cannot be empty".to_string());
        }
        
        Ok(())
    }
}

impl UsageLimitReachedNotice {
    /// Create a new usage limit reached notice
    pub fn new(
        body: String,
        limit_type: String,
        admin_contact: Option<String>,
    ) -> Self {
        Self {
            msgtype: "m.server_notice".to_string(),
            body,
            server_notice_type: "m.server_notice.usage_limit_reached".to_string(),
            limit_type,
            admin_contact,
        }
    }
    
    /// Create a monthly active user limit notice
    pub fn monthly_active_user_limit(admin_contact: Option<String>) -> Self {
        Self::new(
            "The server has exceeded its monthly active user limit. New connections are being refused.".to_string(),
            "monthly_active_user".to_string(),
            admin_contact,
        )
    }
}