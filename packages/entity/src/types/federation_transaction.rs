use crate::types::{EDU, PDU};
use serde::{Deserialize, Serialize};

/// Federation Transaction - Matrix Server-Server API transaction container
///
/// Contains PDUs (Persistent Data Units) and EDUs (Ephemeral Data Units) sent
/// between Matrix homeservers during federation. This is a core Matrix protocol
/// structure for server-to-server communication.
///
/// **Matrix Specification:** Server-Server API Transaction Format
/// **Source:** spec/server/05-md:22-35
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationTransaction {
    /// Origin homeserver that sent this transaction
    pub origin: String,
    /// Timestamp when the transaction was created on the origin server
    pub origin_server_ts: i64,
    /// Persistent Data Units (room events) included in this transaction
    pub pdus: Vec<PDU>,
    /// Optional Ephemeral Data Units (typing notifications, read receipts, etc.)
    pub edus: Option<Vec<EDU>>,
}

impl FederationTransaction {
    pub fn new(
        origin: String,
        origin_server_ts: i64,
        pdus: Vec<PDU>,
        edus: Option<Vec<EDU>>,
    ) -> Self {
        Self { origin, origin_server_ts, pdus, edus }
    }
}
