//! Encryption wrapper with synchronous interfaces
//!
//! This module provides a clean, synchronous interface to Matrix SDK's Encryption functionality
//! that eliminates the need for async_trait and Box<dyn Future> in client code.

use std::sync::Arc;
use tokio::runtime::Handle;

// Imports for SDK 0.11+
use matrix_sdk::{
    encryption::{
        verification::{Emoji, SasVerification, Verification},
    },
    ruma::{DeviceId, UserId},
    Client as MatrixClient,
};

// Define local types until we update to SDK reference types
// For BackupDecryptionKey/recovery key
type BackupDecryptionKey = Vec<u8>; // Simplified for compilation

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

    // Use from_base58 for SDK 0.10+
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
                Verification::QrCodeV1(_) => { // Handle QrCodeV1 if needed, or return error
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
                Verification::QrCodeV1(qr) => { // Handle QrCodeV1
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
            Verification::QrCodeV1(qr) => Ok(qr.other_user_id()), // Assuming QrCodeV1 has other_user_id
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
            Verification::QrCodeV1(qr) => Ok(qr.other_device_id()),
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
            // get_verification likely renamed or moved, e.g., request_verification
            let verification = client
                .encryption()
                .request_verification(&user_id, None) // Assuming this method exists
                .await
                .map_err(EncryptionError::matrix_sdk)?;

            Ok(MatrixVerificationRequest::new(verification)) // Pass Arc directly
        })
    }

    /// Enable room key backup.
    pub fn enable_backup(&self) -> MatrixFuture<String> {
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            // Use the recovery API
            let recovery = client.encryption().recovery();

            // Check if recovery is enabled, enable if not
            if recovery.state() == matrix_sdk::encryption::recovery::RecoveryState::Disabled {
                let _recovery_key = recovery.enable().await.map_err(EncryptionError::matrix_sdk)?;
            }

            // Enable backup
            recovery.enable_backup().await.map_err(EncryptionError::matrix_sdk)?; // Assuming enable_backup exists

            // Get the current backup version (if needed)
            // let state = recovery.state(); // Example
            // let version = state.backup_version().map(|v| v.to_string()).unwrap_or_default(); // Example
            Ok("".to_string()) // Return empty string for now, adjust if version needed
        })
    }

    /// Restore room keys from backup.
    pub fn restore_backup(&self, passphrase: Option<&str>) -> MatrixFuture<usize> {
        let passphrase = passphrase.map(|s| s.to_owned());
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            let recovery = client.encryption().recovery();

            // Restore backup using passphrase if provided
            let result = if let Some(pass) = passphrase {
                recovery.restore_backup_from_passphrase(&pass, None).await // Pass None for progress
            } else {
                // Attempt restore without passphrase (might use cached key)
                // Check SDK 0.10+ documentation for the exact method
                // recovery.restore_backup_with_cached_key().await // Example, method likely changed/removed
                 return Err(EncryptionError::InvalidParameter(
                     "Passphrase needed or cached key restore method not found for SDK 0.10+".to_string(),
                 )); // Keep error until method verified
            };

            match result {
                 Ok(counts) => Ok(counts.total as usize), // Return total count
                 Err(e) => Err(EncryptionError::matrix_sdk(e)),
             }
        })
    }

    /// Check if a backup exists.
    pub fn has_backup(&self) -> MatrixFuture<bool> {
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            // Use recovery status
            let recovery = client.encryption().recovery();
            // TODO: Verify how to check backup status/enabled state in SDK 0.10+ (e.g., using recovery.state())
            let state = recovery.state(); // Example
            // Check if backup is configured/enabled based on status
            Ok(state != matrix_sdk::encryption::recovery::RecoveryState::Disabled) // Example check
        })
    }

    /// Get recovery key as a string.
    pub fn export_recovery_key(&self) -> MatrixFuture<String> {
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            // Use recovery API
            let recovery = client.encryption().recovery();
            // In Matrix SDK 0.13, recovery key is managed through state, not directly accessible
            // Return placeholder for now - in practice, the recovery key would be stored when created
            let key = None; // Assuming get_recovery_key exists

            key.ok_or_else(|| EncryptionError::MatrixSdk("No recovery key found".to_string()))
               .map(|k| k.to_base58()) // Convert key to base58 string
        })
    }

    /// Import recovery key.
    pub fn import_recovery_key(&self, recovery_key: &str) -> MatrixFuture<usize> {
        let recovery_key = recovery_key.to_owned();
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            let key = parse_recovery_key(&recovery_key)
                .map_err(EncryptionError::InvalidRecoveryKey)?;

            // Use recovery API
            let recovery = client.encryption().recovery();

            // Import the key
            // recovery.import_recovery_key(key, None).await // Method likely changed
            // Placeholder error until method verified
            return Err(EncryptionError::MatrixSdk("import_recovery_key needs verification".into()));

            // After importing, keys might be restored automatically or need another step.
            // Check SDK 0.10+ docs. Assuming import triggers restore:
            // We might not get a count directly from import. Return 0 or trigger restore separately.
            // Ok(0) // Placeholder count
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

            // In 0.11, verification requests are handled through the encryption() API
            // rather than directly through events
            let crypto = client.encryption();
            
            // Set up a listener for new verification requests
            crypto.register_verification_handler(move |request| {
                let sender = sender.clone();
                
                async move {
                    // Wrap the verification in our custom type
                    let verification = Arc::new(request);
                    
                    // Send the wrapped verification request
                    let _ = sender
                        .send(Ok(MatrixVerificationRequest::new(verification)))
                        .await;
                    
                    // Return true to indicate we'll handle this request
                    true
                }
            });

            // Convert receiver to stream
            Ok(tokio_stream::wrappers::ReceiverStream::new(receiver))
        })
    }
}
