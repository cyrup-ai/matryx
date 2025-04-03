//! Room member wrapper with synchronous interfaces
//!
//! This module provides a clean wrapper around Matrix SDK's RoomMember class.

use matrix_sdk::room::RoomMember as MatrixRoomMember;
use matrix_sdk::ruma::UserId;
use std::sync::Arc;

/// A synchronous wrapper around the Matrix SDK RoomMember.
///
/// This wrapper provides a clean interface to Matrix room member data.
pub struct CyrumRoomMember {
    inner: Arc<MatrixRoomMember>,
}

impl CyrumRoomMember {
    /// Create a new CyrumRoomMember wrapping the provided Matrix RoomMember.
    pub fn new(inner: MatrixRoomMember) -> Self {
        Self { inner: Arc::new(inner) }
    }

    /// Get the inner Matrix room member.
    pub fn inner(&self) -> &MatrixRoomMember {
        &self.inner
    }

    /// Get the user ID of this member.
    pub fn user_id(&self) -> &UserId {
        self.inner.user_id()
    }

    /// Get the display name of this member.
    pub fn display_name(&self) -> Option<&str> {
        self.inner.display_name()
    }

    /// Get the avatar URL of this member.
    pub fn avatar_url(&self) -> Option<&str> {
        self.inner.avatar_url().map(|uri| uri.as_str()) // Convert Option<&MxcUri> to Option<&str>
    }

    /// Check if this member is a room administrator.
    pub fn is_admin(&self) -> bool {
        self.inner.power_level() >= 100
    }

    /// Check if this member is a room moderator.
    pub fn is_moderator(&self) -> bool {
        self.inner.power_level() >= 50
    }

    /// Check if this member can send messages.
    pub fn can_send_messages(&self) -> bool {
        self.inner.can_send_message()
    }

    /// Check if this member can redact messages.
    pub fn can_redact_messages(&self) -> bool {
        // Can redact if the user can redact either their own messages or messages from others
        self.inner.can_redact_own() || self.inner.can_redact_other()
    }

    /// Check if this member can redact their own messages.
    pub fn can_redact_own_messages(&self) -> bool {
        self.inner.can_redact_own()
    }

    /// Check if this member can redact messages from other users.
    pub fn can_redact_other_messages(&self) -> bool {
        self.inner.can_redact_other()
    }

    /// Check if this member can send state events.
    pub fn can_send_state_events(&self) -> bool {
        self.inner.can_send_state()
    }

    /// Get the power level of this member.
    pub fn power_level(&self) -> i64 {
        self.inner.power_level()
    }

    /// Check if this member is a name only.
    pub fn is_name_ambiguous(&self) -> bool {
        self.inner.name_ambiguous()
    }

    /// Get a normalized display name (adding user ID for disambiguation if needed).
    pub fn normalized_name(&self) -> String {
        if let Some(name) = self.inner.display_name() {
            if self.inner.name_ambiguous() {
                format!("{} ({})", name, self.inner.user_id())
            } else {
                name.to_string()
            }
        } else {
            self.inner.user_id().to_string()
        }
    }
}
