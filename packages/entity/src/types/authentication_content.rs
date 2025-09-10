use serde::{Deserialize, Serialize};

/// Authentication request content for Matrix user-interactive authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AuthenticationContent {
    /// Password authentication (m.login.password)
    #[serde(rename = "m.login.password")]
    Password {
        /// User identifier (user ID or localpart)
        #[serde(skip_serializing_if = "Option::is_none")]
        identifier: Option<UserIdentifier>,
        /// User password
        password: String,
        /// Session ID for multi-stage auth
        #[serde(skip_serializing_if = "Option::is_none")]
        session: Option<String>,
    },

    /// ReCAPTCHA authentication (m.login.recaptcha)
    #[serde(rename = "m.login.recaptcha")]
    Recaptcha {
        /// ReCAPTCHA response token
        response: String,
        /// Session ID for multi-stage auth
        #[serde(skip_serializing_if = "Option::is_none")]
        session: Option<String>,
    },

    /// Token-based authentication (m.login.token)
    #[serde(rename = "m.login.token")]
    Token {
        /// Login token
        token: String,
        /// Session ID for multi-stage auth
        #[serde(skip_serializing_if = "Option::is_none")]
        session: Option<String>,
    },

    /// Dummy authentication (m.login.dummy) - always succeeds
    #[serde(rename = "m.login.dummy")]
    Dummy {
        /// Session ID for multi-stage auth
        #[serde(skip_serializing_if = "Option::is_none")]
        session: Option<String>,
    },
}

/// User identifier for authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum UserIdentifier {
    /// Matrix user ID
    #[serde(rename = "m.id.user")]
    User { user: String },

    /// Third-party identifier (email, phone, etc.)
    #[serde(rename = "m.id.thirdparty")]
    ThirdParty { medium: String, address: String },

    /// Phone number identifier
    #[serde(rename = "m.id.phone")]
    Phone { country: String, phone: String },
}

impl AuthenticationContent {
    /// Create password authentication content
    pub fn password(password: String, session: Option<String>) -> Self {
        Self::Password { identifier: None, password, session }
    }

    /// Create dummy authentication content
    pub fn dummy(session: Option<String>) -> Self {
        Self::Dummy { session }
    }
}
