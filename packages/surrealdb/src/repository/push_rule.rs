use crate::repository::error::RepositoryError;
use futures::Stream;
use matryx_entity::types::PushRule;
use std::pin::Pin;
use surrealdb::{Connection, Surreal};

pub struct PushRuleRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> PushRuleRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    pub async fn create(
        &self,
        push_rule: &PushRule,
        user_id: &str,
    ) -> Result<PushRule, RepositoryError> {
        let push_rule_clone = push_rule.clone();
        let id = format!("{}:{}", user_id, push_rule.rule_id);
        let created: Option<PushRule> =
            self.db.create(("push_rule", id)).content(push_rule_clone).await?;

        created.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to create push rule"))
        })
    }

    pub async fn get_user_rules(&self, user_id: &str) -> Result<Vec<PushRule>, RepositoryError> {
        let user_id_owned = user_id.to_string();
        let rules: Vec<PushRule> = self.db
            .query("SELECT * FROM push_rule WHERE user_id = $user_id ORDER BY priority_class, priority")
            .bind(("user_id", user_id_owned.to_string()))
            .await?
            .take(0)?;
        Ok(rules)
    }

    pub async fn update(
        &self,
        push_rule: &PushRule,
        user_id: &str,
    ) -> Result<PushRule, RepositoryError> {
        let push_rule_clone = push_rule.clone();
        let id = format!("{}:{}", user_id, push_rule.rule_id);
        let updated: Option<PushRule> =
            self.db.update(("push_rule", id)).content(push_rule_clone).await?;

        updated.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to update push rule"))
        })
    }

    pub async fn delete(&self, user_id: &str, rule_id: &str) -> Result<(), RepositoryError> {
        let id = format!("{}:{}", user_id, rule_id);
        let _: Option<PushRule> = self.db.delete(("push_rule", id)).await?;
        Ok(())
    }

    pub fn subscribe_user(
        &self,
        user_id: &str,
    ) -> Pin<Box<dyn Stream<Item = Result<PushRule, RepositoryError>> + Send + '_>> {
        let query = format!("LIVE SELECT * FROM push_rule WHERE user_id = '{}'", user_id);
        let stream = self.db.query(query);

        Box::pin(async_stream::stream! {
            match stream.await {
                Ok(mut response) => {
                    match response.take::<Vec<PushRule>>(0) {
                        Ok(data) => {
                            for item in data {
                                yield Ok(item);
                            }
                        }
                        Err(e) => yield Err(RepositoryError::Database(e)),
                    }
                }
                Err(e) => yield Err(RepositoryError::Database(e)),
            }
        })
    }
}
