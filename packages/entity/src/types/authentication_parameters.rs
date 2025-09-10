use serde::{Deserialize, Serialize};

/// Authentication flow parameters for Matrix user-interactive authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AuthenticationParameters {
    /// ReCAPTCHA parameters
    Recaptcha {
        /// ReCAPTCHA public key
        public_key: String,
    },

    /// Terms of service parameters
    Terms {
        /// Available policies
        policies: std::collections::HashMap<String, TermsPolicy>,
    },

    /// Email identity server parameters
    EmailIdentity {
        /// Identity server URL
        #[serde(skip_serializing_if = "Option::is_none")]
        id_server: Option<String>,
        /// Identity access token
        #[serde(skip_serializing_if = "Option::is_none")]
        id_access_token: Option<String>,
    },

    /// Generic parameters for unknown auth types
    Generic(std::collections::HashMap<String, serde_json::Value>),
}

/// Terms of service policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TermsPolicy {
    /// Policy name
    pub name: String,
    /// Policy URL
    pub url: String,
    /// Policy version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

impl AuthenticationParameters {
    pub fn recaptcha(public_key: String) -> Self {
        Self::Recaptcha { public_key }
    }

    pub fn generic(params: std::collections::HashMap<String, serde_json::Value>) -> Self {
        Self::Generic(params)
    }
}
