use crate::repository::error::RepositoryError;
use matryx_entity::types::{Report, ReportStatus};
use serde_json::Value;
use surrealdb::{Connection, Surreal};
use uuid::Uuid;

/// Parameters for creating a user report
pub struct CreateReportParams<'a> {
    pub reporter_id: &'a str,
    pub reported_user_id: &'a str,
    pub reason: &'a str,
    pub content: Option<Value>,
}

/// Repository dependencies for report operations
pub struct ReportRepositories<'a> {
    pub membership_repo: &'a crate::repository::membership::MembershipRepository,
    pub room_repo: &'a crate::repository::room::RoomRepository,
    pub event_repo: &'a crate::repository::event::EventRepository,
}

/// Parameters for validating report authorization
pub struct ValidateReportParams<'a> {
    pub reporter_id: &'a str,
    pub reported_user_id: &'a str,
    pub room_id: Option<&'a str>,
    pub event_id: Option<&'a str>,
    pub reason: &'a str,
}

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
        params: CreateReportParams<'_>,
        repos: ReportRepositories<'_>,
    ) -> Result<Report, RepositoryError> {
        // Validate parameters
        if params.reason.is_empty() {
            return Err(RepositoryError::Validation {
                field: "reason".to_string(),
                message: "Report reason cannot be empty".to_string(),
            });
        }

        if params.reporter_id == params.reported_user_id {
            return Err(RepositoryError::Validation {
                field: "reported_user_id".to_string(),
                message: "User cannot report themselves".to_string(),
            });
        }

        // Extract room_id and event_id from content parameter if available
        let report_room_id = params.content.as_ref()
            .and_then(|c| c.get("room_id"))
            .and_then(|v| v.as_str());
        let report_event_id = params.content.as_ref()
            .and_then(|c| c.get("event_id"))
            .and_then(|v| v.as_str());

        // Validate report authorization
        let validate_params = ValidateReportParams {
            reporter_id: params.reporter_id,
            reported_user_id: params.reported_user_id,
            room_id: report_room_id,
            event_id: report_event_id,
            reason: params.reason,
        };
        
        if !self.validate_report_authorization(validate_params, repos).await? {
            return Err(RepositoryError::Unauthorized {
                reason: "User does not have permission to report this content".to_string(),
            });
        }

        // Check for existing active report
        if self.check_existing_report(params.reporter_id, params.reported_user_id).await? {
            return Err(RepositoryError::Conflict {
                message: "An active report already exists for this user".to_string(),
            });
        }

        let report_id = Uuid::new_v4().to_string();
        let report = Report::new(
            report_id.clone(),
            params.reporter_id.to_string(),
            params.reported_user_id.to_string(),
            params.reason.to_string(),
            params.content,
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

    /// Validate report authorization
    pub async fn validate_report_authorization(
        &self,
        params: ValidateReportParams<'_>,
        repos: ReportRepositories<'_>,
    ) -> Result<bool, RepositoryError> {
        // Check if reporter exists and is active
        let reporter_query = "SELECT is_active FROM user WHERE user_id = $reporter_id LIMIT 1";
        let mut result = self
            .db
            .query(reporter_query)
            .bind(("reporter_id", params.reporter_id.to_string()))
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
            .bind(("reported_user_id", params.reported_user_id.to_string()))
            .await?;

        let reported_rows: Vec<serde_json::Value> = result.take(0)?;
        if reported_rows.is_empty() {
            return Ok(false);
        }

        // Validate reason length
        if params.reason.is_empty() {
            return Err(RepositoryError::Validation {
                field: "reason".to_string(),
                message: "Report reason is required".to_string(),
            });
        }

        if params.reason.len() > 256 {
            return Err(RepositoryError::Validation {
                field: "reason".to_string(),
                message: "Report reason too long (max 256 chars)".to_string(),
            });
        }

        // Validate event existence and room visibility for event reports
        if let Some(event_id_val) = params.event_id {
            // Verify event exists
            let event = repos.event_repo.get_event_by_id(event_id_val).await?;
            if event.is_none() {
                return Err(RepositoryError::NotFound {
                    entity_type: "event".to_string(),
                    id: event_id_val.to_string(),
                });
            }

            // Verify reporter can see the content (if room_id provided)
            if let Some(room_id_val) = params.room_id {
                let membership = repos.membership_repo
                    .get_membership(room_id_val, params.reporter_id)
                    .await?;

                if membership.is_none() {
                    // Check if room is world_readable
                    let visibility = repos.room_repo.get_room_history_visibility(room_id_val).await?;
                    if visibility != "world_readable" {
                        return Err(RepositoryError::Forbidden {
                            reason: "Cannot report content in room you cannot access".to_string(),
                        });
                    }
                }
            }
        }

        // Check rate limiting (max 10 reports per hour per user)
        let rate_limit_query = "
            SELECT count() as report_count FROM user_reports
            WHERE reporter_id = $reporter_id
            AND created_at > time::now() - 1h
            GROUP ALL
        ";

        let mut result = self
            .db
            .query(rate_limit_query)
            .bind(("reporter_id", params.reporter_id.to_string()))
            .await?;

        let rate_limit_rows: Vec<serde_json::Value> = result.take(0)?;
        if let Some(row) = rate_limit_rows.first()
            && let Some(count) = row.get("report_count").and_then(|v| v.as_i64())
            && count >= 10
        {
            return Err(RepositoryError::Forbidden {
                reason: "Too many reports, please try again later".to_string(),
            });
        }

        Ok(true)
    }
}
