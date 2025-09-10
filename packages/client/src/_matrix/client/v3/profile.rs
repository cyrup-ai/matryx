//! Matrix User Profile Management
//!
//! GET/PUT /_matrix/client/v3/profile/{userId}
//! GET/PUT /_matrix/client/v3/profile/{userId}/displayname  
//! GET/PUT /_matrix/client/v3/profile/{userId}/avatar_url
//!
//! Manage user profiles including display names and avatar URLs.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};
use url::Url;

/// Complete user profile information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    /// The user's display name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub displayname: Option<String>,

    /// The user's avatar URL (MXC URI)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}

/// Request to update display name
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDisplayNameRequest {
    pub displayname: Option<String>,
}

/// Request to update avatar URL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAvatarUrlRequest {
    pub avatar_url: Option<String>,
}

/// Error response from Matrix server
#[derive(Debug, Clone, Deserialize)]
pub struct MatrixError {
    pub errcode: String,
    pub error: String,
}

/// Result type for client operations
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// GET /_matrix/client/v3/profile/{userId}
///
/// Get the combined profile information for a user.
pub async fn get_profile(
    client: &Client,
    homeserver_url: &Url,
    access_token: &str,
    user_id: &str,
) -> Result<UserProfile> {
    let url = homeserver_url
        .join(&format!("/_matrix/client/v3/profile/{}", urlencoding::encode(user_id)))?;

    debug!("Getting profile for user: {}", user_id);

    let response = client
        .get(url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "Matryx-Client/0.1.0")
        .send()
        .await?;

    if response.status().is_success() {
        let profile: UserProfile = response.json().await?;
        debug!(
            "Retrieved profile for {}: displayname={:?}, avatar_url={:?}",
            user_id, profile.displayname, profile.avatar_url
        );
        Ok(profile)
    } else {
        let status = response.status();
        let error_text = response.text().await?;

        // Try to parse as Matrix error
        if let Ok(matrix_error) = serde_json::from_str::<MatrixError>(&error_text) {
            error!(
                "Matrix server error getting profile: {} - {}",
                matrix_error.errcode, matrix_error.error
            );
            return Err(
                format!("Matrix error {}: {}", matrix_error.errcode, matrix_error.error).into()
            );
        }

        error!("HTTP error getting profile: {} - {}", status, error_text);
        Err(format!("HTTP error {}: {}", status, error_text).into())
    }
}

/// GET /_matrix/client/v3/profile/{userId}/displayname
///
/// Get just the display name for a user.
pub async fn get_display_name(
    client: &Client,
    homeserver_url: &Url,
    access_token: &str,
    user_id: &str,
) -> Result<Option<String>> {
    let url = homeserver_url.join(&format!(
        "/_matrix/client/v3/profile/{}/displayname",
        urlencoding::encode(user_id)
    ))?;

    debug!("Getting display name for user: {}", user_id);

    let response = client
        .get(url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "Matryx-Client/0.1.0")
        .send()
        .await?;

    if response.status().is_success() {
        let display_name_response: UpdateDisplayNameRequest = response.json().await?;
        debug!("Retrieved display name for {}: {:?}", user_id, display_name_response.displayname);
        Ok(display_name_response.displayname)
    } else {
        let status = response.status();
        let error_text = response.text().await?;

        if let Ok(matrix_error) = serde_json::from_str::<MatrixError>(&error_text) {
            error!(
                "Matrix server error getting display name: {} - {}",
                matrix_error.errcode, matrix_error.error
            );
            return Err(
                format!("Matrix error {}: {}", matrix_error.errcode, matrix_error.error).into()
            );
        }

        error!("HTTP error getting display name: {} - {}", status, error_text);
        Err(format!("HTTP error {}: {}", status, error_text).into())
    }
}

/// PUT /_matrix/client/v3/profile/{userId}/displayname
///
/// Set the display name for a user. The user can only change their own display name.
pub async fn set_display_name(
    client: &Client,
    homeserver_url: &Url,
    access_token: &str,
    user_id: &str,
    display_name: Option<&str>,
) -> Result<()> {
    let url = homeserver_url.join(&format!(
        "/_matrix/client/v3/profile/{}/displayname",
        urlencoding::encode(user_id)
    ))?;

    debug!("Setting display name for user {}: {:?}", user_id, display_name);

    let request = UpdateDisplayNameRequest { displayname: display_name.map(|s| s.to_string()) };

    let response = client
        .put(url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .header("User-Agent", "Matryx-Client/0.1.0")
        .json(&request)
        .send()
        .await?;

    if response.status().is_success() {
        info!("Successfully updated display name for {}: {:?}", user_id, display_name);
        Ok(())
    } else {
        let status = response.status();
        let error_text = response.text().await?;

        if let Ok(matrix_error) = serde_json::from_str::<MatrixError>(&error_text) {
            error!(
                "Matrix server error setting display name: {} - {}",
                matrix_error.errcode, matrix_error.error
            );
            return Err(
                format!("Matrix error {}: {}", matrix_error.errcode, matrix_error.error).into()
            );
        }

        error!("HTTP error setting display name: {} - {}", status, error_text);
        Err(format!("HTTP error {}: {}", status, error_text).into())
    }
}

/// GET /_matrix/client/v3/profile/{userId}/avatar_url
///
/// Get just the avatar URL for a user.
pub async fn get_avatar_url(
    client: &Client,
    homeserver_url: &Url,
    access_token: &str,
    user_id: &str,
) -> Result<Option<String>> {
    let url = homeserver_url
        .join(&format!("/_matrix/client/v3/profile/{}/avatar_url", urlencoding::encode(user_id)))?;

    debug!("Getting avatar URL for user: {}", user_id);

    let response = client
        .get(url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "Matryx-Client/0.1.0")
        .send()
        .await?;

    if response.status().is_success() {
        let avatar_response: UpdateAvatarUrlRequest = response.json().await?;
        debug!("Retrieved avatar URL for {}: {:?}", user_id, avatar_response.avatar_url);
        Ok(avatar_response.avatar_url)
    } else {
        let status = response.status();
        let error_text = response.text().await?;

        if let Ok(matrix_error) = serde_json::from_str::<MatrixError>(&error_text) {
            error!(
                "Matrix server error getting avatar URL: {} - {}",
                matrix_error.errcode, matrix_error.error
            );
            return Err(
                format!("Matrix error {}: {}", matrix_error.errcode, matrix_error.error).into()
            );
        }

        error!("HTTP error getting avatar URL: {} - {}", status, error_text);
        Err(format!("HTTP error {}: {}", status, error_text).into())
    }
}

/// PUT /_matrix/client/v3/profile/{userId}/avatar_url
///
/// Set the avatar URL for a user. The user can only change their own avatar.
/// The avatar_url should be an MXC URI (e.g., "mxc://example.org/abc123").
pub async fn set_avatar_url(
    client: &Client,
    homeserver_url: &Url,
    access_token: &str,
    user_id: &str,
    avatar_url: Option<&str>,
) -> Result<()> {
    let url = homeserver_url
        .join(&format!("/_matrix/client/v3/profile/{}/avatar_url", urlencoding::encode(user_id)))?;

    debug!("Setting avatar URL for user {}: {:?}", user_id, avatar_url);

    let request = UpdateAvatarUrlRequest { avatar_url: avatar_url.map(|s| s.to_string()) };

    let response = client
        .put(url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .header("User-Agent", "Matryx-Client/0.1.0")
        .json(&request)
        .send()
        .await?;

    if response.status().is_success() {
        info!("Successfully updated avatar URL for {}: {:?}", user_id, avatar_url);
        Ok(())
    } else {
        let status = response.status();
        let error_text = response.text().await?;

        if let Ok(matrix_error) = serde_json::from_str::<MatrixError>(&error_text) {
            error!(
                "Matrix server error setting avatar URL: {} - {}",
                matrix_error.errcode, matrix_error.error
            );
            return Err(
                format!("Matrix error {}: {}", matrix_error.errcode, matrix_error.error).into()
            );
        }

        error!("HTTP error setting avatar URL: {} - {}", status, error_text);
        Err(format!("HTTP error {}: {}", status, error_text).into())
    }
}
