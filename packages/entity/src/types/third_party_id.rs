use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Third-party identifier medium types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ThirdPartyMedium {
    /// Email address
    #[serde(rename = "email")]
    Email,
    /// Phone number (MSISDN)
    #[serde(rename = "msisdn")]
    PhoneNumber,
}

impl ThirdPartyMedium {
    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            ThirdPartyMedium::Email => "email",
            ThirdPartyMedium::PhoneNumber => "msisdn",
        }
    }
}

impl FromStr for ThirdPartyMedium {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "email" => Ok(ThirdPartyMedium::Email),
            "msisdn" => Ok(ThirdPartyMedium::PhoneNumber),
            _ => Err(()),
        }
    }
}

/// Third-party identifier entity
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThirdPartyId {
    /// Unique identifier for this third-party ID
    pub id: String,

    /// User ID this third-party identifier belongs to
    pub user_id: String,

    /// Medium type (email, msisdn, etc.)
    pub medium: String,

    /// The actual address (email address, phone number, etc.)
    pub address: String,

    /// Whether this identifier has been validated
    pub validated: bool,

    /// Validation token (for pending validations)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_token: Option<String>,

    /// Token expiration time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_expires_at: Option<DateTime<Utc>>,

    /// When this identifier was added
    pub created_at: DateTime<Utc>,

    /// When this identifier was validated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validated_at: Option<DateTime<Utc>>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl ThirdPartyId {
    /// Create a new third-party identifier
    pub fn new(
        id: String,
        user_id: String,
        medium: String,
        address: String,
        validated: bool,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            user_id,
            medium,
            address,
            validated,
            validation_token: None,
            token_expires_at: None,
            created_at: now,
            validated_at: if validated { Some(now) } else { None },
            updated_at: now,
        }
    }

    /// Create with validation token
    pub fn with_validation_token(
        id: String,
        user_id: String,
        medium: String,
        address: String,
        validation_token: String,
        token_expires_at: DateTime<Utc>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            user_id,
            medium,
            address,
            validated: false,
            validation_token: Some(validation_token),
            token_expires_at: Some(token_expires_at),
            created_at: now,
            validated_at: None,
            updated_at: now,
        }
    }

    /// Mark as validated
    pub fn validate(&mut self) {
        self.validated = true;
        self.validated_at = Some(Utc::now());
        self.updated_at = Utc::now();
        self.validation_token = None;
        self.token_expires_at = None;
    }

    /// Check if validation token is expired
    pub fn is_token_expired(&self) -> bool {
        if let Some(expires_at) = self.token_expires_at {
            return Utc::now() > expires_at;
        }
        false
    }

    /// Check if this identifier is for email
    pub fn is_email(&self) -> bool {
        self.medium == "email"
    }

    /// Check if this identifier is for phone number
    pub fn is_phone_number(&self) -> bool {
        self.medium == "msisdn"
    }
}
