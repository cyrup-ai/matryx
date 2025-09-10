# Matrix Server-Server API: Device Management

## Overview

Device management is a critical component of Matrix federation that enables secure end-to-end encryption and to-device messaging. This specification defines how servers efficiently synchronize device information, cryptographic keys, and device state across federated homeservers.

The device management system supports:
- **Device List Synchronization**: Real-time updates of user device lists across federation
- **Key Distribution**: Secure distribution of device identity keys, one-time keys, and cross-signing keys
- **Device Verification**: Cross-signing mechanisms for device trust
- **To-Device Messaging**: Direct encrypted messaging between specific devices

## Key Concepts

### Device Lists
Each user maintains a list of registered devices. When devices are added, removed, or modified, servers must notify all federated servers that share rooms with that user.

### Incremental Updates
Device list changes are communicated through incremental EDU updates (`m.device_list_update`) that form a directed acyclic graph, allowing servers to detect missing updates and resynchronize when necessary.

### Cross-Signing
Users can establish cryptographic trust through cross-signing keys:
- **Master Key**: Root of trust for a user's identity
- **Self-Signing Key**: Used to sign the user's own devices
- **User-Signing Key**: Used to sign other users' master keys (not covered in server-server API)

## Device List Synchronization

### Initial Population
When a server needs a remote user's device list for the first time, it queries the `/user/devices/{userId}` endpoint to populate its local cache.

### Incremental Updates
Subsequent changes are applied through `m.device_list_update` EDUs, which provide:
- **Sequential Updates**: Each EDU has a unique `stream_id` per user
- **Dependency Tracking**: `prev_id` field references previous EDUs
- **Gap Detection**: Missing EDUs trigger full resynchronization
- **Idempotency**: Duplicate EDUs can be safely ignored

### Update Triggers
Servers must send device list updates when:
- A user adds or removes a device
- Device information changes (e.g., display name, keys)
- A user joins a room with new federated servers
- Cross-signing keys are updated

## Federation Endpoints

### GET /_matrix/federation/v1/user/devices/{userId}

Retrieves complete device information for a user, including device keys and cross-signing keys.

**Request Parameters:**
- `userId` (string, required): The user ID to retrieve devices for. Must be local to the receiving server.

**Response (200):**
```json
{
  "devices": [
    {
      "device_display_name": "Alice's Mobile Phone",
      "device_id": "JLAFKJWSCS",
      "keys": {
        "algorithms": [
          "m.olm.v1.curve25519-aes-sha2",
          "m.megolm.v1.aes-sha2"
        ],
        "device_id": "JLAFKJWSCS",
        "keys": {
          "curve25519:JLAFKJWSCS": "3C5BFWi2Y8MaVvjM8M22DBmh24PmgR0nPvJOIArzgyI",
          "ed25519:JLAFKJWSCS": "lEuiRJBit0IG6nUf5pUzWTUEsRVVe/HJkoKuEww9ULI"
        },
        "signatures": {
          "@alice:example.com": {
            "ed25519:JLAFKJWSCS": "dSO80A01XiigH3uBiDVx/EjzaoycHcjq9lfQX0uWsqxl2giMIiSPR8a4d291W1ihKJL/a+myXS367WT6NAIcBA"
          }
        },
        "user_id": "@alice:example.com"
      }
    }
  ],
  "master_key": {
    "keys": {
      "ed25519:base64+master+public+key": "base64+master+public+key"
    },
    "usage": ["master"],
    "user_id": "@alice:example.com"
  },
  "self_signing_key": {
    "keys": {
      "ed25519:base64+self+signing+public+key": "base64+self+signing+master+public+key"
    },
    "signatures": {
      "@alice:example.com": {
        "ed25519:base64+master+public+key": "signature+of+self+signing+key"
      }
    },
    "usage": ["self_signing"],
    "user_id": "@alice:example.com"
  },
  "stream_id": 5,
  "user_id": "@alice:example.org"
}
```

### POST /_matrix/federation/v1/user/keys/claim

Claims one-time keys for use in pre-key messages. Returns available one-time keys or fallback keys if no one-time keys are available.

**Request Body:**
```json
{
  "one_time_keys": {
    "@alice:example.com": {
      "JLAFKJWSCS": "signed_curve25519"
    }
  }
}
```

**Response (200):**
```json
{
  "one_time_keys": {
    "@alice:example.com": {
      "JLAFKJWSCS": {
        "signed_curve25519:AAAAHg": {
          "key": "zKbLg+NrIjpnagy+pIY6uPL4ZwEG2v+8F9lmgsnlZzs",
          "signatures": {
            "@alice:example.com": {
              "ed25519:JLAFKJWSCS": "FLWxXqGbwrb8SM3Y795eB6OA8bwBcoMZFXBqnTn58AYWZSqiD45tlBVcDa2L7RwdKXebW/VzDlnfVJ+9jok1Bw"
            }
          }
        }
      }
    }
  }
}
```

### POST /_matrix/federation/v1/user/keys/query

Returns current device keys and cross-signing keys for specified users.

**Request Body:**
```json
{
  "device_keys": {
    "@alice:example.com": []
  }
}
```

**Response (200):**
```json
{
  "device_keys": {
    "@alice:example.com": {
      "JLAFKJWSCS": {
        "algorithms": [
          "m.olm.v1.curve25519-aes-sha2",
          "m.megolm.v1.aes-sha2"
        ],
        "device_id": "JLAFKJWSCS",
        "keys": {
          "curve25519:JLAFKJWSCS": "3C5BFWi2Y8MaVvjM8M22DBmh24PmgR0nPvJOIArzgyI",
          "ed25519:JLAFKJWSCS": "lEuiRJBit0IG6nUf5pUzWTUEsRVVe/HJkoKuEww9ULI"
        },
        "signatures": {
          "@alice:example.com": {
            "ed25519:JLAFKJWSCS": "dSO80A01XiigH3uBiDVx/EjzaoycHcjq9lfQX0uWsqxl2giMIiSPR8a4d291W1ihKJL/a+myXS367WT6NAIcBA"
          }
        },
        "unsigned": {
          "device_display_name": "Alice's mobile phone"
        },
        "user_id": "@alice:example.com"
      }
    }
  },
  "master_keys": {
    "@alice:example.com": {
      "keys": {
        "ed25519:base64+master+public+key": "base64+master+public+key"
      },
      "usage": ["master"],
      "user_id": "@alice:example.com"
    }
  },
  "self_signing_keys": {
    "@alice:example.com": {
      "keys": {
        "ed25519:base64+self+signing+public+key": "base64+self+signing+master+public+key"
      },
      "signatures": {
        "@alice:example.com": {
          "ed25519:base64+master+public+key": "signature+of+self+signing+key"
        }
      },
      "usage": ["self_signing"],
      "user_id": "@alice:example.com"
    }
  }
}
```

## Ephemeral Data Units (EDUs)

### m.device_list_update

Notifies servers when a user's device list changes. Forms a dependency graph through `prev_id` references.

```json
{
  "edu_type": "m.device_list_update",
  "content": {
    "device_display_name": "Mobile",
    "device_id": "QBUAZIFURK", 
    "keys": {
      "algorithms": [
        "m.olm.v1.curve25519-aes-sha2",
        "m.megolm.v1.aes-sha2"
      ],
      "device_id": "JLAFKJWSCS",
      "keys": {
        "curve25519:JLAFKJWSCS": "3C5BFWi2Y8MaVvjM8M22DBmh24PmgR0nPvJOIArzgyI",
        "ed25519:JLAFKJWSCS": "lEuiRJBit0IG6nUf5pUzWTUEsRVVe/HJkoKuEww9ULI"
      },
      "signatures": {
        "@alice:example.com": {
          "ed25519:JLAFKJWSCS": "dSO80A01XiigH3uBiDVx/EjzaoycHcjq9lfQX0uWsqxl2giMIiSPR8a4d291W1ihKJL/a+myXS367WT6NAIcBA"
        }
      },
      "user_id": "@alice:example.com"
    },
    "prev_id": [5],
    "stream_id": 6,
    "user_id": "@john:example.com",
    "deleted": false
  }
}
```

**Key Fields:**
- `stream_id`: Unique sequential ID per user for ordering updates
- `prev_id`: Array of previous stream IDs this update depends on
- `deleted`: Boolean indicating if device was deleted
- `keys`: Device identity keys (omitted for deleted devices)

### m.signing_key_update

Notifies servers when a user's cross-signing keys change.

```json
{
  "edu_type": "m.signing_key_update",
  "content": {
    "user_id": "@alice:example.com",
    "master_key": {
      "keys": {
        "ed25519:base64+master+public+key": "base64+master+public+key"
      },
      "usage": ["master"],
      "user_id": "@alice:example.com"
    },
    "self_signing_key": {
      "keys": {
        "ed25519:base64+self+signing+public+key": "base64+self+signing+master+public+key"
      },
      "signatures": {
        "@alice:example.com": {
          "ed25519:base64+master+public+key": "signature+of+self+signing+key"
        }
      },
      "usage": ["self_signing"],
      "user_id": "@alice:example.com"
    }
  }
}
```

### m.direct_to_device

Enables direct encrypted messaging between specific devices across federation.

```json
{
  "edu_type": "m.direct_to_device", 
  "content": {
    "sender": "@alice:example.com",
    "message_id": "unique_message_identifier",
    "messages": {
      "@bob:remote.server": {
        "DEVICE_ID": {
          "algorithm": "m.olm.v1.curve25519-aes-sha2",
          "sender_key": "curve25519_sender_key",
          "ciphertext": {
            "curve25519_receiver_key": {
              "type": 0,
              "body": "encrypted_message_body"
            }
          }
        },
        "*": {
          "algorithm": "m.room.encrypted",
          "content": "broadcast_to_all_devices"
        }
      }
    }
  }
}
```

## Implementation Guidelines

### Device List Caching

```rust
pub struct DeviceListCache {
    pub devices: HashMap<String, DeviceInfo>,
    pub master_key: Option<CrossSigningKey>,
    pub self_signing_key: Option<CrossSigningKey>,
    pub stream_id: i64,
    pub last_updated: DateTime<Utc>,
}

impl DeviceListCache {
    pub async fn apply_update(&mut self, update: &DeviceListUpdate) -> Result<(), DeviceError> {
        // Verify update sequence
        if !self.can_apply_update(update) {
            return Err(DeviceError::MissingPreviousUpdate);
        }
        
        if update.deleted {
            self.devices.remove(&update.device_id);
        } else {
            let device_info = DeviceInfo {
                device_id: update.device_id.clone(),
                display_name: update.device_display_name.clone(),
                keys: update.keys.clone(),
                deleted: false,
            };
            self.devices.insert(update.device_id.clone(), device_info);
        }
        
        self.stream_id = update.stream_id;
        self.last_updated = Utc::now();
        
        Ok(())
    }
    
    fn can_apply_update(&self, update: &DeviceListUpdate) -> bool {
        // Check if we have all prerequisite updates
        for prev_id in &update.prev_id {
            if *prev_id > self.stream_id {
                return false;
            }
        }
        true
    }
}
```

### One-Time Key Management

```rust
pub struct OneTimeKeyStore {
    pub keys: HashMap<String, Vec<OneTimeKey>>,  // algorithm -> keys
    pub fallback_keys: HashMap<String, OneTimeKey>,
}

impl OneTimeKeyStore {
    pub async fn claim_key(&mut self, algorithm: &str) -> Option<OneTimeKey> {
        // Try one-time keys first
        if let Some(keys) = self.keys.get_mut(algorithm) {
            if !keys.is_empty() {
                return keys.remove(0).into();
            }
        }
        
        // Fall back to fallback key
        self.fallback_keys.get(algorithm).cloned()
    }
    
    pub async fn replenish_keys(&mut self, algorithm: &str, keys: Vec<OneTimeKey>) {
        self.keys.entry(algorithm.to_string())
            .or_insert_with(Vec::new)
            .extend(keys);
    }
}
```

### Cross-Signing Verification

```rust
pub struct CrossSigningVerifier {
    pub crypto: Arc<dyn CryptoProvider>,
}

impl CrossSigningVerifier {
    pub async fn verify_device_signature(
        &self,
        device_keys: &DeviceKeys,
        self_signing_key: &CrossSigningKey,
    ) -> Result<bool, CryptoError> {
        // Extract device signature
        let user_signatures = device_keys.signatures
            .get(&device_keys.user_id)
            .ok_or(CryptoError::MissingSignature)?;
            
        let self_signing_key_id = format!("ed25519:{}", 
            self_signing_key.keys.keys().next()
                .ok_or(CryptoError::InvalidKey)?
                .split(':').nth(1)
                .ok_or(CryptoError::InvalidKey)?);
                
        let signature = user_signatures
            .get(&self_signing_key_id)
            .ok_or(CryptoError::MissingSignature)?;
        
        // Verify signature against canonical JSON
        let canonical_json = self.canonical_json(device_keys)?;
        self.crypto.verify_ed25519_signature(
            signature,
            &canonical_json,
            &self_signing_key.keys.values().next().unwrap()
        )
    }
    
    pub async fn verify_self_signing_key(
        &self,
        self_signing_key: &CrossSigningKey,
        master_key: &CrossSigningKey,
    ) -> Result<bool, CryptoError> {
        // Verify self-signing key is signed by master key
        let master_signatures = self_signing_key.signatures
            .as_ref()
            .and_then(|sigs| sigs.get(&self_signing_key.user_id))
            .ok_or(CryptoError::MissingSignature)?;
            
        let master_key_id = format!("ed25519:{}", 
            master_key.keys.keys().next().unwrap().split(':').nth(1).unwrap());
            
        let signature = master_signatures
            .get(&master_key_id)
            .ok_or(CryptoError::MissingSignature)?;
        
        let canonical_json = self.canonical_json(self_signing_key)?;
        self.crypto.verify_ed25519_signature(
            signature,
            &canonical_json,
            &master_key.keys.values().next().unwrap()
        )
    }
}
```

## Security Considerations

### Key Distribution
1. **Signature Verification**: All device keys must be properly signed
2. **Master Key Validation**: Cross-signing chains must be validated
3. **One-Time Key Uniqueness**: Each one-time key must only be returned once
4. **Key Rotation**: Support for graceful key rotation and revocation

### Update Integrity
1. **Sequence Validation**: Verify stream_id ordering and prev_id dependencies
2. **Gap Detection**: Trigger resynchronization when updates are missing  
3. **Origin Verification**: Validate EDU origin matches device owner's server
4. **Rate Limiting**: Implement reasonable limits on update frequency

### Privacy Protection
1. **Access Control**: Only return device information for users in shared rooms
2. **Metadata Minimization**: Include only necessary device information
3. **Selective Disclosure**: Support filtered device key queries

## Error Handling

### Missing Updates
When receiving a device list update with unknown `prev_id` values:

```rust
pub async fn handle_device_update(&mut self, update: DeviceListUpdate) -> Result<(), DeviceError> {
    if !self.can_apply_update(&update) {
        // Missing prerequisite updates - trigger resync
        self.resynchronize_device_list(&update.user_id).await?;
        
        // Retry applying the update after resync
        self.apply_device_update(update).await
    } else {
        self.apply_device_update(update).await
    }
}

async fn resynchronize_device_list(&mut self, user_id: &str) -> Result<(), DeviceError> {
    let server = extract_server_name(user_id)?;
    let response = self.federation_client
        .get_user_devices(server, user_id)
        .await?;
        
    // Replace local cache with authoritative response
    self.device_cache.insert(user_id.to_string(), DeviceListCache {
        devices: response.devices,
        master_key: response.master_key,
        self_signing_key: response.self_signing_key,
        stream_id: response.stream_id,
        last_updated: Utc::now(),
    });
    
    Ok(())
}
```

### Key Claim Failures
Handle cases where one-time keys are exhausted:

```rust
pub async fn claim_one_time_keys(
    &self,
    server: &str,
    request: &KeyClaimRequest,
) -> Result<KeyClaimResponse, FederationError> {
    let mut response = KeyClaimResponse::new();
    
    for (user_id, device_keys) in &request.one_time_keys {
        let mut user_keys = HashMap::new();
        
        for (device_id, algorithm) in device_keys {
            match self.key_store.claim_key(user_id, device_id, algorithm).await {
                Ok(Some(key)) => {
                    user_keys.insert(device_id.clone(), key);
                }
                Ok(None) => {
                    // No keys available - this is not an error
                    log::warn!("No one-time keys available for {}:{}", user_id, device_id);
                }
                Err(e) => {
                    log::error!("Failed to claim key for {}:{}: {}", user_id, device_id, e);
                }
            }
        }
        
        if !user_keys.is_empty() {
            response.one_time_keys.insert(user_id.clone(), user_keys);
        }
    }
    
    Ok(response)
}
```

## Performance Optimizations

### Batch Processing
Process multiple device updates efficiently:

```rust
pub async fn process_device_updates(&mut self, updates: Vec<DeviceListUpdate>) -> Result<(), DeviceError> {
    // Group updates by user
    let mut user_updates: HashMap<String, Vec<DeviceListUpdate>> = HashMap::new();
    for update in updates {
        user_updates.entry(update.user_id.clone()).or_default().push(update);
    }
    
    // Process each user's updates in sequence
    for (user_id, mut user_updates) in user_updates {
        // Sort by stream_id to ensure proper ordering
        user_updates.sort_by_key(|u| u.stream_id);
        
        for update in user_updates {
            if let Err(e) = self.apply_device_update(update).await {
                log::error!("Failed to apply device update for {}: {}", user_id, e);
                // Trigger resync for this user
                self.resynchronize_device_list(&user_id).await?;
                break;
            }
        }
    }
    
    Ok(())
}
```

### Caching Strategy
Implement efficient device cache management:

```rust
pub struct DeviceCacheManager {
    cache: HashMap<String, DeviceListCache>,
    cache_expiry: HashMap<String, DateTime<Utc>>,
    max_cache_size: usize,
    cache_ttl: Duration,
}

impl DeviceCacheManager {
    pub async fn get_device_list(&mut self, user_id: &str) -> Result<&DeviceListCache, DeviceError> {
        // Check if cache entry exists and is not expired
        if let Some(expiry) = self.cache_expiry.get(user_id) {
            if Utc::now() > *expiry {
                self.cache.remove(user_id);
                self.cache_expiry.remove(user_id);
            }
        }
        
        if !self.cache.contains_key(user_id) {
            // Cache miss - fetch from remote server
            self.fetch_and_cache_device_list(user_id).await?;
        }
        
        self.cache.get(user_id).ok_or(DeviceError::CacheMiss)
    }
    
    async fn fetch_and_cache_device_list(&mut self, user_id: &str) -> Result<(), DeviceError> {
        // Enforce cache size limits
        self.evict_if_necessary();
        
        let server = extract_server_name(user_id)?;
        let response = self.federation_client
            .get_user_devices(server, user_id)
            .await?;
            
        let cache_entry = DeviceListCache {
            devices: response.devices.into_iter()
                .map(|d| (d.device_id.clone(), d))
                .collect(),
            master_key: response.master_key,
            self_signing_key: response.self_signing_key,
            stream_id: response.stream_id,
            last_updated: Utc::now(),
        };
        
        self.cache.insert(user_id.to_string(), cache_entry);
        self.cache_expiry.insert(user_id.to_string(), Utc::now() + self.cache_ttl);
        
        Ok(())
    }
}
```

This comprehensive device management specification ensures secure, efficient, and reliable device synchronization across Matrix federation while providing practical implementation guidance and security best practices.