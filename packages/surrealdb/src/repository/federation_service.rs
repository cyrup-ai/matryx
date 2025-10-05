use crate::repository::error::RepositoryError;
use crate::repository::{FederationRepository, RoomRepository, DeviceRepository, EventRepository};
use crate::repository::federation::{RoomHierarchy, ValidationResult};
use crate::repository::device::{DeviceKeysResponse, FederationDevice, OneTimeKey};
use matryx_entity::types::Event;
use serde::{Deserialize, Serialize};
use surrealdb::{Connection, Surreal};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HierarchyResponse {
    pub room_id: String,
    pub children: Vec<HierarchyChild>,
    pub children_state: Vec<Event>,
    pub inaccessible_children: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HierarchyChild {
    pub room_id: String,
    pub via: Vec<String>,
    pub room_type: Option<String>,
    pub suggested: bool,
    pub order: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeysQueryResponse {
    pub device_keys: HashMap<String, HashMap<String, serde_json::Value>>,
    pub failures: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeysClaimResponse {
    pub one_time_keys: HashMap<String, HashMap<String, HashMap<String, serde_json::Value>>>,
    pub failures: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevicesResponse {
    pub user_id: String,
    pub stream_id: i64,
    pub devices: Vec<FederationDevice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateResponse {
    pub auth_chain: Vec<Event>,
    pub pdus: Vec<Event>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingResult {
    pub accepted: bool,
    pub reason: Option<String>,
    pub event_id: String,
}

pub struct FederationService {
    federation_repo: FederationRepository,
    room_repo: RoomRepository,
    device_repo: DeviceRepository,
    event_repo: EventRepository,
}

impl FederationService {
    pub fn new(
        federation_repo: FederationRepository,
        room_repo: RoomRepository,
        device_repo: DeviceRepository,
        event_repo: EventRepository,
    ) -> Self {
        Self {
            federation_repo,
            room_repo,
            device_repo,
            event_repo,
        }
    }

    /// Handle room hierarchy request
    pub async fn handle_room_hierarchy_request(&self, room_id: &str, suggested_only: bool) -> Result<HierarchyResponse, RepositoryError> {
        // Validate room exists and is accessible
        if self.room_repo.get_room_by_id(room_id).await?.is_none() {
            return Err(RepositoryError::NotFound {
                entity_type: "Room".to_string(),
                id: room_id.to_string(),
            });
        }

        // Get hierarchy from federation repository
        let hierarchy = self.federation_repo.get_room_hierarchy(room_id, suggested_only).await?;

        Ok(HierarchyResponse {
            room_id: hierarchy.room_id,
            children: hierarchy.children.into_iter().map(|child| HierarchyChild {
                room_id: child.room_id,
                via: child.via,
                room_type: child.room_type,
                suggested: child.suggested,
                order: child.order,
            }).collect(),
            children_state: hierarchy.children_state,
            inaccessible_children: hierarchy.inaccessible_children,
        })
    }

    /// Handle user keys query
    pub async fn handle_user_keys_query(&self, user_devices: &[(String, Vec<String>)]) -> Result<KeysQueryResponse, RepositoryError> {
        let device_keys_response = self.device_repo.query_device_keys(user_devices).await?;
        
        let mut device_keys = HashMap::new();
        for (user_id, devices) in device_keys_response.device_keys {
            let mut user_devices_map = HashMap::new();
            for (device_id, keys) in devices {
                user_devices_map.insert(device_id, serde_json::to_value(keys)?);
            }
            device_keys.insert(user_id, user_devices_map);
        }

        Ok(KeysQueryResponse {
            device_keys,
            failures: device_keys_response.failures,
        })
    }

    /// Handle user keys claim
    pub async fn handle_user_keys_claim(&self, one_time_keys: &[(String, String, String)]) -> Result<KeysClaimResponse, RepositoryError> {
        let mut claimed_keys = HashMap::new();
        let mut failures = HashMap::new();

        for (user_id, device_id, algorithm) in one_time_keys {
            match self.device_repo.claim_one_time_keys(user_id, device_id, algorithm).await {
                Ok(Some(key)) => {
                    let user_keys = claimed_keys.entry(user_id.clone()).or_insert_with(HashMap::new);
                    let device_keys = user_keys.entry(device_id.clone()).or_insert_with(HashMap::new);
                    device_keys.insert(
                        format!("{}:{}", algorithm, key.key_id),
                        serde_json::to_value(key)?
                    );
                },
                Ok(None) => {
                    // No keys available for this device/algorithm combination
                },
                Err(e) => {
                    failures.insert(
                        format!("{}:{}", user_id, device_id),
                        serde_json::json!({
                            "error": "Failed to claim keys",
                            "details": e.to_string()
                        })
                    );
                }
            }
        }

        Ok(KeysClaimResponse {
            one_time_keys: claimed_keys,
            failures,
        })
    }

    /// Handle user devices query
    pub async fn handle_user_devices_query(&self, user_id: &str) -> Result<DevicesResponse, RepositoryError> {
        let devices = self.device_repo.get_user_devices_for_federation(user_id).await?;
        
        // Generate stream ID (in real implementation, this would be from device list changes)
        let stream_id = chrono::Utc::now().timestamp_millis();

        Ok(DevicesResponse {
            user_id: user_id.to_string(),
            stream_id,
            devices,
        })
    }

    /// Handle room state request
    pub async fn handle_room_state_request(&self, room_id: &str, event_id: Option<&str>) -> Result<StateResponse, RepositoryError> {
        // Validate room exists
        if self.room_repo.get_room_by_id(room_id).await?.is_none() {
            return Err(RepositoryError::NotFound {
                entity_type: "Room".to_string(),
                id: room_id.to_string(),
            });
        }

        // Get room state
        let pdus = self.federation_repo.get_room_state_for_federation(room_id, event_id).await?;

        // Get auth chain for the state events
        let mut auth_chain = Vec::new();
        for pdu in &pdus {
            if let Ok(auth_events) = self.event_repo.get_auth_events_for_event(pdu).await {
                for auth_event in auth_events {
                    if !auth_chain.iter().any(|e: &Event| e.event_id == auth_event.event_id) {
                        auth_chain.push(auth_event);
                    }
                }
            }
        }

        Ok(StateResponse {
            auth_chain,
            pdus,
        })
    }

    /// Validate and process PDU
    pub async fn validate_and_process_pdu(&self, pdu: &Event, origin: &str) -> Result<ProcessingResult, RepositoryError> {
        // Validate PDU with federation repository
        let validation_result = self.federation_repo.validate_pdu(pdu, origin).await?;
        
        if !validation_result.valid {
            return Ok(ProcessingResult {
                accepted: false,
                reason: validation_result.reason,
                event_id: pdu.event_id.clone(),
            });
        }

        // Get room version for validation
        let room_version = self.room_repo.get_room_version(&pdu.room_id).await?;

        // Validate event for federation
        let event_validation = self.event_repo.validate_event_for_federation(pdu, &room_version).await?;
        
        if !event_validation.valid {
            return Ok(ProcessingResult {
                accepted: false,
                reason: event_validation.reason,
                event_id: pdu.event_id.clone(),
            });
        }

        // Validate event signatures
        let signature_validation = self.event_repo.verify_event_signatures(pdu).await?;
        
        if !signature_validation.valid {
            return Ok(ProcessingResult {
                accepted: false,
                reason: Some("Invalid event signatures".to_string()),
                event_id: pdu.event_id.clone(),
            });
        }

        // Validate auth chain
        let auth_events = self.event_repo.get_auth_events_for_event(pdu).await?;
        let auth_valid = self.event_repo.validate_event_auth_chain(pdu, &auth_events).await?;
        
        if !auth_valid {
            return Ok(ProcessingResult {
                accepted: false,
                reason: Some("Invalid auth chain".to_string()),
                event_id: pdu.event_id.clone(),
            });
        }

        // Store the event
        self.event_repo.store_event_with_hash(pdu).await?;

        Ok(ProcessingResult {
            accepted: true,
            reason: None,
            event_id: pdu.event_id.clone(),
        })
    }

    /// Sign outgoing event
    pub async fn sign_outgoing_event(&self, event: &mut Event, destination: &str) -> Result<(), RepositoryError> {
        // Get server name from destination or use default
        let server_name = destination; // In real implementation, would extract from destination
        let key_id = "ed25519:1"; // In real implementation, would get active key ID

        self.event_repo.sign_event(event, server_name, key_id).await
    }

    /// Validate federation request
    pub async fn validate_federation_request(&self, origin: &str, destination: &str, request_id: &str) -> Result<bool, RepositoryError> {
        self.federation_repo.validate_federation_request(origin, destination, request_id).await
    }

    /// Check room federation ACL
    pub async fn check_room_federation_acl(&self, room_id: &str, server_name: &str) -> Result<bool, RepositoryError> {
        // First check room-level federation settings
        if !self.room_repo.validate_room_for_federation(room_id, server_name).await? {
            return Ok(false);
        }

        // Then check federation ACL
        self.federation_repo.check_federation_acl(room_id, server_name).await
    }

    /// Get server keys for federation
    pub async fn get_server_keys(&self, server_name: &str) -> Result<Vec<crate::repository::federation::ServerKey>, RepositoryError> {
        self.federation_repo.get_server_keys(server_name).await
    }

    /// Store server keys from federation
    pub async fn store_server_keys(&self, server_name: &str, keys: &[crate::repository::federation::ServerKey]) -> Result<(), RepositoryError> {
        self.federation_repo.store_server_keys(server_name, keys).await
    }

    /// Handle federation transaction
    pub async fn handle_federation_transaction(&self, origin: &str, transaction_id: &str, pdus: &[Event]) -> Result<HashMap<String, ProcessingResult>, RepositoryError> {
        let mut results = HashMap::new();

        // Validate transaction
        if !self.validate_federation_request(origin, "localhost", transaction_id).await? {
            // Return error for all PDUs
            for pdu in pdus {
                results.insert(pdu.event_id.clone(), ProcessingResult {
                    accepted: false,
                    reason: Some("Invalid federation request".to_string()),
                    event_id: pdu.event_id.clone(),
                });
            }
            return Ok(results);
        }

        // Process each PDU
        for pdu in pdus {
            // Check room federation ACL
            if !self.check_room_federation_acl(&pdu.room_id, origin).await? {
                results.insert(pdu.event_id.clone(), ProcessingResult {
                    accepted: false,
                    reason: Some("Server not allowed in room".to_string()),
                    event_id: pdu.event_id.clone(),
                });
                continue;
            }

            // Validate and process PDU
            match self.validate_and_process_pdu(pdu, origin).await {
                Ok(result) => {
                    results.insert(pdu.event_id.clone(), result);
                },
                Err(e) => {
                    results.insert(pdu.event_id.clone(), ProcessingResult {
                        accepted: false,
                        reason: Some(e.to_string()),
                        event_id: pdu.event_id.clone(),
                    });
                }
            }
        }

        Ok(results)
    }

    /// Get federation statistics
    pub async fn get_federation_statistics(&self, server_name: &str) -> Result<serde_json::Value, RepositoryError> {
        self.federation_repo.get_federation_stats(server_name).await
    }

    /// Cleanup expired federation data
    pub async fn cleanup_expired_federation_data(&self) -> Result<(), RepositoryError> {
        let cutoff = chrono::Utc::now() - chrono::Duration::hours(24);
        
        // Clean up old request logs
        let log_query = "DELETE FROM federation_request_log WHERE created_at < $cutoff";
        let mut log_result = self.federation_repo.get_db()
            .query(log_query)
            .bind(("cutoff", cutoff))
            .await?;
        let logs_deleted: Option<u64> = log_result.take(0).unwrap_or(Some(0));
        
        // Clean up expired transactions
        let txn_query = "DELETE FROM federation_transactions WHERE expires_at < time::now()";
        let mut txn_result = self.federation_repo.get_db()
            .query(txn_query)
            .await?;
        let txns_deleted: Option<u64> = txn_result.take(0).unwrap_or(Some(0));
        
        // Log cleanup statistics
        tracing::debug!(
            "Cleaned up {} federation request logs and {} transactions",
            logs_deleted.unwrap_or(0),
            txns_deleted.unwrap_or(0)
        );
        
        Ok(())
    }
}