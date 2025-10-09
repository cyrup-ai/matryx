//! Support contact configuration for Matrix homeserver
//!
//! Provides configuration structure for support contacts, support pages,
//! and help information that can be exposed via the /.well-known/matrix/support endpoint.

use serde::{Deserialize, Serialize};
use std::env;
use tracing::info;

/// Support contact information for the Matrix homeserver
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportContact {
    /// Matrix ID of the support contact
    pub matrix_id: String,
    /// Email address for support contact
    pub email_address: String,
    /// Role of the support contact (e.g., "administrator", "moderator", "support")
    pub role: String,
}

/// Support page configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportPageConfig {
    /// URL to the support page
    pub url: String,
    /// Whether the support page is enabled
    pub enabled: bool,
}

/// Complete support configuration for the homeserver
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportConfig {
    /// List of support contacts
    pub contacts: Vec<SupportContact>,
    /// Support page configuration
    pub support_page: SupportPageConfig,
    /// Whether support information is publicly available
    pub public_support_enabled: bool,
}

impl Default for SupportConfig {
    fn default() -> Self {
        Self {
            contacts: Vec::new(),
            support_page: SupportPageConfig { url: String::new(), enabled: false },
            public_support_enabled: true,
        }
    }
}

impl SupportConfig {
    /// Create a new SupportConfig from environment variables and server configuration
    pub fn from_env(homeserver_name: &str, admin_email: &str, use_https: bool) -> Self {
        let mut contacts = Vec::new();

        // Add primary administrator contact
        let admin_matrix_id = format!("@admin:{}", homeserver_name);
        contacts.push(SupportContact {
            matrix_id: admin_matrix_id,
            email_address: admin_email.to_string(),
            role: "administrator".to_string(),
        });

        // Add additional support contacts from environment if configured
        if let Ok(support_emails) = env::var("MATRIX_SUPPORT_EMAILS") {
            for (index, email) in support_emails.split(',').enumerate() {
                let email = email.trim();
                if !email.is_empty() && email != admin_email {
                    let support_matrix_id = format!(
                        "@support{}:{}",
                        if index == 0 {
                            String::new()
                        } else {
                            index.to_string()
                        },
                        homeserver_name
                    );
                    contacts.push(SupportContact {
                        matrix_id: support_matrix_id,
                        email_address: email.to_string(),
                        role: "support".to_string(),
                    });
                }
            }
        }

        // Configure support page
        let protocol = if use_https { "https" } else { "http" };
        let support_page_url = env::var("MATRIX_SUPPORT_PAGE")
            .unwrap_or_else(|_| format!("{}://{}/support", protocol, homeserver_name));

        let support_page_enabled = env::var("MATRIX_SUPPORT_PAGE_ENABLED")
            .map(|val| val.parse().unwrap_or(true))
            .unwrap_or(true);

        let public_support_enabled = env::var("MATRIX_PUBLIC_SUPPORT_ENABLED")
            .map(|val| val.parse().unwrap_or(true))
            .unwrap_or(true);

        info!("Support configuration initialized with {} contacts", contacts.len());

        Self {
            contacts,
            support_page: SupportPageConfig {
                url: support_page_url,
                enabled: support_page_enabled,
            },
            public_support_enabled,
        }
    }

    /// Get all administrator contacts
    pub fn get_admin_contacts(&self) -> Vec<&SupportContact> {
        self.contacts
            .iter()
            .filter(|contact| contact.role == "administrator")
            .collect()
    }

    /// Get all support contacts (including administrators)
    pub fn get_support_contacts(&self) -> Vec<&SupportContact> {
        self.contacts
            .iter()
            .filter(|contact| contact.role == "administrator" || contact.role == "support")
            .collect()
    }

    /// Get the primary support page URL if enabled
    pub fn get_support_page_url(&self) -> Option<&str> {
        if self.support_page.enabled {
            Some(&self.support_page.url)
        } else {
            None
        }
    }

    /// Validate the support configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.contacts.is_empty() {
            return Err("No support contacts configured".to_string());
        }

        // Validate each contact
        for contact in &self.contacts {
            if contact.matrix_id.is_empty() {
                return Err("Support contact matrix_id cannot be empty".to_string());
            }
            if contact.email_address.is_empty() || !contact.email_address.contains('@') {
                return Err(format!(
                    "Invalid email address for contact: {}",
                    contact.email_address
                ));
            }
            if contact.role.is_empty() {
                return Err("Support contact role cannot be empty".to_string());
            }
        }

        // Ensure at least one administrator
        if self.get_admin_contacts().is_empty() {
            return Err("At least one administrator contact is required".to_string());
        }

        Ok(())
    }
}
