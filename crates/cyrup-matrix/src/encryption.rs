//! Encryption wrapper with synchronous interfaces
//!
//! This module provides a clean, synchronous interface to Matrix SDK's Encryption functionality
//! that eliminates the need for async_trait and Box<dyn Future> in client code.

use std::sync::Arc;
use tokio::runtime::Handle;

use matrix_sdk::{
    encryption::{
        verification::{Emoji, SasVerification, Verification},
        BackupDownloadStrategy,
        VerificationState,
    },
    ruma::{DeviceId, UserId},
    Client as MatrixClient,
};

// Import the matrix-sdk's BackupDecryptionKey type
use matrix_sdk::crypto::store::BackupDecryptionKey;

// Helper function to convert a string recovery key to BackupDecryptionKey
fn parse_recovery_key(recovery_key: &str) -> std::result::Result<BackupDecryptionKey, String> {
    // Validate the recovery key format - should be base58 encoded
    if !recovery_key.starts_with("EzR") {
        return Err("Invalid recovery key format".into());
    }

    // Matrix SDK 0.10.0 uses this method to parse a recovery key
    BackupDecryptionKey::from_recovery_key(recovery_key)
        .map_err(|e| format!("Invalid recovery key: {}", e))
}

use crate::error::EncryptionError;
use crate::future::{MatrixFuture, MatrixStream};

/// Wrapper for SAS verification that hides async complexity
pub struct CyrumSasVerification {
    inner: Arc<SasVerification>,
    runtime_handle: Handle,
}

impl CyrumSasVerification {
    /// Create a new CyrumSasVerification wrapping a SasVerification
    fn new(inner: SasVerification) -> Self {
        Self {
            inner: Arc::new(inner),
            runtime_handle: Handle::current(),
        }
    }

    /// Get the emoji for verification
    pub fn emoji(&self) -> Vec<Emoji> {
        self.inner.emoji().to_vec()
    }

    /// Accept the verification
    pub fn accept(&self) -> MatrixFuture<()> {
        let inner = self.inner.clone();

        MatrixFuture::spawn(
            async move { inner.accept().await.map_err(EncryptionError::matrix_sdk) },
        )
    }

    /// Cancel the verification
    pub fn cancel(&self) -> MatrixFuture<()> {
        let inner = self.inner.clone();

        MatrixFuture::spawn(
            async move { inner.cancel().await.map_err(EncryptionError::matrix_sdk) },
        )
    }

    /// Confirm the verification (emoji match)
    pub fn confirm(&self) -> MatrixFuture<()> {
        let inner = self.inner.clone();

        MatrixFuture::spawn(
            async move { inner.confirm().await.map_err(EncryptionError::matrix_sdk) },
        )
    }

    /// Get the user ID being verified
    pub fn other_user_id(&self) -> &UserId {
        self.inner.other_user_id()
    }

    /// Get the device ID being verified, if available
    pub fn other_device_id(&self) -> Option<&DeviceId> {
        self.inner.other_device_id()
    }
}

/// A wrapper around Matrix SDK's VerificationRequest that hides async complexity
pub struct CyrumVerificationRequest {
    inner: Arc<Verification>,
    runtime_handle: Handle,
}

impl CyrumVerificationRequest {
    /// Create a new CyrumVerificationRequest from a Verification
    fn new(inner: Verification) -> Self {
        Self {
            inner: Arc::new(inner),
            runtime_handle: Handle::current(),
        }
    }

    /// Accept the verification request with the SAS method
    pub fn accept_sas(&self) -> MatrixFuture<CyrumSasVerification> {
        let inner = self.inner.clone();

        MatrixFuture::spawn(async move {
            match inner {
                Verification::SasV1(sas) => {
                    sas.accept().await.map_err(EncryptionError::matrix_sdk)?;
                    Ok(CyrumSasVerification::new(sas.clone()))
                },
                _ => {
                    Err(EncryptionError::InvalidVerificationType(
                        "This verification is not a SAS verification".into(),
                    ))
                },
            }
        })
    }

    /// Cancel the verification request
    pub fn cancel(&self) -> MatrixFuture<()> {
        let inner = self.inner.clone();

        MatrixFuture::spawn(async move {
            match &*inner {
                Verification::SasV1(sas) => sas.cancel().await.map_err(EncryptionError::matrix_sdk),
                Verification::QrV1(_) => {
                    Err(EncryptionError::UnsupportedVerificationType(
                        "QR verification is not yet supported in this wrapper".into(),
                    ))
                },
                #[allow(unreachable_patterns)]
                _ => {
                    Err(EncryptionError::UnsupportedVerificationType(
                        "Unknown verification type".into(),
                    ))
                },
            }
        })
    }

    /// Get the user ID of the other party
    pub fn other_user_id(&self) -> &UserId {
        match &*self.inner {
            Verification::SasV1(sas) => sas.other_user_id(),
            Verification::QrV1(qr) => qr.other_user_id(),
            #[allow(unreachable_patterns)]
            _ => panic!("Unknown verification type"),
        }
    }

    /// Get the device ID of the other party, if available
    pub fn other_device_id(&self) -> Option<&DeviceId> {
        match &*self.inner {
            Verification::SasV1(sas) => sas.other_device_id(),
            Verification::QrV1(qr) => qr.other_device_id(),
            #[allow(unreachable_patterns)]
            _ => None,
        }
    }
}

/// A synchronous wrapper around the Matrix SDK Encryption functionality.
///
/// This wrapper enables using the Encryption manager with a synchronous interface,
/// hiding all async complexity behind MatrixFuture objects that properly
/// implement the Future trait.
pub struct CyrumEncryption {
    client: Arc<MatrixClient>,
    runtime_handle: Handle,
}

impl CyrumEncryption {
    /// Create a new CyrumEncryption with the provided Matrix client.
    pub fn new(client: Arc<MatrixClient>) -> Self {
        Self { client, runtime_handle: Handle::current() }
    }

    /// Start a key verification with the given user and device.
    pub fn verify_device(
        &self,
        user_id: &UserId,
        device_id: &DeviceId,
    ) -> MatrixFuture<CyrumVerificationRequest> {
        let user_id = user_id.to_owned();
        let device_id = device_id.to_owned();
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            let request = client
                .encryption()
                .get_verification(&user_id, Some(&device_id))
                .await
                .map_err(EncryptionError::matrix_sdk)?;

            Ok(CyrumVerificationRequest::new(request))
        })
    }

    /// Start a key verification with the given user (without specifying a device).
    pub fn verify_user(&self, user_id: &UserId) -> MatrixFuture<CyrumVerificationRequest> {
        let user_id = user_id.to_owned();
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            let request = client
                .encryption()
                .get_verification(&user_id, None)
                .await
                .map_err(EncryptionError::matrix_sdk)?;

            Ok(CyrumVerificationRequest::new(request))
        })
    }

    /// Enable room key backup.
    pub fn enable_backup(&self) -> MatrixFuture<String> {
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            // In Matrix SDK 0.10.0, we need to create a backup version first
            let version = client
                .encryption()
                .create_backup_version()
                .await
                .map_err(EncryptionError::matrix_sdk)?;

            // Then we need to enable the backup with that version
            client
                .encryption()
                .enable_backup_v1(version.clone())
                .await
                .map_err(EncryptionError::matrix_sdk)?;

            Ok(version)
        })
    }

    /// Restore room keys from backup.
    pub fn restore_backup(&self, passphrase: Option<&str>) -> MatrixFuture<usize> {
        let passphrase = passphrase.map(|s| s.to_owned());
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            // In Matrix SDK 0.10.0, the method signature has changed
            let res = match passphrase {
                Some(pass) => {
                    client
                        .encryption()
                        .restore_backup_with_passphrase_from_latest_version(&pass)
                        .await
                        .map_err(EncryptionError::matrix_sdk)?
                },
                None => {
                    return Err(EncryptionError::InvalidParameter(
                        "Passphrase is required for backup restoration in SDK 0.10.0".to_string(),
                    ));
                },
            };

            Ok(res.imported_count)
        })
    }

    /// Check if a backup exists.
    pub fn has_backup(&self) -> MatrixFuture<bool> {
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            let info = client
                .encryption()
                .backup_info()
                .await
                .map_err(EncryptionError::matrix_sdk)?;

            Ok(info.is_some())
        })
    }

    /// Get recovery key as a string.
    pub fn export_recovery_key(&self) -> MatrixFuture<String> {
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            // In Matrix SDK 0.10.0, we need to get the backup info first
            let backup_info = client
                .encryption()
                .backup_info()
                .await
                .map_err(EncryptionError::matrix_sdk)?;

            let backup_info = backup_info.ok_or_else(|| {
                EncryptionError::MatrixSdk("No backup info available".to_string())
            })?;

            // Then we can export the backup key
            let recovery_key = client
                .encryption()
                .export_backup_key(backup_info.version)
                .await
                .map_err(EncryptionError::matrix_sdk)?;

            Ok(recovery_key.to_base58())
        })
    }

    /// Import recovery key.
    pub fn import_recovery_key(&self, recovery_key: &str) -> MatrixFuture<usize> {
        let recovery_key = recovery_key.to_owned();
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            let key = parse_recovery_key(&recovery_key)
                .map_err(|e| EncryptionError::InvalidRecoveryKey(e))?;

            // In Matrix SDK 0.10.0, we need to get the backup info first
            let backup_info = client
                .encryption()
                .backup_info()
                .await
                .map_err(EncryptionError::matrix_sdk)?;

            let backup_info = backup_info.ok_or_else(|| {
                EncryptionError::MatrixSdk("No backup info available".to_string())
            })?;

            let version = backup_info.version;

            let result = client
                .encryption()
                .receive_room_keys_from_backup(
                    &version,
                    &key,
                    BackupDownloadStrategy::LazyLoadRoomKeys,
                )
                .await
                .map_err(EncryptionError::matrix_sdk)?;

            Ok(result.imported_count)
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
                .map_err(EncryptionError::matrix_sdk)?;

            Ok(device.verification_state())
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
                .map_err(EncryptionError::matrix_sdk)?;

            Ok(device.is_verified())
        })
    }

    /// Subscribe to verification requests.
    pub fn subscribe_to_verification_requests(&self) -> MatrixStream<CyrumVerificationRequest> {
        let client = self.client.clone();

        MatrixStream::spawn(async move {
            let (sender, receiver) = tokio::sync::mpsc::channel(100);

            client.add_event_handler(
                move |ev: matrix_sdk::encryption::verification::VerificationRequest| {
                    let sender = sender.clone();

                    async move {
                        match ev.accept().await {
                            Ok(verification) => {
                                let _ = sender
                                    .send(Ok(CyrumVerificationRequest::new(verification)))
                                    .await;
                            },
                            Err(e) => {
                                let _ = sender.send(Err(EncryptionError::matrix_sdk(e))).await;
                            },
                        }
                    }
                },
            );

            Ok(receiver.into_stream())
        })
    }
}
