use crate::repository::error::RepositoryError;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::{Connection, Surreal};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptchaChallenge {
    pub challenge_id: String,
    pub challenge_type: String, // "recaptcha", "hcaptcha", "custom", "math"
    pub site_key: Option<String>,
    pub question: Option<String>, // For math challenges
    pub answer: Option<String>,   // For math challenges
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub used: bool,
    pub verified: bool,
    pub user_id: Option<String>,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
    pub session_id: Option<String>,
}

/// CAPTCHA statistics for monitoring
#[derive(Debug, Serialize, Deserialize)]
pub struct CaptchaStats {
    pub total_challenges: u64,
    pub used_count: u64,
    pub verified_count: u64,
    pub expired_count: u64,
}

/// Rate limit violation record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitViolation {
    pub id: String,
    pub ip_address: String,
    pub operation: String,
    pub created_at: DateTime<Utc>,
}

pub struct CaptchaRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> CaptchaRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    /// Create a new CAPTCHA challenge
    pub async fn create_challenge(
        &self,
        challenge: &CaptchaChallenge,
    ) -> Result<CaptchaChallenge, RepositoryError> {
        let challenge_clone = challenge.clone();
        let created: Option<CaptchaChallenge> = self
            .db
            .create(("captcha_challenges", &challenge.challenge_id))
            .content(challenge_clone)
            .await?;

        created.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to create CAPTCHA challenge"))
        })
    }

    /// Get CAPTCHA challenge by ID
    pub async fn get_challenge(
        &self,
        challenge_id: &str,
    ) -> Result<Option<CaptchaChallenge>, RepositoryError> {
        let challenge: Option<CaptchaChallenge> =
            self.db.select(("captcha_challenges", challenge_id)).await?;
        Ok(challenge)
    }

    /// Update CAPTCHA challenge
    pub async fn update_challenge(
        &self,
        challenge: &CaptchaChallenge,
    ) -> Result<(), RepositoryError> {
        let _: Option<CaptchaChallenge> = self
            .db
            .update(("captcha_challenges", &challenge.challenge_id))
            .content(challenge.clone())
            .await?;
        Ok(())
    }

    /// Cleanup expired challenges
    pub async fn cleanup_expired_challenges(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<u64, RepositoryError> {
        let query = "
            DELETE captcha_challenges 
            WHERE expires_at < $cutoff OR (used = true AND verified = true)
        ";

        let mut result = self.db.query(query).bind(("cutoff", cutoff)).await?;

        let deleted_count: Option<u64> = result.take(0).unwrap_or(Some(0));
        Ok(deleted_count.unwrap_or(0))
    }

    /// Get the count of challenges created for an IP address within a time window
    pub async fn get_ip_challenge_count(
        &self,
        ip_address: &str,
        window: Duration,
    ) -> Result<u64, RepositoryError> {
        let query = "
            SELECT count() as challenge_count
            FROM captcha_challenges
            WHERE client_ip = $ip_address 
              AND created_at > datetime::sub(datetime::now(), $window)
        ";

        let mut response = self
            .db
            .query(query)
            .bind(("ip_address", ip_address.to_string()))
            .bind(("window", format!("{}h", window.num_hours())))
            .await?;

        #[derive(serde::Deserialize)]
        struct ChallengeCount {
            challenge_count: u64,
        }

        let count_record: Option<ChallengeCount> = response.take(0)?;
        let challenge_count = count_record.map(|r| r.challenge_count).unwrap_or(0);

        Ok(challenge_count)
    }

    /// Check for suspicious activity from IP address
    pub async fn check_suspicious_activity(
        &self,
        ip_address: &str,
        operation: &str,
    ) -> Result<bool, RepositoryError> {
        let query = "
            SELECT count() as attempt_count
            FROM rate_limit_violations 
            WHERE ip_address = $ip_address 
              AND operation = $operation 
              AND created_at > datetime::sub(datetime::now(), duration('1h'))
        ";

        let mut response = self
            .db
            .query(query)
            .bind(("ip_address", ip_address.to_string()))
            .bind(("operation", operation.to_string()))
            .await?;

        #[derive(serde::Deserialize)]
        struct AttemptCount {
            attempt_count: u64,
        }

        let attempt_record: Option<AttemptCount> = response.take(0)?;
        let attempt_count = attempt_record.map(|r| r.attempt_count).unwrap_or(0);

        // Require CAPTCHA if more than 3 failed attempts in the last hour
        Ok(attempt_count > 3)
    }

    /// Record rate limit violation for suspicious activity tracking
    pub async fn record_rate_limit_violation(
        &self,
        ip_address: &str,
        operation: &str,
    ) -> Result<(), RepositoryError> {
        let violation_id = format!("violation_{}", uuid::Uuid::new_v4());

        let violation = RateLimitViolation {
            id: violation_id.clone(),
            ip_address: ip_address.to_string(),
            operation: operation.to_string(),
            created_at: Utc::now(),
        };

        let _: Option<RateLimitViolation> = self
            .db
            .create(("rate_limit_violations", violation_id))
            .content(violation)
            .await?;

        Ok(())
    }

    /// Get CAPTCHA statistics for monitoring
    pub async fn get_captcha_stats(&self) -> Result<CaptchaStats, RepositoryError> {
        let query = "
            SELECT 
                count() as total_challenges,
                count(used = true) as used_count,
                count(verified = true) as verified_count,
                count(expires_at < datetime::now()) as expired_count
            FROM captcha_challenges
            WHERE created_at > datetime::sub(datetime::now(), duration('24h'))
        ";

        let mut response = self.db.query(query).await?;
        let stats: Option<CaptchaStats> = response.take(0)?;
        stats.ok_or_else(|| {
            RepositoryError::NotFound {
                entity_type: "CaptchaStats".to_string(),
                id: "default".to_string(),
            }
        })
    }

    /// Validate CAPTCHA response
    pub async fn validate_response(
        &self,
        challenge_id: &str,
        response: &str,
    ) -> Result<bool, RepositoryError> {
        let challenge: Option<CaptchaChallenge> =
            self.db.select(("captcha_challenges", challenge_id)).await?;

        if let Some(mut challenge) = challenge {
            if challenge.used || chrono::Utc::now() > challenge.expires_at {
                return Ok(false);
            }

            let is_valid = challenge.answer.as_ref().is_some_and(|ans| ans == response);

            if is_valid {
                challenge.verified = true;
                challenge.used = true;
                let _: Option<CaptchaChallenge> = self
                    .db
                    .update(("captcha_challenges", challenge_id))
                    .content(challenge)
                    .await?;
            }

            Ok(is_valid)
        } else {
            Ok(false)
        }
    }

    /// Create a math challenge
    pub async fn create_math_challenge(
        &self,
        user_id: Option<String>,
        client_ip: Option<String>,
        user_agent: Option<String>,
    ) -> Result<CaptchaChallenge, RepositoryError> {
        use rand::Rng;

        let mut rng = rand::rng();
        let a = rng.random_range(1..=20);
        let b = rng.random_range(1..=20);
        let answer = (a + b).to_string();
        let question = format!("What is {} + {}?", a, b);

        let challenge_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let expires_at = now + chrono::Duration::minutes(10);

        let challenge = CaptchaChallenge {
            challenge_id: challenge_id.clone(),
            challenge_type: "math".to_string(),
            site_key: None,
            question: Some(question),
            answer: Some(answer),
            user_id,
            client_ip,
            user_agent,
            session_id: None,
            created_at: now,
            expires_at,
            used: false,
            verified: false,
        };

        let created: Option<CaptchaChallenge> = self
            .db
            .create(("captcha_challenges", &challenge_id))
            .content(challenge.clone())
            .await?;
        created.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to create CAPTCHA challenge"))
        })
    }
}
