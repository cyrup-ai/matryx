use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str::FromStr;

/// User report status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum ReportStatus {
    /// Report is pending review
    #[default]
    Pending,
    /// Report is under investigation
    Investigating,
    /// Report has been resolved
    Resolved,
    /// Report was dismissed
    Dismissed,
    /// Report was escalated
    Escalated,
}

impl ReportStatus {
    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            ReportStatus::Pending => "pending",
            ReportStatus::Investigating => "investigating",
            ReportStatus::Resolved => "resolved",
            ReportStatus::Dismissed => "dismissed",
            ReportStatus::Escalated => "escalated",
        }
    }
}

impl FromStr for ReportStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(ReportStatus::Pending),
            "investigating" => Ok(ReportStatus::Investigating),
            "resolved" => Ok(ReportStatus::Resolved),
            "dismissed" => Ok(ReportStatus::Dismissed),
            "escalated" => Ok(ReportStatus::Escalated),
            _ => Err(()),
        }
    }
}

/// User report entity
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// Unique report ID
    pub id: String,

    /// User ID of the reporter
    pub reporter_id: String,

    /// User ID of the reported user
    pub reported_user_id: String,

    /// Reason for the report
    pub reason: String,

    /// Additional content/details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Value>,

    /// Report status
    pub status: ReportStatus,

    /// Admin notes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admin_notes: Option<String>,

    /// Report creation timestamp
    pub created_at: DateTime<Utc>,

    /// Report last update timestamp
    pub updated_at: DateTime<Utc>,

    /// User who last updated the report
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_by: Option<String>,
}

impl Default for Report {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            id: String::new(),
            reporter_id: String::new(),
            reported_user_id: String::new(),
            reason: String::new(),
            content: None,
            status: ReportStatus::Pending,
            admin_notes: None,
            created_at: now,
            updated_at: now,
            updated_by: None,
        }
    }
}

impl Report {
    /// Create a new user report
    pub fn new(
        id: String,
        reporter_id: String,
        reported_user_id: String,
        reason: String,
        content: Option<Value>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            reporter_id,
            reported_user_id,
            reason,
            content,
            status: ReportStatus::Pending,
            admin_notes: None,
            created_at: now,
            updated_at: now,
            updated_by: None,
        }
    }

    /// Update report status
    pub fn update_status(
        &mut self,
        status: ReportStatus,
        admin_notes: Option<String>,
        updated_by: Option<String>,
    ) {
        self.status = status;
        self.admin_notes = admin_notes;
        self.updated_at = Utc::now();
        self.updated_by = updated_by;
    }

    /// Check if report is active (not resolved or dismissed)
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            ReportStatus::Pending | ReportStatus::Investigating | ReportStatus::Escalated
        )
    }

    /// Check if report can be updated
    pub fn can_be_updated(&self) -> bool {
        !matches!(self.status, ReportStatus::Resolved | ReportStatus::Dismissed)
    }
}
