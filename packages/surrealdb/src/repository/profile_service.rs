use crate::repository::{
    AccountDataRepository,
    ReportsRepository,
    TagsRepository,
    ThirdPartyRepository,
    UserRepository,
    error::RepositoryError,
};
use surrealdb::Surreal;

use matryx_entity::types::{
    ProfileResponse,
    TagsResponse,
    ThirdPartyIdInfo,
    ThirdPartyOperation,
    ThirdPartyResponse,
    WhoAmIResponse,
};
use serde_json::Value;

/// Profile management service that coordinates between repositories
#[derive(Clone)]
pub struct ProfileManagementService {
    user_repo: UserRepository,
    account_data_repo: AccountDataRepository,
    tags_repo: TagsRepository,
    reports_repo: ReportsRepository<surrealdb::engine::any::Any>,
    third_party_repo: ThirdPartyRepository<surrealdb::engine::any::Any>,
}

impl ProfileManagementService {
    pub fn new(db: Surreal<surrealdb::engine::any::Any>) -> Self {
        Self {
            user_repo: UserRepository::new(db.clone()),
            account_data_repo: AccountDataRepository::new(db.clone()),
            tags_repo: TagsRepository::new(db.clone()),
            reports_repo: ReportsRepository::new(db.clone()),
            third_party_repo: ThirdPartyRepository::new(db),
        }
    }

    /// Get user profile with permission validation
    pub async fn get_user_profile(
        &self,
        user_id: &str,
        requesting_user: &str,
    ) -> Result<ProfileResponse, RepositoryError> {
        // Validate permissions to view profile
        if !self
            .user_repo
            .validate_profile_update_permissions(user_id, requesting_user)
            .await?
        {
            // For profile viewing, we're more permissive - only restrict if user doesn't exist
            if self.user_repo.get_by_id(user_id).await?.is_none() {
                return Err(RepositoryError::NotFound {
                    entity_type: "User".to_string(),
                    id: user_id.to_string(),
                });
            }
        }

        let profile = self.user_repo.get_user_profile(user_id).await?;
        Ok(ProfileResponse::from_user_profile(&profile))
    }

    /// Update user display name
    pub async fn update_display_name(
        &self,
        user_id: &str,
        display_name: Option<String>,
    ) -> Result<(), RepositoryError> {
        self.user_repo.update_display_name(user_id, display_name).await
    }

    /// Update user avatar URL
    pub async fn update_avatar_url(
        &self,
        user_id: &str,
        avatar_url: Option<String>,
    ) -> Result<(), RepositoryError> {
        self.user_repo.update_avatar_url(user_id, avatar_url).await
    }
    /// Set account data (global or room-specific)
    pub async fn set_account_data(
        &self,
        user_id: &str,
        data_type: &str,
        content: Value,
        room_id: Option<&str>,
    ) -> Result<(), RepositoryError> {
        match room_id {
            Some(room) => {
                self.account_data_repo
                    .set_room_account_data(user_id, room, data_type, content)
                    .await
            },
            None => {
                self.account_data_repo
                    .set_global_account_data(user_id, data_type, content)
                    .await
            },
        }
    }

    /// Get account data (global or room-specific)
    pub async fn get_account_data(
        &self,
        user_id: &str,
        data_type: &str,
        room_id: Option<&str>,
    ) -> Result<Option<Value>, RepositoryError> {
        let content = match room_id {
            Some(room) => {
                self.account_data_repo
                    .get_room_account_data_content(user_id, room, data_type)
                    .await?
            },
            None => self.account_data_repo.get_global_account_data(user_id, data_type).await?,
        };

        Ok(content)
    }

    /// Manage room tag (set or update)
    pub async fn manage_room_tag(
        &self,
        user_id: &str,
        room_id: &str,
        tag: &str,
        content: Option<Value>,
    ) -> Result<(), RepositoryError> {
        self.tags_repo.set_room_tag(user_id, room_id, tag, content).await
    }

    /// Remove room tag
    pub async fn remove_room_tag(
        &self,
        user_id: &str,
        room_id: &str,
        tag: &str,
    ) -> Result<(), RepositoryError> {
        self.tags_repo.remove_room_tag(user_id, room_id, tag).await
    }

    /// Get room tags
    pub async fn get_room_tags(
        &self,
        user_id: &str,
        room_id: &str,
    ) -> Result<TagsResponse, RepositoryError> {
        let tags = self.tags_repo.get_room_tags(user_id, room_id).await?;
        Ok(TagsResponse::new(tags))
    }

    /// Report a user
    pub async fn report_user(
        &self,
        reporter_id: &str,
        reported_user_id: &str,
        reason: &str,
        content: Option<Value>,
    ) -> Result<(), RepositoryError> {
        let _report = self
            .reports_repo
            .create_user_report(reporter_id, reported_user_id, reason, content)
            .await?;
        Ok(())
    }

    /// Deactivate user account
    pub async fn deactivate_account(
        &self,
        user_id: &str,
        erase_data: bool,
    ) -> Result<(), RepositoryError> {
        self.user_repo.deactivate_account(user_id, erase_data).await
    }

    /// Get whoami information
    pub async fn get_whoami_info(&self, user_id: &str) -> Result<WhoAmIResponse, RepositoryError> {
        // Verify user exists and get basic info
        let user_info = self.user_repo.get_user_info(user_id).await?;

        // For now, we don't track device info in this service
        // In a full implementation, we'd also query device repository
        Ok(WhoAmIResponse::user(user_info.user_id))
    }

    /// Manage third-party identifiers
    pub async fn manage_third_party_ids(
        &self,
        user_id: &str,
        operation: ThirdPartyOperation,
    ) -> Result<ThirdPartyResponse, RepositoryError> {
        match operation {
            ThirdPartyOperation::Add { medium, address, validated } => {
                let _third_party_id = self
                    .third_party_repo
                    .add_third_party_identifier(user_id, &medium, &address, validated)
                    .await?;
                // Return updated list
                let identifiers = self.third_party_repo.get_user_third_party_ids(user_id).await?;
                let threepids =
                    identifiers.iter().map(ThirdPartyIdInfo::from_third_party_id).collect();
                Ok(ThirdPartyResponse::new(threepids))
            },
            ThirdPartyOperation::Remove { medium, address } => {
                self.third_party_repo
                    .remove_third_party_identifier(user_id, &medium, &address)
                    .await?;
                // Return updated list
                let identifiers = self.third_party_repo.get_user_third_party_ids(user_id).await?;
                let threepids =
                    identifiers.iter().map(ThirdPartyIdInfo::from_third_party_id).collect();
                Ok(ThirdPartyResponse::new(threepids))
            },
            ThirdPartyOperation::Validate { medium, address, token } => {
                let validated = self
                    .third_party_repo
                    .validate_third_party_identifier(user_id, &medium, &address, &token)
                    .await?;
                if !validated {
                    return Err(RepositoryError::Validation {
                        field: "token".to_string(),
                        message: "Invalid or expired validation token".to_string(),
                    });
                }
                // Return updated list
                let identifiers = self.third_party_repo.get_user_third_party_ids(user_id).await?;
                let threepids =
                    identifiers.iter().map(ThirdPartyIdInfo::from_third_party_id).collect();
                Ok(ThirdPartyResponse::new(threepids))
            },
            ThirdPartyOperation::List => {
                let identifiers = self.third_party_repo.get_user_third_party_ids(user_id).await?;
                let threepids =
                    identifiers.iter().map(ThirdPartyIdInfo::from_third_party_id).collect();
                Ok(ThirdPartyResponse::new(threepids))
            },
        }
    }
}
