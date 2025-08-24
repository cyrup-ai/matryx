//! Encryption wrapper with synchronous interfaces
//!
//! This module provides a clean, synchronous interface to Matrix SDK's Encryption functionality
//! that eliminates the need for async_trait and Box<dyn Future> in client code.

use std::sync::Arc;
use tokio::runtime::Handle;

// Imports for SDK 0.13+
use matrix_sdk::{
    encryption::{
        verification::{Emoji, SasVerification, Verification, VerificationRequest},
    },
    ruma::{
        DeviceId, UserId,
        events::{
            key::verification::request::ToDeviceKeyVerificationRequestEvent,
            room::message::{MessageType, OriginalSyncRoomMessageEvent},
        },
    },
    Client as MatrixClient,
};
use matrix_sdk_base::crypto::store::types::BackupDecryptionKey;

// Define our verification state enum
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationState {
    Verified,
    Unverified,
    Unknown,
}

// Helper function to convert a string recovery key to BackupDecryptionKey
fn parse_recovery_key(recovery_key: &str) -> std::result::Result<BackupDecryptionKey, String> {
    // Validate the recovery key format - should be base58 encoded
    // The prefix might change, adjust if needed based on SDK 0.10+ keys
    // if !recovery_key.starts_with("...") {
    //     return Err("Invalid recovery key format".into());
    // }

    // Use from_base58 for SDK 0.13+
    BackupDecryptionKey::from_base58(recovery_key)
        .map_err(|e| format!("Invalid recovery key: {}", e))
}

use crate::error::EncryptionError;
use crate::future::{MatrixFuture, MatrixStream};

/// Wrapper for SAS verification that hides async complexity
pub struct MatrixSasVerification {
    inner: Arc<SasVerification>,
    runtime_handle: Handle,
}

impl MatrixSasVerification {
    /// Create a new MatrixSasVerification wrapping a SasVerification
    fn new(inner: SasVerification) -> Self {
        Self {
            inner: Arc::new(inner),
            runtime_handle: Handle::current(),
        }
    }

    /// Get the emoji for verification
    pub fn emoji(&self) -> Option<Vec<Emoji>> { // Returns Option<Arc<[Emoji]>>
        self.inner.emoji().map(|e| e.to_vec()) // Convert Arc<[Emoji]> to Vec<Emoji>
    }

    /// Accept the verification
    pub fn accept(&self) -> MatrixFuture<()> {
        let inner = self.inner.clone();

        MatrixFuture::spawn(async move {
            inner.accept().await.map_err(EncryptionError::matrix_sdk).map_err(crate::error::Error::Encryption)
        })
    }

    /// Cancel the verification
    pub fn cancel(&self) -> MatrixFuture<()> {
        let inner = self.inner.clone();

        MatrixFuture::spawn(async move {
            inner.cancel().await.map_err(EncryptionError::matrix_sdk).map_err(crate::error::Error::Encryption)
        })
    }

    /// Confirm the verification (emoji match)
    pub fn confirm(&self) -> MatrixFuture<()> {
        let inner = self.inner.clone();

        MatrixFuture::spawn(async move {
            inner.confirm().await.map_err(EncryptionError::matrix_sdk).map_err(crate::error::Error::Encryption)
        })
    }

    /// Get the user ID being verified
    pub fn other_user_id(&self) -> &UserId {
        self.inner.other_user_id()
    }

    /// Get the device ID being verified
    pub fn other_device_id(&self) -> &DeviceId { // Returns &DeviceId directly
        self.inner.other_device_id()
    }
}

/// A wrapper around Matrix SDK's VerificationRequest that hides async complexity
pub struct MatrixVerificationRequest {
    inner: Arc<Verification>,
    runtime_handle: Handle,
}

impl MatrixVerificationRequest {
    /// Create a new MatrixVerificationRequest from a Verification
    fn new(inner: Arc<Verification>) -> Self { // Accept Arc<Verification>
        Self {
            inner, // Store the Arc directly
            runtime_handle: Handle::current(),
        }
    }

    /// Accept the verification request with the SAS method
    pub fn accept_sas(&self) -> MatrixFuture<MatrixSasVerification> {
        let inner = self.inner.clone();

        MatrixFuture::spawn(async move {
            let result = match &*inner { // Match on the dereferenced Arc
                Verification::SasV1(sas) => {
                    sas.accept().await.map_err(EncryptionError::matrix_sdk)?;
                    // Pass the existing Arc<SasVerification>
                    Ok(MatrixSasVerification::new(sas.clone()))
                },
                Verification::QrV1(_) => { // Handle QrV1 if needed, or return error
                    Err(EncryptionError::UnsupportedVerificationType(
                        "QR code verification not handled by accept_sas".into(),
                    ))
                }
                // Add other verification types if they exist in SDK 0.10+
                #[allow(unreachable_patterns)] // Keep allow if only Sas and Qr exist
                _ => {
                    Err(EncryptionError::InvalidVerificationType(
                        "This verification is not SAS or QR".into(),
                    ))
                },
            };
            result.map_err(crate::error::Error::Encryption)
        })
    }

    /// Cancel the verification request
    pub fn cancel(&self) -> MatrixFuture<()> {
        let inner = self.inner.clone();

        MatrixFuture::spawn(async move {
            let result = match &*inner { // Match on the dereferenced Arc
                Verification::SasV1(sas) => sas.cancel().await.map_err(EncryptionError::matrix_sdk),
                Verification::QrV1(qr) => { // Handle QrV1
                    qr.cancel().await.map_err(EncryptionError::matrix_sdk) // Assuming cancel exists
                },
                // Add other verification types if they exist in SDK 0.10+
                #[allow(unreachable_patterns)] // Keep allow if only Sas and Qr exist
                _ => {
                    Err(EncryptionError::UnsupportedVerificationType(
                        "Unknown verification type for cancel".into(),
                    ))
                },
            };
            result.map_err(crate::error::Error::Encryption)
        })
    }

    /// Get the user ID of the other party
    pub fn other_user_id(&self) -> Result<&UserId, EncryptionError> {
        match &*self.inner {
            Verification::SasV1(sas) => Ok(sas.other_user_id()),
            Verification::QrV1(qr) => Ok(qr.other_user_id()), // Assuming QrV1 has other_user_id
            #[allow(unreachable_patterns)]
            _ => Err(EncryptionError::UnsupportedVerificationType(
                "Cannot get other_user_id for this verification type".into(),
            )),
        }
    }

    /// Get the device ID of the other party
    pub fn other_device_id(&self) -> Result<&DeviceId, EncryptionError> { // Returns Result<&DeviceId>
        match &*self.inner {
            Verification::SasV1(sas) => Ok(sas.other_device_id()),
            Verification::QrV1(qr) => Ok(qr.other_device_id()),
            // Add other verification types if they exist in SDK 0.10+
            #[allow(unreachable_patterns)] // Keep allow if only Sas and Qr exist
            _ => Err(EncryptionError::UnsupportedVerificationType(
                "Cannot get other_device_id for this verification type".into(),
            )),
        }
    }
}

/// A synchronous wrapper around the Matrix SDK Encryption functionality.
///
/// This wrapper enables using the Encryption manager with a synchronous interface,
/// hiding all async complexity behind MatrixFuture objects that properly
/// implement the Future trait.
pub struct MatrixEncryption {
    client: Arc<MatrixClient>,
    runtime_handle: Handle,
}

impl MatrixEncryption {
    /// Create a new MatrixEncryption with the provided Matrix client.
    pub fn new(client: Arc<MatrixClient>) -> Self {
        Self { client, runtime_handle: Handle::current() }
    }

    /// Start a key verification with the given user and device.
    pub fn verify_device(
        &self,
        user_id: &UserId,
        device_id: &DeviceId,
    ) -> MatrixFuture<MatrixVerificationRequest> {
        let user_id = user_id.to_owned();
        let device_id = device_id.to_owned();
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            // In Matrix SDK 0.13, request verification from specific device
            let encryption = client.encryption();
            
            let device = encryption.get_device(&user_id, &device_id).await.map_err(EncryptionError::matrix_sdk)?;
            
            let verification = if let Some(device) = device {
                device.request_verification().await.map_err(EncryptionError::matrix_sdk)?
            } else {
                return Err(EncryptionError::MatrixSdk("Device not found".to_string()));
            };
            
            Ok(MatrixVerificationRequest::new(verification))
        })
    }

    /// Start a key verification with the given user (without specifying a device).
    pub fn verify_user(&self, user_id: &UserId) -> MatrixFuture<MatrixVerificationRequest> {
        let user_id = user_id.to_owned();
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            // First get the user identity, then request verification
            let user_identity = client
                .encryption()
                .get_user_identity(&user_id)
                .await
                .map_err(EncryptionError::matrix_sdk)?
                .ok_or(EncryptionError::UserIdentityNotFound)?;

            let verification_request = user_identity
                .request_verification()
                .await
                .map_err(EncryptionError::matrix_sdk)?;

            Ok(MatrixVerificationRequest::new(Arc::new(verification_request)))
        })
    }

    /// Enable room key backup.
    pub fn enable_backup(&self) -> MatrixFuture<String> {
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            let recovery = client.encryption().recovery();

            // Enable backup through recovery API
            recovery.enable_backup().await.map_err(EncryptionError::matrix_sdk)?;

            // Return empty string as backup version isn't directly accessible
            Ok("".to_string())
        })
    }

    /// Restore room keys from backup using recovery key.
    pub fn restore_backup(&self, recovery_key: Option<&str>) -> MatrixFuture<usize> {
        let recovery_key = recovery_key.map(|s| s.to_owned());
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            let recovery = client.encryption().recovery();

            // Restore backup using recovery key if provided
            let result = if let Some(key) = recovery_key {
                recovery.recover(&key).await.map_err(EncryptionError::matrix_sdk)
            } else {
                return Err(EncryptionError::InvalidParameter(
                    "Recovery key required for backup restoration".to_string(),
                ));
            };

            match result {
                Ok(()) => {
                    // Recovery doesn't return a count directly, but we can check if backups are enabled
                    let backups_enabled = client.encryption().backups().are_enabled().await;
                    if backups_enabled {
                        Ok(1) // Return 1 to indicate successful recovery
                    } else {
                        Ok(0)
                    }
                },
                Err(e) => Err(e),
            }
        })
    }

    /// Check if a backup exists.
    pub fn has_backup(&self) -> MatrixFuture<bool> {
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            // Check if backups exist on server
            client.encryption().backups().exists_on_server().await.map_err(EncryptionError::matrix_sdk)
        })
    }

    /// Get recovery key as a string.
    pub fn export_recovery_key(&self) -> MatrixFuture<String> {
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            // In Matrix SDK 0.13, recovery keys are generated and returned during the enable process
            // This method is for exporting an existing key, which may not be directly accessible
            // Return error indicating the key should be saved during creation
            Err(EncryptionError::MatrixSdk("Recovery key must be saved during creation - not directly exportable".to_string()))
        })
    }

    /// Import recovery key.
    pub fn import_recovery_key(&self, recovery_key: &str) -> MatrixFuture<usize> {
        let recovery_key = recovery_key.to_owned();
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            // In Matrix SDK 0.13, importing a recovery key is done through the recover method
            let recovery = client.encryption().recovery();

            // Import and restore using the recovery key
            recovery.recover(&recovery_key).await.map_err(EncryptionError::matrix_sdk)?;

            // Check if backups are now enabled to return success indicator
            let backups_enabled = client.encryption().backups().are_enabled().await;
            if backups_enabled {
                Ok(1) // Return 1 to indicate successful import and activation
            } else {
                Ok(0)
            }
        })
    }

    /// Get device verification status.
    pub fn get_device_verification(
        &self,
        user_id: &UserId,
        device_id: &DeviceId,
    ) -> MatrixFuture<VerificationState> {
        let user_id = user_id.to_owned();
        let device_id = device_id.to_owned();
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            let device = client
                .encryption()
                .get_device(&user_id, &device_id)
                .await
                .map_err(EncryptionError::matrix_sdk)? // Propagate error
                .ok_or_else(|| EncryptionError::MatrixSdk(format!("Device not found: {}/{}", user_id, device_id)))?; // Handle Option

            // Map SDK verification status to our enum
            if device.is_verified() {
                Ok(VerificationState::Verified)
            } else {
                Ok(VerificationState::Unverified)
            }
        })
    }

    /// Check if a device is verified.
    pub fn is_device_verified(&self, user_id: &UserId, device_id: &DeviceId) -> MatrixFuture<bool> {
        let user_id = user_id.to_owned();
        let device_id = device_id.to_owned();
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            let device = client
                .encryption()
                .get_device(&user_id, &device_id)
                .await
                .map_err(EncryptionError::matrix_sdk)? // Propagate error
                .ok_or_else(|| EncryptionError::MatrixSdk(format!("Device not found: {}/{}", user_id, device_id)))?; // Handle Option

            Ok(device.is_verified())
        })
    }

    /// Subscribe to verification requests.
    pub fn subscribe_to_verification_requests(&self) -> MatrixStream<MatrixVerificationRequest> {
        let client = self.client.clone();

        MatrixStream::spawn(async move {
            let (sender, receiver) = tokio::sync::mpsc::channel(100);

            // In Matrix SDK 0.13, verification requests are handled through event handlers
            use matrix_sdk::ruma::events::{
                key::verification::request::ToDeviceKeyVerificationRequestEvent,
                room::message::{MessageType, OriginalSyncRoomMessageEvent},
            };

            let sender_clone = sender.clone();
            client.add_event_handler(move |ev: ToDeviceKeyVerificationRequestEvent, client: matrix_sdk::Client| {
                let sender = sender_clone.clone();
                async move {
                    if let Ok(Some(request)) = client
                        .encryption()
                        .get_verification_request(&ev.sender, &ev.content.transaction_id)
                        .await 
                    {
                        let verification = Arc::new(request);
                        let _ = sender.send(Ok(MatrixVerificationRequest::new(verification))).await;
                    }
                }
            });

            let sender_clone2 = sender.clone();
            client.add_event_handler(move |ev: OriginalSyncRoomMessageEvent, client: matrix_sdk::Client| {
                let sender = sender_clone2.clone();
                async move {
                    if let MessageType::VerificationRequest(_) = &ev.content.msgtype {
                        if let Ok(Some(request)) = client
                            .encryption()
                            .get_verification_request(&ev.sender, &ev.event_id)
                            .await
                        {
                            let verification = Arc::new(request);
                            let _ = sender.send(Ok(MatrixVerificationRequest::new(verification))).await;
                        }
                    }
                }
            });

            // Convert receiver to stream
            Ok(tokio_stream::wrappers::ReceiverStream::new(receiver))
        })
    }
}
