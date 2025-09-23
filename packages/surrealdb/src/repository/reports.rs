use crate::repository::error::RepositoryError;
use matryx_entity::types::{Report, ReportStatus};
use serde_json::Value;
use surrealdb::{Connection, Surreal};
use uuid::Uuid;

#[derive(Clone)]
pub struct ReportsRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> ReportsRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    /// Create a new user report
    pub async fn create_user_report(
        &self,
        reporter_id: &str,
        reported_user_id: &str,
        reason: &str,
        content: Option<Value>,
    ) -> Result<Report, RepositoryError> {
        // Validate parameters
        if reason.is_empty() {
            return Err(RepositoryError::Validation {
                field: "reason".to_string(),
                message: "Report reason cannot be empty".to_string(),
            });
        }

        if reporter_id == reported_user_id {
            return Err(RepositoryError::Validation {
                field: "reported_user_id".to_string(),
                message: "User cannot report themselves".to_string(),
            });
        }

        // Validate report permissions
        if !self.validate_report_permissions(reporter_id, reported_user_id).await? {
            return Err(RepositoryError::Unauthorized {
                reason: "User does not have permission to report this user".to_string(),
            });
        }

        // Check for existing active report
        if self.check_existing_report(reporter_id, reported_user_id).await? {
            return Err(RepositoryError::Conflict {
                message: "An active report already exists for this user".to_string(),
            });
        }

        let report_id = Uuid::new_v4().to_string();
        let report = Report::new(
            report_id.clone(),
            reporter_id.to_string(),
            reported_user_id.to_string(),
            reason.to_string(),
            content,
        );

        let report_content = report.clone();
        let created: Option<Report> = self
            .db
            .create(("user_reports", &report_id))
            .content(report_content)
            .await?;

        created.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to create user report"))
        })
    }
    /// Get a report by ID
    pub async fn get_report(&self, report_id: &str) -> Result<Option<Report>, RepositoryError> {
        let report: Option<Report> = self.db.select(("user_reports", report_id)).await?;
        Ok(report)
    }

    /// Get all reports for a reported user
    pub async fn get_user_reports(
        &self,
        reported_user_id: &str,
    ) -> Result<Vec<Report>, RepositoryError> {
        let query = "SELECT * FROM user_reports WHERE reported_user_id = $reported_user_id ORDER BY created_at DESC";
        let mut result = self
            .db
            .query(query)
            .bind(("reported_user_id", reported_user_id.to_string()))
            .await?;

        let reports: Vec<Report> = result.take(0)?;
        Ok(reports)
    }

    /// Get all reports submitted by a reporter
    pub async fn get_reports_by_reporter(
        &self,
        reporter_id: &str,
    ) -> Result<Vec<Report>, RepositoryError> {
        let query =
            "SELECT * FROM user_reports WHERE reporter_id = $reporter_id ORDER BY created_at DESC";
        let mut result =
            self.db.query(query).bind(("reporter_id", reporter_id.to_string())).await?;

        let reports: Vec<Report> = result.take(0)?;
        Ok(reports)
    }

    /// Update report status
    pub async fn update_report_status(
        &self,
        report_id: &str,
        status: ReportStatus,
        admin_notes: Option<String>,
    ) -> Result<(), RepositoryError> {
        // Check if report exists
        let existing_report = self.get_report(report_id).await?.ok_or_else(|| {
            RepositoryError::NotFound {
                entity_type: "Report".to_string(),
                id: report_id.to_string(),
            }
        })?;

        // Check if report can be updated
        if !existing_report.can_be_updated() {
            return Err(RepositoryError::Validation {
                field: "status".to_string(),
                message: "Report cannot be updated - already resolved or dismissed".to_string(),
            });
        }

        let query = r#"
            UPDATE user_reports SET
                status = $status,
                admin_notes = $admin_notes,
                updated_at = time::now()
            WHERE id = $report_id
        "#;

        let mut result = self
            .db
            .query(query)
            .bind(("report_id", report_id.to_string()))
            .bind(("status", status.as_str().to_string()))
            .bind(("admin_notes", admin_notes))
            .await?;

        let updated: Vec<Report> = result.take(0)?;
        if updated.is_empty() {
            return Err(RepositoryError::NotFound {
                entity_type: "Report".to_string(),
                id: report_id.to_string(),
            });
        }

        Ok(())
    }

    /// Check if an active report exists between reporter and reported user
    pub async fn check_existing_report(
        &self,
        reporter_id: &str,
        reported_user_id: &str,
    ) -> Result<bool, RepositoryError> {
        let query = "
            SELECT count() FROM user_reports 
            WHERE reporter_id = $reporter_id 
            AND reported_user_id = $reported_user_id 
            AND status IN ['pending', 'investigating', 'escalated']
            GROUP ALL
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("reporter_id", reporter_id.to_string()))
            .bind(("reported_user_id", reported_user_id.to_string()))
            .await?;

        let count: Option<i64> = result.take(0)?;
        Ok(count.unwrap_or(0) > 0)
    }

    /// Validate report permissions
    pub async fn validate_report_permissions(
        &self,
        reporter_id: &str,
        reported_user_id: &str,
    ) -> Result<bool, RepositoryError> {
        // Check if reporter exists and is active
        let reporter_query = "SELECT is_active FROM user WHERE user_id = $reporter_id LIMIT 1";
        let mut result = self
            .db
            .query(reporter_query)
            .bind(("reporter_id", reporter_id.to_string()))
            .await?;

        let reporter_rows: Vec<serde_json::Value> = result.take(0)?;
        if let Some(reporter_row) = reporter_rows.first() {
            if let Some(is_active) = reporter_row.get("is_active").and_then(|v| v.as_bool()) {
                if !is_active {
                    return Ok(false);
                }
            } else {
                return Ok(false);
            }
        } else {
            return Ok(false);
        }

        // Check if reported user exists
        let reported_query = "SELECT user_id FROM user WHERE user_id = $reported_user_id LIMIT 1";
        let mut result = self
            .db
            .query(reported_query)
            .bind(("reported_user_id", reported_user_id.to_string()))
            .await?;

        let reported_rows: Vec<serde_json::Value> = result.take(0)?;
        if reported_rows.is_empty() {
            return Ok(false);
        }

        // Additional validation: check if users have interacted (optional)
        // For now, allow any active user to report any existing user
        Ok(true)
    }
}
