use crate::repository::error::RepositoryError;
use crate::repository::third_party::{
    ThirdPartyRepository, BridgeHealth, ProtocolStatistics, ThirdPartyProtocol, 
    ThirdPartyLocation, ThirdPartyUser, BridgeConfig
};
use crate::repository::bridge::BridgeRepository;
use std::collections::HashMap;
use surrealdb::{Connection, Surreal};

// TASK17 SUBTASK 8: Create Third-Party Service
pub struct ThirdPartyService<C: Connection> {
    third_party_repo: ThirdPartyRepository<C>,
    bridge_repo: BridgeRepository<C>,
}

impl<C: Connection> ThirdPartyService<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self {
            third_party_repo: ThirdPartyRepository::new(db.clone()),
            bridge_repo: BridgeRepository::new(db),
        }
    }

    /// Query all third-party protocols with their configurations
    pub async fn query_third_party_protocols(&self) -> Result<HashMap<String, ThirdPartyProtocol>, RepositoryError> {
        let protocols = self.third_party_repo.get_all_protocols().await?;
        
        let mut protocol_map = HashMap::new();
        for protocol in protocols {
            protocol_map.insert(protocol.protocol_id.clone(), protocol);
        }
        
        Ok(protocol_map)
    }

    /// Lookup locations by protocol and search fields
    pub async fn lookup_location(&self, protocol: &str, fields: &HashMap<String, String>) -> Result<Vec<ThirdPartyLocation>, RepositoryError> {
        // Validate protocol exists
        let protocol_config = self.third_party_repo.get_protocol_by_id(protocol).await?;
        if protocol_config.is_none() {
            return Err(RepositoryError::NotFound {
                entity_type: "Protocol".to_string(),
                id: protocol.to_string(),
            });
        }

        // Validate search fields against protocol configuration
        let protocol_config = protocol_config.unwrap();
        for field_name in fields.keys() {
            let field_exists = protocol_config.location_fields
                .iter()
                .any(|f| f.placeholder == *field_name);
            
            if !field_exists {
                return Err(RepositoryError::ValidationError {
                    field: field_name.clone(),
                    message: format!("Field '{}' is not valid for protocol '{}'", field_name, protocol),
                });
            }
        }

        // Perform the lookup
        self.third_party_repo.lookup_third_party_location(protocol, fields).await
    }

    /// Lookup users by protocol and search fields
    pub async fn lookup_user(&self, protocol: &str, fields: &HashMap<String, String>) -> Result<Vec<ThirdPartyUser>, RepositoryError> {
        // Validate protocol exists
        let protocol_config = self.third_party_repo.get_protocol_by_id(protocol).await?;
        if protocol_config.is_none() {
            return Err(RepositoryError::NotFound {
                entity_type: "Protocol".to_string(),
                id: protocol.to_string(),
            });
        }

        // Validate search fields against protocol configuration
        let protocol_config = protocol_config.unwrap();
        for field_name in fields.keys() {
            let field_exists = protocol_config.user_fields
                .iter()
                .any(|f| f.placeholder == *field_name);
            
            if !field_exists {
                return Err(RepositoryError::ValidationError {
                    field: field_name.clone(),
                    message: format!("Field '{}' is not valid for protocol '{}'", field_name, protocol),
                });
            }
        }

        // Perform the lookup
        self.third_party_repo.lookup_third_party_user(protocol, fields).await
    }

    /// Resolve room alias to third-party location
    pub async fn resolve_room_alias(&self, alias: &str) -> Result<Option<ThirdPartyLocation>, RepositoryError> {
        // Validate alias format (should start with # for room aliases)
        if !alias.starts_with('#') {
            return Err(RepositoryError::ValidationError {
                field: "alias".to_string(),
                message: "Room alias must start with '#'".to_string(),
            });
        }

        self.third_party_repo.get_location_by_alias(alias).await
    }

    /// Resolve user ID to third-party user
    pub async fn resolve_user_id(&self, userid: &str) -> Result<Option<ThirdPartyUser>, RepositoryError> {
        // Validate userid format (should start with @ for user IDs)
        if !userid.starts_with('@') {
            return Err(RepositoryError::ValidationError {
                field: "userid".to_string(),
                message: "User ID must start with '@'".to_string(),
            });
        }

        self.third_party_repo.get_user_by_userid(userid).await
    }

    /// Register a bridge protocol with its configuration
    pub async fn register_bridge_protocol(&self, protocol: &ThirdPartyProtocol, bridge_config: &BridgeConfig) -> Result<(), RepositoryError> {
        // Validate protocol and bridge configuration match
        if protocol.protocol_id != bridge_config.protocol {
            return Err(RepositoryError::ValidationError {
                field: "protocol".to_string(),
                message: "Protocol ID mismatch between protocol and bridge configuration".to_string(),
            });
        }

        // Validate bridge configuration
        if bridge_config.bridge_id.is_empty() {
            return Err(RepositoryError::ValidationError {
                field: "bridge_id".to_string(),
                message: "Bridge ID cannot be empty".to_string(),
            });
        }

        if bridge_config.url.is_empty() {
            return Err(RepositoryError::ValidationError {
                field: "url".to_string(),
                message: "Bridge URL cannot be empty".to_string(),
            });
        }

        if bridge_config.as_token.is_empty() {
            return Err(RepositoryError::ValidationError {
                field: "as_token".to_string(),
                message: "Application service token cannot be empty".to_string(),
            });
        }

        if bridge_config.hs_token.is_empty() {
            return Err(RepositoryError::ValidationError {
                field: "hs_token".to_string(),
                message: "Homeserver token cannot be empty".to_string(),
            });
        }

        // Register protocol first
        self.third_party_repo.register_protocol(protocol).await?;

        // Then register bridge
        self.bridge_repo.register_bridge(bridge_config).await?;

        Ok(())
    }

    /// Update bridge health status
    pub async fn update_bridge_health(&self, bridge_id: &str, health_status: BridgeHealth) -> Result<(), RepositoryError> {
        // Validate bridge exists
        let bridge = self.bridge_repo.get_bridge_by_id(bridge_id).await?;
        if bridge.is_none() {
            return Err(RepositoryError::NotFound {
                entity_type: "Bridge".to_string(),
                id: bridge_id.to_string(),
            });
        }

        // Convert health status to bridge status
        let bridge_status = match health_status {
            BridgeHealth::Healthy => crate::repository::third_party::BridgeStatus::Active,
            BridgeHealth::Degraded => crate::repository::third_party::BridgeStatus::Active, // Still active but degraded
            BridgeHealth::Unhealthy => crate::repository::third_party::BridgeStatus::Error,
            BridgeHealth::Unknown => crate::repository::third_party::BridgeStatus::Inactive,
        };

        // Update bridge status
        self.bridge_repo.update_bridge_status(bridge_id, bridge_status).await?;

        Ok(())
    }

    /// Get protocol statistics including bridge information
    pub async fn get_protocol_statistics(&self, protocol: &str) -> Result<ProtocolStatistics, RepositoryError> {
        // Validate protocol exists
        let protocol_config = self.third_party_repo.get_protocol_by_id(protocol).await?;
        if protocol_config.is_none() {
            return Err(RepositoryError::NotFound {
                entity_type: "Protocol".to_string(),
                id: protocol.to_string(),
            });
        }

        // Get bridges for this protocol
        let bridges = self.bridge_repo.get_bridges_for_protocol(protocol).await?;
        let total_bridges = bridges.len() as u64;
        let active_bridges = bridges.iter()
            .filter(|b| matches!(b.status, crate::repository::third_party::BridgeStatus::Active))
            .count() as u64;

        // Get user and location counts
        let empty_fields = HashMap::new();
        let users = self.third_party_repo.lookup_third_party_user(protocol, &empty_fields).await?;
        let locations = self.third_party_repo.lookup_third_party_location(protocol, &empty_fields).await?;

        // Calculate average uptime from bridge statistics
        let mut total_uptime = 0.0;
        let mut bridge_count = 0;
        
        for bridge in &bridges {
            if let Ok(stats) = self.bridge_repo.get_bridge_statistics(&bridge.bridge_id).await {
                total_uptime += stats.uptime_percentage;
                bridge_count += 1;
            }
        }

        let uptime_percentage = if bridge_count > 0 {
            total_uptime / bridge_count as f64
        } else {
            0.0
        };

        Ok(ProtocolStatistics {
            protocol_id: protocol.to_string(),
            total_bridges,
            active_bridges,
            total_users: users.len() as u64,
            total_locations: locations.len() as u64,
            messages_24h: 0, // Would need message tracking implementation
            uptime_percentage,
        })
    }

    /// Get third-party repository reference
    pub fn third_party_repo(&self) -> &ThirdPartyRepository<C> {
        &self.third_party_repo
    }

    /// Get bridge repository reference
    pub fn bridge_repo(&self) -> &BridgeRepository<C> {
        &self.bridge_repo
    }

    // TASK17 SUBTASK 11: Add Protocol Validation

    /// Validate protocol field requirements
    pub async fn validate_protocol_fields(&self, protocol_id: &str, field_type: &str, fields: &HashMap<String, String>) -> Result<(), RepositoryError> {
        let protocol = self.third_party_repo.get_protocol_by_id(protocol_id).await?
            .ok_or_else(|| RepositoryError::NotFound {
                entity_type: "Protocol".to_string(),
                id: protocol_id.to_string(),
            })?;

        let field_definitions = match field_type {
            "user" => &protocol.user_fields,
            "location" => &protocol.location_fields,
            _ => return Err(RepositoryError::ValidationError {
                field: "field_type".to_string(),
                message: "Field type must be 'user' or 'location'".to_string(),
            }),
        };

        // Check all required fields are present
        for field_def in field_definitions {
            if !fields.contains_key(&field_def.placeholder) {
                return Err(RepositoryError::ValidationError {
                    field: field_def.placeholder.clone(),
                    message: format!("Required field '{}' is missing", field_def.placeholder),
                });
            }

            // Validate field value against regexp (simplified validation)
            if let Some(field_value) = fields.get(&field_def.placeholder) {
                // Basic validation - check if field value is not empty
                if field_value.is_empty() {
                    return Err(RepositoryError::ValidationError {
                        field: field_def.placeholder.clone(),
                        message: format!("Field '{}' cannot be empty", field_def.placeholder),
                    });
                }

                // Additional basic format validation based on common patterns
                if field_def.regexp.contains("@") && !field_value.contains('@') {
                    return Err(RepositoryError::ValidationError {
                        field: field_def.placeholder.clone(),
                        message: format!("Field '{}' appears to require an email format", field_def.placeholder),
                    });
                }
            }
        }

        Ok(())
    }

    /// Check protocol instance configurations
    pub async fn validate_protocol_instances(&self, protocol_id: &str) -> Result<(), RepositoryError> {
        let instances = self.third_party_repo.get_protocol_instances(protocol_id).await?;
        
        for instance in instances {
            // Validate instance ID is not empty
            if instance.instance_id.is_empty() {
                return Err(RepositoryError::ValidationError {
                    field: "instance_id".to_string(),
                    message: "Instance ID cannot be empty".to_string(),
                });
            }

            // Validate network ID is not empty
            if instance.network_id.is_empty() {
                return Err(RepositoryError::ValidationError {
                    field: "network_id".to_string(),
                    message: "Network ID cannot be empty".to_string(),
                });
            }

            // Validate description is not empty
            if instance.desc.is_empty() {
                return Err(RepositoryError::ValidationError {
                    field: "desc".to_string(),
                    message: "Instance description cannot be empty".to_string(),
                });
            }

            // Validate instance fields
            for (field_name, field_value) in &instance.fields {
                if field_name.is_empty() {
                    return Err(RepositoryError::ValidationError {
                        field: "field_name".to_string(),
                        message: "Instance field name cannot be empty".to_string(),
                    });
                }

                if field_value.is_empty() {
                    return Err(RepositoryError::ValidationError {
                        field: field_name.clone(),
                        message: format!("Instance field '{}' value cannot be empty", field_name),
                    });
                }
            }
        }

        Ok(())
    }

    /// Verify bridge authentication tokens
    pub async fn validate_bridge_tokens(&self, bridge_id: &str) -> Result<(), RepositoryError> {
        let bridge = self.bridge_repo.get_bridge_by_id(bridge_id).await?
            .ok_or_else(|| RepositoryError::NotFound {
                entity_type: "Bridge".to_string(),
                id: bridge_id.to_string(),
            })?;

        // Validate AS token format (should be a valid token)
        if bridge.as_token.len() < 32 {
            return Err(RepositoryError::ValidationError {
                field: "as_token".to_string(),
                message: "Application service token must be at least 32 characters".to_string(),
            });
        }

        // Validate HS token format (should be a valid token)
        if bridge.hs_token.len() < 32 {
            return Err(RepositoryError::ValidationError {
                field: "hs_token".to_string(),
                message: "Homeserver token must be at least 32 characters".to_string(),
            });
        }

        // Check if tokens are different (security requirement)
        if bridge.as_token == bridge.hs_token {
            return Err(RepositoryError::ValidationError {
                field: "tokens".to_string(),
                message: "Application service token and homeserver token must be different".to_string(),
            });
        }

        Ok(())
    }

    /// Validate third-party network connectivity
    pub async fn validate_network_connectivity(&self, protocol_id: &str) -> Result<NetworkConnectivityResult, RepositoryError> {
        let bridges = self.bridge_repo.get_bridges_for_protocol(protocol_id).await?;
        
        if bridges.is_empty() {
            return Ok(NetworkConnectivityResult {
                protocol_id: protocol_id.to_string(),
                total_bridges: 0,
                healthy_bridges: 0,
                connectivity_status: NetworkConnectivityStatus::Nobridges,
                bridge_statuses: Vec::new(),
            });
        }

        let mut healthy_count = 0;
        let mut bridge_statuses = Vec::new();

        for bridge in &bridges {
            let health_status = match self.bridge_repo.monitor_bridge_health(&bridge.bridge_id).await {
                Ok(health) => {
                    if matches!(health, crate::repository::third_party::BridgeHealth::Healthy) {
                        healthy_count += 1;
                    }
                    health
                },
                Err(_) => crate::repository::third_party::BridgeHealth::Unhealthy,
            };

            bridge_statuses.push(BridgeConnectivityStatus {
                bridge_id: bridge.bridge_id.clone(),
                bridge_name: bridge.name.clone(),
                health_status,
                last_check: chrono::Utc::now(),
            });
        }

        let connectivity_status = if healthy_count == 0 {
            NetworkConnectivityStatus::AllDown
        } else if healthy_count == bridges.len() {
            NetworkConnectivityStatus::AllHealthy
        } else {
            NetworkConnectivityStatus::PartiallyHealthy
        };

        Ok(NetworkConnectivityResult {
            protocol_id: protocol_id.to_string(),
            total_bridges: bridges.len(),
            healthy_bridges: healthy_count,
            connectivity_status,
            bridge_statuses,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NetworkConnectivityResult {
    pub protocol_id: String,
    pub total_bridges: usize,
    pub healthy_bridges: usize,
    pub connectivity_status: NetworkConnectivityStatus,
    pub bridge_statuses: Vec<BridgeConnectivityStatus>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum NetworkConnectivityStatus {
    AllHealthy,
    PartiallyHealthy,
    AllDown,
    Nobridges,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BridgeConnectivityStatus {
    pub bridge_id: String,
    pub bridge_name: String,
    pub health_status: crate::repository::third_party::BridgeHealth,
    pub last_check: chrono::DateTime<chrono::Utc>,
}

// TASK17 SUBTASK 12: Add Application Service Integration

impl<C: Connection> ThirdPartyService<C> {
    /// Link third-party protocols to application services
    pub async fn link_protocol_to_application_service(&self, protocol_id: &str, as_id: &str, as_token: &str) -> Result<(), RepositoryError> {
        // Validate protocol exists
        let _protocol = self.third_party_repo.get_protocol_by_id(protocol_id).await?
            .ok_or_else(|| RepositoryError::NotFound {
                entity_type: "Protocol".to_string(),
                id: protocol_id.to_string(),
            })?;

        // Validate AS token
        if as_token.len() < 32 {
            return Err(RepositoryError::ValidationError {
                field: "as_token".to_string(),
                message: "Application service token must be at least 32 characters".to_string(),
            });
        }

        // Create application service link
        let link = ApplicationServiceLink {
            protocol_id: protocol_id.to_string(),
            as_id: as_id.to_string(),
            as_token: as_token.to_string(),
            created_at: chrono::Utc::now(),
            active: true,
        };

        // Store the link
        let query = r#"
            CREATE application_service_links CONTENT {
                protocol_id: $protocol_id,
                as_id: $as_id,
                as_token: $as_token,
                created_at: $created_at,
                active: $active
            }
        "#;

        self.third_party_repo.db.query(query)
            .bind(("protocol_id", link.protocol_id))
            .bind(("as_id", link.as_id))
            .bind(("as_token", link.as_token))
            .bind(("created_at", link.created_at))
            .bind(("active", link.active))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "link_protocol_to_application_service".to_string(),
            })?;

        Ok(())
    }

    /// Validate AS tokens for third-party operations
    pub async fn validate_as_token_for_operation(&self, as_token: &str, protocol_id: &str, operation: &str) -> Result<bool, RepositoryError> {
        // Get application service link for this protocol
        let query = r#"
            SELECT * FROM application_service_links 
            WHERE protocol_id = $protocol_id AND active = true 
            LIMIT 1
        "#;

        let mut result = self.third_party_repo.db.query(query)
            .bind(("protocol_id", protocol_id.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "validate_as_token_for_operation".to_string(),
            })?;

        let links: Vec<ApplicationServiceLink> = result.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "validate_as_token_for_operation_parse".to_string(),
        })?;

        if let Some(link) = links.first() {
            // Validate token matches
            if link.as_token != as_token {
                return Ok(false);
            }

            // Validate operation is allowed for this AS
            self.validate_as_operation_permissions(&link.as_id, operation).await
        } else {
            Ok(false)
        }
    }

    /// Implement AS-based user and room namespace validation
    pub async fn validate_as_namespace(&self, as_id: &str, namespace_type: &str, identifier: &str) -> Result<bool, RepositoryError> {
        // Get AS namespace configuration
        let query = r#"
            SELECT namespaces FROM application_services 
            WHERE as_id = $as_id 
            LIMIT 1
        "#;

        let mut result = self.third_party_repo.db.query(query)
            .bind(("as_id", as_id.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "validate_as_namespace".to_string(),
            })?;

        let rows: Vec<serde_json::Value> = result.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "validate_as_namespace_parse".to_string(),
        })?;

        if let Some(row) = rows.first()
            && let Some(namespaces) = row.get("namespaces").and_then(|v| v.as_object())
            && let Some(namespace_patterns) = namespaces.get(namespace_type).and_then(|v| v.as_array()) {
                // Check if identifier matches any of the namespace patterns
                for pattern in namespace_patterns {
                    if let Some(pattern_str) = pattern.as_str() {
                        // Simple pattern matching (could be enhanced with regex)
                        if identifier.starts_with(pattern_str) || pattern_str == "*" {
                            return Ok(true);
                        }
                    }
                }
            }

        Ok(false)
    }

    /// Add AS event routing for third-party events
    pub async fn route_third_party_event(&self, protocol_id: &str, event_data: &serde_json::Value) -> Result<String, RepositoryError> {
        // Get application service for this protocol
        let query = r#"
            SELECT as_id FROM application_service_links 
            WHERE protocol_id = $protocol_id AND active = true 
            LIMIT 1
        "#;

        let mut result = self.third_party_repo.db.query(query)
            .bind(("protocol_id", protocol_id.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "route_third_party_event".to_string(),
            })?;

        let rows: Vec<serde_json::Value> = result.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "route_third_party_event_parse".to_string(),
        })?;

        if let Some(row) = rows.first()
            && let Some(as_id) = row.get("as_id").and_then(|v| v.as_str()) {
                // Create event routing record
                let event_id = uuid::Uuid::new_v4().to_string();
                let routing_record = ThirdPartyEventRouting {
                    event_id: event_id.clone(),
                    protocol_id: protocol_id.to_string(),
                    as_id: as_id.to_string(),
                    event_data: event_data.clone(),
                    routed_at: chrono::Utc::now(),
                    status: EventRoutingStatus::Pending,
                };

                // Store routing record
                let insert_query = r#"
                    CREATE third_party_event_routing CONTENT {
                        event_id: $event_id,
                        protocol_id: $protocol_id,
                        as_id: $as_id,
                        event_data: $event_data,
                        routed_at: $routed_at,
                        status: $status
                    }
                "#;

                self.third_party_repo.db.query(insert_query)
                    .bind(("event_id", routing_record.event_id.clone()))
                    .bind(("protocol_id", routing_record.protocol_id))
                    .bind(("as_id", routing_record.as_id))
                    .bind(("event_data", routing_record.event_data))
                    .bind(("routed_at", routing_record.routed_at))
                    .bind(("status", "Pending"))
                    .await
                    .map_err(|e| RepositoryError::DatabaseError {
                        message: e.to_string(),
                        operation: "route_third_party_event_store".to_string(),
                    })?;

                return Ok(event_id);
            }

        Err(RepositoryError::NotFound {
            entity_type: "ApplicationServiceLink".to_string(),
            id: protocol_id.to_string(),
        })
    }

    /// Validate AS operation permissions
    async fn validate_as_operation_permissions(&self, as_id: &str, operation: &str) -> Result<bool, RepositoryError> {
        // Get AS permissions configuration
        let query = r#"
            SELECT permissions FROM application_services 
            WHERE as_id = $as_id 
            LIMIT 1
        "#;

        let mut result = self.third_party_repo.db.query(query)
            .bind(("as_id", as_id.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "validate_as_operation_permissions".to_string(),
            })?;

        let rows: Vec<serde_json::Value> = result.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "validate_as_operation_permissions_parse".to_string(),
        })?;

        if let Some(row) = rows.first()
            && let Some(permissions) = row.get("permissions").and_then(|v| v.as_array()) {
                // Check if operation is in allowed permissions
                for permission in permissions {
                    if let Some(perm_str) = permission.as_str()
                        && (perm_str == operation || perm_str == "*") {
                            return Ok(true);
                        }
                }
            }

        Ok(false)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ApplicationServiceLink {
    pub protocol_id: String,
    pub as_id: String,
    pub as_token: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub active: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ThirdPartyEventRouting {
    pub event_id: String,
    pub protocol_id: String,
    pub as_id: String,
    pub event_data: serde_json::Value,
    pub routed_at: chrono::DateTime<chrono::Utc>,
    pub status: EventRoutingStatus,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum EventRoutingStatus {
    Pending,
    Delivered,
    Failed,
    Retrying,
}